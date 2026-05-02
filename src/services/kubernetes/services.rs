//! Kubernetes service operations

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

/// Refresh services list
pub fn refresh_services(cx: &mut App) {
  use crate::state::LoadState;

  let state = docker_state(cx);

  // Only show loading state on initial load, not on background refreshes
  let is_initial_load = matches!(state.read(cx).services_state, LoadState::NotLoaded);
  if is_initial_load {
    state.update(cx, |state, _cx| {
      state.set_services_loading();
    });
  }

  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_services(namespace.as_deref()).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await;

    cx.update(|cx| {
      state.update(cx, |state, cx| match result {
        Ok(Ok(services)) => {
          state.set_k8s_error(None);
          state.set_services(services);
          cx.emit(StateChanged::ServicesUpdated);
        }
        Ok(Err(e)) => {
          let error_msg = e.to_string();
          state.set_k8s_error(Some(error_msg.clone()));
          state.set_services_error(error_msg);
        }
        Err(join_err) => {
          let error_msg = join_err.to_string();
          state.set_k8s_error(Some(error_msg.clone()));
          state.set_services_error(error_msg);
        }
      });
    })
  })
  .detach();
}

/// Delete a service
pub fn delete_service(name: String, namespace: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting service '{name}'..."));
  let name_clone = name.clone();
  let _state = docker_state(cx);
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_service(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Service '{name_clone}' deleted"),
          });
        });
        refresh_services(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete service '{name_clone}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

/// Get service YAML
pub fn get_service_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  let name_clone = name.clone();
  let namespace_clone = namespace.clone();

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.get_service_yaml(&name, &namespace).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    let yaml = match result {
      Ok(y) => y,
      Err(e) => format!("Error: {e}"),
    };

    cx.update(|cx| {
      state.update(cx, |_state, cx| {
        cx.emit(StateChanged::ServiceYamlLoaded {
          service_name: name_clone,
          namespace: namespace_clone,
          yaml,
        });
      });
    })
  })
  .detach();
}

/// Create a new Kubernetes service
pub fn apply_service_yaml(name: String, namespace: String, yaml: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Applying service '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.apply_service_yaml(&name, &namespace, &yaml).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Service '{label}' applied"),
          });
        });
        refresh_services(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to apply service '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn create_service(options: crate::kubernetes::CreateServiceOptions, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating service '{}'...", options.name));
  let name = options.name.clone();
  let disp = dispatcher(cx);

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.create_service(options).await
  });

  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));

    cx.update(|cx| match result {
      Ok(msg) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted { message: msg });
        });
        refresh_services(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create service '{name}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
