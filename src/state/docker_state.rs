use gpui::{App, AppContext, Entity, EventEmitter, Global};

use crate::colima::ColimaVm;
use crate::docker::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo};
use crate::kubernetes::{DeploymentInfo, PodInfo, ServiceInfo};

use super::app_state::CurrentView;

use crate::docker::VolumeFileEntry;

/// Image inspect data for detailed view
#[derive(Clone, Debug, Default)]
pub struct ImageInspectData {
  pub config_cmd: Option<Vec<String>>,
  pub config_workdir: Option<String>,
  pub config_env: Vec<(String, String)>,
  pub config_entrypoint: Option<Vec<String>>,
  pub config_exposed_ports: Vec<String>,
  pub used_by: Vec<String>,
}

/// Event emitted when docker state changes
#[derive(Clone, Debug)]
pub enum StateChanged {
  MachinesUpdated,
  ContainersUpdated,
  ImagesUpdated,
  VolumesUpdated,
  NetworksUpdated,
  PodsUpdated,
  NamespacesUpdated,
  ViewChanged,
  Loading,
  VolumeFilesLoaded {
    volume_name: String,
    path: String,
    files: Vec<VolumeFileEntry>,
  },
  VolumeFilesError {
    volume_name: String,
  },
  ImageInspectLoaded {
    image_id: String,
    data: ImageInspectData,
  },
  PodLogsLoaded {
    pod_name: String,
    namespace: String,
    logs: String,
  },
  PodDescribeLoaded {
    pod_name: String,
    namespace: String,
    describe: String,
  },
  PodYamlLoaded {
    pod_name: String,
    namespace: String,
    yaml: String,
  },
  /// Request to open a machine with a specific tab
  MachineTabRequest {
    machine_name: String,
    tab: usize,
  },
  /// Request to open edit dialog for a machine
  EditMachineRequest {
    machine_name: String,
  },
  /// Request to open a container with a specific tab
  ContainerTabRequest {
    container_id: String,
    tab: usize,
  },
  /// Request to open rename dialog for a container
  RenameContainerRequest {
    container_id: String,
    current_name: String,
  },
  /// Request to open commit dialog for a container
  CommitContainerRequest {
    container_id: String,
    container_name: String,
  },
  /// Request to open export dialog for a container
  ExportContainerRequest {
    container_id: String,
    container_name: String,
  },
  /// Request to open a pod with a specific tab
  PodTabRequest {
    pod_name: String,
    namespace: String,
    tab: usize,
  },
  // Services
  ServicesUpdated,
  ServiceYamlLoaded {
    service_name: String,
    namespace: String,
    yaml: String,
  },
  /// Request to open a service with a specific tab
  ServiceTabRequest {
    service_name: String,
    namespace: String,
    tab: usize,
  },
  // Deployments
  DeploymentsUpdated,
  DeploymentYamlLoaded {
    deployment_name: String,
    namespace: String,
    yaml: String,
  },
  /// Request to open a deployment with a specific tab
  DeploymentTabRequest {
    deployment_name: String,
    namespace: String,
    tab: usize,
  },
  /// Request to open scale dialog for a deployment
  DeploymentScaleRequest {
    deployment_name: String,
    namespace: String,
    current_replicas: i32,
  },
}

/// Global docker state - all views subscribe to this
pub struct DockerState {
  // Docker Data
  pub colima_vms: Vec<ColimaVm>,
  pub containers: Vec<ContainerInfo>,
  pub images: Vec<ImageInfo>,
  pub volumes: Vec<VolumeInfo>,
  pub networks: Vec<NetworkInfo>,

  // Kubernetes Data
  pub pods: Vec<PodInfo>,
  pub services: Vec<ServiceInfo>,
  pub deployments: Vec<DeploymentInfo>,
  pub namespaces: Vec<String>,
  pub selected_namespace: String,
  pub k8s_available: bool,

  // UI state
  pub current_view: CurrentView,
  pub active_detail_tab: usize,

  // Loading states
  pub is_loading: bool,
}

impl DockerState {
  pub fn new() -> Self {
    Self {
      colima_vms: Vec::new(),
      containers: Vec::new(),
      images: Vec::new(),
      volumes: Vec::new(),
      networks: Vec::new(),
      pods: Vec::new(),
      services: Vec::new(),
      deployments: Vec::new(),
      namespaces: vec!["default".to_string()],
      selected_namespace: "default".to_string(),
      k8s_available: false,
      current_view: CurrentView::default(),
      active_detail_tab: 0,
      is_loading: true,
    }
  }

  // Machines
  pub fn set_machines(&mut self, vms: Vec<ColimaVm>) {
    self.colima_vms = vms;
  }

  // Containers
  pub fn set_containers(&mut self, containers: Vec<ContainerInfo>) {
    self.containers = containers;
  }

  // Images
  pub fn set_images(&mut self, images: Vec<ImageInfo>) {
    self.images = images;
  }

  // Volumes
  pub fn set_volumes(&mut self, volumes: Vec<VolumeInfo>) {
    self.volumes = volumes;
  }

  // Networks
  pub fn set_networks(&mut self, networks: Vec<NetworkInfo>) {
    self.networks = networks;
  }

  // Pods (Kubernetes)
  pub fn set_pods(&mut self, pods: Vec<PodInfo>) {
    self.pods = pods;
  }

  pub fn get_pod(&self, name: &str, namespace: &str) -> Option<&PodInfo> {
    self.pods.iter().find(|p| p.name == name && p.namespace == namespace)
  }

  pub fn set_namespaces(&mut self, namespaces: Vec<String>) {
    self.namespaces = namespaces;
  }

  pub fn set_selected_namespace(&mut self, namespace: String) {
    self.selected_namespace = namespace;
  }

  pub fn set_k8s_available(&mut self, available: bool) {
    self.k8s_available = available;
  }

  // Services (Kubernetes)
  pub fn set_services(&mut self, services: Vec<ServiceInfo>) {
    self.services = services;
  }

  pub fn get_service(&self, name: &str, namespace: &str) -> Option<&ServiceInfo> {
    self
      .services
      .iter()
      .find(|s| s.name == name && s.namespace == namespace)
  }

  // Deployments (Kubernetes)
  pub fn set_deployments(&mut self, deployments: Vec<DeploymentInfo>) {
    self.deployments = deployments;
  }

  pub fn get_deployment(&self, name: &str, namespace: &str) -> Option<&DeploymentInfo> {
    self
      .deployments
      .iter()
      .find(|d| d.name == name && d.namespace == namespace)
  }

  // Navigation
  pub fn set_view(&mut self, view: CurrentView) {
    self.current_view = view;
    self.active_detail_tab = 0;
  }
}

impl Default for DockerState {
  fn default() -> Self {
    Self::new()
  }
}

// Enable event emission for reactive updates
impl EventEmitter<StateChanged> for DockerState {}

/// Global wrapper for `DockerState`
pub struct GlobalDockerState(pub Entity<DockerState>);

impl Global for GlobalDockerState {}

/// Initialize the global docker state
pub fn init_docker_state(cx: &mut App) -> Entity<DockerState> {
  let state = cx.new(|_cx| DockerState::new());
  cx.set_global(GlobalDockerState(state.clone()));
  state
}

/// Get the global docker state entity
pub fn docker_state(cx: &App) -> Entity<DockerState> {
  cx.global::<GlobalDockerState>().0.clone()
}
