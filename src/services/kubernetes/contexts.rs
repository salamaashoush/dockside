//! Kubeconfig context discovery + switching.
//!
//! The active context is persisted to `settings.kube_context` so every
//! subsequent `KubeClient::new()` (which loads settings from disk) binds
//! to it. Switching wipes per-cluster state and bumps a generation guard
//! so a slow in-flight request from the previous context can't repaint.

use std::path::PathBuf;

use gpui::App;

use crate::kubernetes::{Kubeconfigs, NewCluster};
use crate::services::Tokio;
use crate::state::{SettingsChanged, StateChanged, docker_state, settings_state};

use super::super::core::{DispatcherEvent, dispatcher};

fn toast_ok(cx: &mut App, message: String) {
  dispatcher(cx).update(cx, |_, cx| {
    cx.emit(DispatcherEvent::TaskCompleted { message });
  });
}

fn toast_err(cx: &mut App, error: String) {
  dispatcher(cx).update(cx, |_, cx| {
    cx.emit(DispatcherEvent::TaskFailed { error });
  });
}

/// Startup hook: restore the persisted active context and parse the
/// kubeconfig so the switcher is populated from launch. No-op when the
/// Kubernetes feature is disabled.
pub fn bootstrap_kube_contexts(cx: &mut App) {
  let settings = settings_state(cx).read(cx).settings.clone();
  if !settings.kubernetes_enabled {
    return;
  }
  if !settings.kube_context.is_empty() {
    docker_state(cx).update(cx, |s, _cx| {
      s.set_active_kube_context(Some(settings.kube_context.clone()));
    });
  }
  refresh_kube_contexts(cx);
}

/// Reload the kubeconfig context list (sync parse, no API connection).
pub fn refresh_kube_contexts(cx: &mut App) {
  let state = docker_state(cx);
  let tokio_task = Tokio::spawn(cx, async move { crate::kubernetes::list_kube_contexts() });
  cx.spawn(async move |cx| {
    let contexts = tokio_task.await.unwrap_or_default();
    cx.update(|cx| {
      state.update(cx, |s, cx| {
        s.set_kube_contexts(contexts);
        cx.emit(StateChanged::KubeContextsUpdated);
      });
    })
  })
  .detach();
}

/// Switch the active kubeconfig context: persist it, clear all k8s state,
/// bump the generation guard, then fan out a full reload across every
/// k8s view. No-op if `name` is already active.
pub fn set_kube_context(name: String, cx: &mut App) {
  let state = docker_state(cx);
  if state.read(cx).current_kube_context_name().as_deref() == Some(name.as_str()) {
    return;
  }

  // Persist so KubeClient::new() (loads settings from disk) picks it up.
  settings_state(cx).update(cx, |s, cx| {
    s.settings.kube_context.clone_from(&name);
    if let Err(e) = s.settings.save() {
      tracing::warn!("Failed to persist kube_context: {e}");
    }
    cx.emit(SettingsChanged::SettingsUpdated);
  });

  let label = name.clone();
  state.update(cx, |s, cx| {
    s.set_active_kube_context(Some(name));
    s.clear_k8s_data();
    s.bump_kube_generation();
    cx.emit(StateChanged::KubeContextSwitched);
    // Nudge views that key off these so they show the loading state.
    cx.emit(StateChanged::PodsUpdated);
    cx.emit(StateChanged::NamespacesUpdated);
    cx.emit(StateChanged::NodesUpdated);
  });

  dispatcher(cx).update(cx, |_, cx| {
    cx.emit(DispatcherEvent::TaskCompleted {
      message: format!("Switched to context '{label}'"),
    });
  });

  // Re-parse contexts (updates the current-context marker), then refetch
  // every resource under the new context.
  refresh_kube_contexts(cx);
  super::pods::refresh_namespaces(cx);
  super::pods::refresh_pods(cx);
  super::deployments::refresh_deployments(cx);
  super::services::refresh_services(cx);
  super::secrets::refresh_secrets(cx);
  super::configmaps::refresh_configmaps(cx);
  super::statefulsets::refresh_statefulsets(cx);
  super::daemonsets::refresh_daemonsets(cx);
  super::jobs::refresh_jobs(cx);
  super::cronjobs::refresh_cronjobs(cx);
  super::ingresses::refresh_ingresses(cx);
  super::pvcs::refresh_pvcs(cx);
  super::cluster::refresh_nodes(cx);
  super::cluster::refresh_events(cx);
  super::cluster::refresh_node_metrics(cx);
}

