//! Kubeconfig context discovery + switching.
//!
//! The active context is persisted to `settings.kube_context` so every
//! subsequent `KubeClient::new()` (which loads settings from disk) binds
//! to it. Switching wipes per-cluster state and bumps a generation guard
//! so a slow in-flight request from the previous context can't repaint.

use gpui::App;

use crate::services::Tokio;
use crate::state::{SettingsChanged, StateChanged, docker_state, settings_state};

use super::super::core::{DispatcherEvent, dispatcher};

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
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| {
      state.update(cx, |s, cx| {
        if let Ok(contexts) = result {
          s.set_kube_contexts(contexts);
          cx.emit(StateChanged::KubeContextsUpdated);
        }
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
}
