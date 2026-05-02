//! Kubernetes `CronJob` operations

use std::future::Future;

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_cronjobs(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).cronjobs_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.cronjobs_state = LoadState::Loading);
  }
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_cronjobs(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.cronjobs = items;
          s.cronjobs_state = LoadState::Loaded;
          cx.emit(StateChanged::CronJobsUpdated);
        }
        Ok(Err(e)) => s.cronjobs_state = LoadState::Error(e.to_string()),
        Err(e) => s.cronjobs_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn get_cronjob_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_cronjob_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::CronJobYamlLoaded {
          name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

pub fn apply_cronjob_yaml(name: String, namespace: String, yaml: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Applying cronjob '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.apply_cronjob_yaml(&name, &namespace, &yaml).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("CronJob '{label}' applied"),
          });
        });
        refresh_cronjobs(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to apply cronjob '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn delete_cronjob(name: String, namespace: String, cx: &mut App) {
  cronjob_action(cx, "Deleting", name, namespace, |client, n, ns| async move {
    client.delete_cronjob(&n, &ns).await.map(|()| String::new())
  });
}

pub fn set_cronjob_suspend(name: String, namespace: String, suspend: bool, cx: &mut App) {
  let label = if suspend { "Suspending" } else { "Resuming" };
  cronjob_action(cx, label, name, namespace, move |client, n, ns| async move {
    client
      .set_cronjob_suspend(&n, &ns, suspend)
      .await
      .map(|()| String::new())
  });
}

pub fn trigger_cronjob(name: String, namespace: String, cx: &mut App) {
  cronjob_action(cx, "Triggering", name, namespace, |client, n, ns| async move {
    client.trigger_cronjob(&n, &ns).await
  });
}

fn cronjob_action<F, Fut>(cx: &mut App, verb: &'static str, name: String, namespace: String, op: F)
where
  F: FnOnce(crate::kubernetes::KubeClient, String, String) -> Fut + Send + 'static,
  Fut: Future<Output = anyhow::Result<String>> + Send,
{
  let task_id = start_task(cx, format!("{verb} cronjob '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    op(client, name, namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(extra) => {
        complete_task(cx, task_id);
        let msg = if extra.is_empty() {
          format!("{verb} cronjob '{label}' OK")
        } else {
          format!("{verb} cronjob '{label}' → {extra}")
        };
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
        refresh_cronjobs(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed: {verb} cronjob '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
