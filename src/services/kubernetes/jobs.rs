//! Kubernetes `Job` operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_jobs(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).jobs_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.jobs_state = LoadState::Loading);
  }
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_jobs(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.jobs = items;
          s.jobs_state = LoadState::Loaded;
          cx.emit(StateChanged::JobsUpdated);
        }
        Ok(Err(e)) => s.jobs_state = LoadState::Error(e.to_string()),
        Err(e) => s.jobs_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn get_job_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_job_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::JobYamlLoaded {
          name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

pub fn apply_job_yaml(name: String, namespace: String, yaml: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Applying job '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.apply_job_yaml(&name, &namespace, &yaml).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Job '{label}' applied"),
          });
        });
        refresh_jobs(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to apply job '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn delete_job(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting job '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_job(&name, &namespace).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Job '{label}' deleted"),
          });
        });
        refresh_jobs(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete job '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
