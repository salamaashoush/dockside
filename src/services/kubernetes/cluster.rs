//! Kubernetes cluster overview operations: nodes, events, namespace CRUD.

use gpui::App;

use crate::services::{Tokio, complete_task, fail_task, start_task};
use crate::state::{LoadState, StateChanged, docker_state};

use super::super::core::{DispatcherEvent, dispatcher};

pub fn refresh_nodes(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).nodes_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.nodes_state = LoadState::Loading);
  }
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_nodes().await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.nodes = items;
          s.nodes_state = LoadState::Loaded;
          cx.emit(StateChanged::NodesUpdated);
        }
        Ok(Err(e)) => s.nodes_state = LoadState::Error(e.to_string()),
        Err(e) => s.nodes_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn refresh_events(cx: &mut App) {
  let state = docker_state(cx);
  let is_initial = matches!(state.read(cx).events_state, LoadState::NotLoaded);
  if is_initial {
    state.update(cx, |s, _| s.events_state = LoadState::Loading);
  }
  let selected_ns = state.read(cx).selected_namespace.clone();
  let namespace = if selected_ns == "all" { None } else { Some(selected_ns) };

  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.list_events(namespace.as_deref()).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await;
    cx.update(|cx| {
      state.update(cx, |s, cx| match result {
        Ok(Ok(items)) => {
          s.events = items;
          s.events_state = LoadState::Loaded;
          cx.emit(StateChanged::EventsUpdated);
        }
        Ok(Err(e)) => s.events_state = LoadState::Error(e.to_string()),
        Err(e) => s.events_state = LoadState::Error(e.to_string()),
      });
    })
  })
  .detach();
}

pub fn create_namespace(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Creating namespace '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.create_namespace(&name).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Namespace '{label}' created"),
          });
        });
        super::pods::refresh_namespaces(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to create namespace '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn cordon_node(name: String, cx: &mut App) {
  set_node_unschedulable(name, true, cx);
}

pub fn uncordon_node(name: String, cx: &mut App) {
  set_node_unschedulable(name, false, cx);
}

fn set_node_unschedulable(name: String, unschedulable: bool, cx: &mut App) {
  let label = name.clone();
  let action = if unschedulable { "Cordoning" } else { "Uncordoning" };
  let task_id = start_task(cx, format!("{action} node '{name}'..."));
  let disp = dispatcher(cx);
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.set_node_unschedulable(&name, unschedulable).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        let verb = if unschedulable { "cordoned" } else { "uncordoned" };
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Node '{label}' {verb}"),
          });
        });
        refresh_nodes(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to update node '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn drain_node(name: String, cx: &mut App) {
  let label = name.clone();
  let task_id = start_task(cx, format!("Draining node '{name}'..."));
  let disp = dispatcher(cx);
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.drain_node(&name).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(count) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Node '{label}' drained ({count} pods evicted)"),
          });
        });
        refresh_nodes(cx);
        super::pods::refresh_pods(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to drain node '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}

pub fn delete_namespace(name: String, cx: &mut App) {
  let task_id = start_task(cx, format!("Deleting namespace '{name}'..."));
  let disp = dispatcher(cx);
  let label = name.clone();
  let tokio_task = Tokio::spawn(cx, async move {
    let client = crate::kubernetes::KubeClient::new().await?;
    client.delete_namespace(&name).await
  });
  cx.spawn(async move |cx| {
    let result = tokio_task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
    cx.update(|cx| match result {
      Ok(()) => {
        complete_task(cx, task_id);
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskCompleted {
            message: format!("Namespace '{label}' delete requested"),
          });
        });
        super::pods::refresh_namespaces(cx);
      }
      Err(e) => {
        fail_task(cx, task_id, e.to_string());
        disp.update(cx, |_, cx| {
          cx.emit(DispatcherEvent::TaskFailed {
            error: format!("Failed to delete namespace '{label}': {e}"),
          });
        });
      }
    })
  })
  .detach();
}