// ============================================================================
// Cluster CRUD (kubeconfig editor) — all writes go through Kubeconfigs,
// which edits per-origin file atomically with a one-time .dockside.bak.
// ============================================================================

/// Add a manually-described cluster to the primary kubeconfig.
pub fn add_cluster(spec: NewCluster, cx: &mut App) {
  let name = spec.context_name.clone();
  let task = Tokio::spawn(cx, async move { Kubeconfigs::discover().add(&spec) });
  cx.spawn(async move |cx| {
    let result = task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        toast_ok(cx, format!("Cluster '{name}' added"));
        refresh_kube_contexts(cx);
      }
      Err(e) => toast_err(cx, format!("Failed to add cluster: {e}")),
    })
  })
  .detach();
}

/// Merge an external kubeconfig file into the primary kubeconfig.
pub fn import_kubeconfig_file(path: PathBuf, cx: &mut App) {
  let shown = path.display().to_string();
  let task = Tokio::spawn(cx, async move { Kubeconfigs::discover().import_file(&path) });
  cx.spawn(async move |cx| {
    let result = task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(n) => {
        toast_ok(cx, format!("Imported {n} context(s) from {shown}"));
        refresh_kube_contexts(cx);
      }
      Err(e) => toast_err(cx, format!("Import failed: {e}")),
    })
  })
  .detach();
}

/// Delete a context. If it was the app's active context, fall back to the
/// kubeconfig default on next client build.
pub fn remove_kube_context(name: String, cx: &mut App) {
  let was_active = docker_state(cx).read(cx).current_kube_context_name().as_deref() == Some(name.as_str());
  let label = name.clone();
  let task = Tokio::spawn(cx, async move { Kubeconfigs::discover().remove_context(&name) });
  cx.spawn(async move |cx| {
    let result = task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        if was_active {
          settings_state(cx).update(cx, |s, cx| {
            s.settings.kube_context.clear();
            let _ = s.settings.save();
            cx.emit(SettingsChanged::SettingsUpdated);
          });
          docker_state(cx).update(cx, |s, _| s.set_active_kube_context(None));
        }
        toast_ok(cx, format!("Context '{label}' removed"));
        refresh_kube_contexts(cx);
      }
      Err(e) => toast_err(cx, format!("Failed to remove context: {e}")),
    })
  })
  .detach();
}

/// Rename a context and/or change its default namespace.
pub fn edit_kube_context(old_name: String, new_name: String, namespace: Option<String>, cx: &mut App) {
  let was_active = docker_state(cx).read(cx).current_kube_context_name().as_deref() == Some(old_name.as_str());
  let old_for_call = old_name.clone();
  let new_for_call = new_name.clone();
  let ns_for_call = namespace;
  let task = Tokio::spawn(cx, async move {
    Kubeconfigs::discover().edit_context(&old_for_call, &new_for_call, ns_for_call.as_deref(), None, None)
  });
  cx.spawn(async move |cx| {
    let result = task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        if was_active && old_name != new_name {
          let nn = new_name.clone();
          settings_state(cx).update(cx, |s, cx| {
            s.settings.kube_context.clone_from(&nn);
            let _ = s.settings.save();
            cx.emit(SettingsChanged::SettingsUpdated);
          });
          docker_state(cx).update(cx, |s, _| s.set_active_kube_context(Some(nn)));
        }
        toast_ok(cx, format!("Context '{old_name}' updated"));
        refresh_kube_contexts(cx);
      }
      Err(e) => toast_err(cx, format!("Failed to edit context: {e}")),
    })
  })
  .detach();
}

/// Make a context the kubeconfig `current-context` *and* the app's active
/// context (full reload), so it behaves like `kubectl config use-context`.
pub fn set_current_kube_context(name: String, cx: &mut App) {
  let for_write = name.clone();
  let task = Tokio::spawn(cx, async move { Kubeconfigs::discover().set_current(&for_write) });
  cx.spawn(async move |cx| {
    let result = task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => set_kube_context(name.clone(), cx),
      Err(e) => toast_err(cx, format!("Failed to set current context: {e}")),
    })
  })
  .detach();
}

/// Probe a context: build a client for it and read the API server version.
pub fn test_kube_connection(name: String, cx: &mut App) {
  let label = name.clone();
  let task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::for_context(Some(&name)).await?;
    client.server_version().await
  });
  cx.spawn(async move |cx| {
    let result = task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(v) => toast_ok(cx, format!("'{label}' reachable — server {v}")),
      Err(e) => toast_err(cx, format!("'{label}' unreachable: {e}")),
    })
  })
  .detach();
}
