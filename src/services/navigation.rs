//! View and tab navigation functions

use gpui::App;

use crate::colima::MachineId;
use crate::state::{
  ContainerDetailTab, CurrentView, DeploymentDetailTab, MachineDetailTab, PodDetailTab, ServiceDetailTab, StateChanged,
  docker_state,
};

/// Set the current view
pub fn set_view(view: CurrentView, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_view(view);
    cx.emit(StateChanged::ViewChanged);
  });
}

// ==================== Container Tab Navigation ====================

/// Open a container's terminal tab
pub fn open_container_terminal(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: ContainerDetailTab::Terminal,
    });
  });
}

/// Open a container's logs tab
pub fn open_container_logs(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: ContainerDetailTab::Logs,
    });
  });
}

/// Open a container's inspect tab
pub fn open_container_inspect(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: ContainerDetailTab::Inspect,
    });
  });
}

/// Open a container's files tab
pub fn open_container_files(id: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ContainerTabRequest {
      container_id: id,
      tab: ContainerDetailTab::Files,
    });
  });
}

// ==================== Machine Tab Navigation ====================

/// Open a machine's terminal tab (Colima VMs only - Host doesn't support terminal)
pub fn open_machine_terminal(machine_id: MachineId, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::MachineTabRequest {
      machine_id,
      tab: MachineDetailTab::Terminal,
    });
  });
}

/// Open a machine's files tab (Colima VMs only - Host doesn't support file browsing)
pub fn open_machine_files(machine_id: MachineId, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::MachineTabRequest {
      machine_id,
      tab: MachineDetailTab::Files,
    });
  });
}

// ==================== Pod Tab Navigation ====================

/// Open a pod's info tab
pub fn open_pod_info(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_view(CurrentView::Pods);
    cx.emit(StateChanged::ViewChanged);
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: PodDetailTab::Info,
    });
  });
}

/// Open a pod's terminal tab
pub fn open_pod_terminal(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: PodDetailTab::Terminal,
    });
  });
}

/// Open a pod's logs tab
pub fn open_pod_logs(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: PodDetailTab::Logs,
    });
  });
}

/// Open a pod's describe tab
pub fn open_pod_describe(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: PodDetailTab::Describe,
    });
  });
}

/// Open a pod's YAML tab
pub fn open_pod_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::PodTabRequest {
      pod_name: name,
      namespace,
      tab: PodDetailTab::Yaml,
    });
  });
}

// ==================== Service Tab Navigation ====================

/// Open service with YAML tab selected
pub fn open_service_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::ServiceTabRequest {
      service_name: name.clone(),
      namespace: namespace.clone(),
      tab: ServiceDetailTab::Yaml,
    });
  });
  // Also trigger the YAML fetch
  super::kubernetes::get_service_yaml(name, namespace, cx);
}

// ==================== Deployment Tab Navigation ====================

/// Open deployment with YAML tab selected
pub fn open_statefulset_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_view(CurrentView::StatefulSets);
    state.set_selection(crate::state::Selection::StatefulSet {
      name: name.clone(),
      namespace: namespace.clone(),
    });
    cx.emit(StateChanged::StatefulSetTabRequest {
      name: name.clone(),
      namespace: namespace.clone(),
      tab: crate::state::StatefulSetDetailTab::Yaml,
    });
  });
  super::kubernetes::get_statefulset_yaml(name, namespace, cx);
}

pub fn open_daemonset_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |state, cx| {
    state.set_view(CurrentView::DaemonSets);
    state.set_selection(crate::state::Selection::DaemonSet {
      name: name.clone(),
      namespace: namespace.clone(),
    });
    cx.emit(StateChanged::DaemonSetTabRequest {
      name: name.clone(),
      namespace: namespace.clone(),
      tab: crate::state::DaemonSetDetailTab::Yaml,
    });
  });
  super::kubernetes::get_daemonset_yaml(name, namespace, cx);
}

pub fn open_deployment_yaml(name: String, namespace: String, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::DeploymentTabRequest {
      deployment_name: name.clone(),
      namespace: namespace.clone(),
      tab: DeploymentDetailTab::Yaml,
    });
  });
  // Also trigger the YAML fetch
  super::kubernetes::get_deployment_yaml(name, namespace, cx);
}

/// Request to open scale dialog for a deployment
pub fn request_scale_dialog(name: String, namespace: String, current_replicas: i32, cx: &mut App) {
  let state = docker_state(cx);
  state.update(cx, |_state, cx| {
    cx.emit(StateChanged::DeploymentScaleRequest {
      deployment_name: name,
      namespace,
      current_replicas,
    });
  });
}
