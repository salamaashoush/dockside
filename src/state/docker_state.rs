use gpui::{App, AppContext, Entity, EventEmitter, Global};

use crate::colima::ColimaVm;
use crate::docker::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo};
use crate::kubernetes::{DeploymentInfo, PodInfo, ServiceInfo};

use super::app_state::CurrentView;

use crate::docker::VolumeFileEntry;

/// Tab indices for machine detail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum MachineDetailTab {
  #[default]
  Info = 0,
  Processes = 1,
  Stats = 2,
  Logs = 3,
  Terminal = 4,
  Files = 5,
}

impl MachineDetailTab {
  pub const ALL: [MachineDetailTab; 6] = [
    MachineDetailTab::Info,
    MachineDetailTab::Processes,
    MachineDetailTab::Stats,
    MachineDetailTab::Logs,
    MachineDetailTab::Terminal,
    MachineDetailTab::Files,
  ];

  pub fn label(self) -> &'static str {
    match self {
      MachineDetailTab::Info => "Info",
      MachineDetailTab::Processes => "Processes",
      MachineDetailTab::Stats => "Stats",
      MachineDetailTab::Logs => "Logs",
      MachineDetailTab::Terminal => "Terminal",
      MachineDetailTab::Files => "Files",
    }
  }
}

/// Tab indices for container detail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum ContainerDetailTab {
  #[default]
  Info = 0,
  Logs = 1,
  Terminal = 2,
  Files = 3,
  Inspect = 4,
}

impl ContainerDetailTab {
  pub const ALL: [ContainerDetailTab; 5] = [
    ContainerDetailTab::Info,
    ContainerDetailTab::Logs,
    ContainerDetailTab::Terminal,
    ContainerDetailTab::Files,
    ContainerDetailTab::Inspect,
  ];

  pub fn label(self) -> &'static str {
    match self {
      ContainerDetailTab::Info => "Info",
      ContainerDetailTab::Logs => "Logs",
      ContainerDetailTab::Terminal => "Terminal",
      ContainerDetailTab::Files => "Files",
      ContainerDetailTab::Inspect => "Inspect",
    }
  }
}

/// Tab indices for pod detail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum PodDetailTab {
  #[default]
  Info = 0,
  Logs = 1,
  Terminal = 2,
  Describe = 3,
  Yaml = 4,
}

impl PodDetailTab {
  pub const ALL: [PodDetailTab; 5] = [
    PodDetailTab::Info,
    PodDetailTab::Logs,
    PodDetailTab::Terminal,
    PodDetailTab::Describe,
    PodDetailTab::Yaml,
  ];

  pub fn label(self) -> &'static str {
    match self {
      PodDetailTab::Info => "Info",
      PodDetailTab::Logs => "Logs",
      PodDetailTab::Terminal => "Terminal",
      PodDetailTab::Describe => "Describe",
      PodDetailTab::Yaml => "YAML",
    }
  }
}

/// Tab indices for service detail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum ServiceDetailTab {
  #[default]
  Info = 0,
  Ports = 1,
  Endpoints = 2,
  Yaml = 3,
}

/// Tab indices for deployment detail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum DeploymentDetailTab {
  #[default]
  Info = 0,
  Pods = 1,
  Yaml = 2,
}

/// Represents the currently selected item across all views
/// This enables keyboard shortcuts to act on the selection
#[derive(Clone, Debug, Default)]
pub enum Selection {
  #[default]
  None,
  Container(ContainerInfo),
  Image(ImageInfo),
  Volume(String),  // Volume name
  Network(String), // Network ID
  Pod {
    name: String,
    namespace: String,
  },
  Deployment {
    name: String,
    namespace: String,
  },
  Service {
    name: String,
    namespace: String,
  },
  Machine(String), // Machine name
}

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
  SelectionChanged,
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
    tab: MachineDetailTab,
  },
  /// Request to open edit dialog for a machine
  EditMachineRequest {
    machine_name: String,
  },
  /// Request to open a container with a specific tab
  ContainerTabRequest {
    container_id: String,
    tab: ContainerDetailTab,
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
    tab: PodDetailTab,
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
    tab: ServiceDetailTab,
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
    tab: DeploymentDetailTab,
  },
  /// Request to open scale dialog for a deployment
  DeploymentScaleRequest {
    deployment_name: String,
    namespace: String,
    current_replicas: i32,
  },
}

/// Represents the load state of a resource
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LoadState {
  #[default]
  NotLoaded,
  Loading,
  Loaded,
  Error(String),
}

#[allow(dead_code)]
impl LoadState {
  pub fn is_loading(&self) -> bool {
    matches!(self, LoadState::Loading)
  }

  pub fn is_loaded(&self) -> bool {
    matches!(self, LoadState::Loaded)
  }

  pub fn is_error(&self) -> bool {
    matches!(self, LoadState::Error(_))
  }

  pub fn error_message(&self) -> Option<&str> {
    match self {
      LoadState::Error(msg) => Some(msg),
      _ => None,
    }
  }
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
  /// Error message for K8s connectivity issues
  pub k8s_error: Option<String>,

  // UI state
  pub current_view: CurrentView,
  pub active_detail_tab: usize,
  /// Currently selected item - used by keyboard shortcuts
  pub selection: Selection,

  // Loading states - general loading indicator
  pub is_loading: bool,

  // Per-resource load states (tracks loading, loaded, and error)
  pub containers_state: LoadState,
  pub images_state: LoadState,
  pub volumes_state: LoadState,
  pub networks_state: LoadState,
  pub pods_state: LoadState,
  pub services_state: LoadState,
  pub deployments_state: LoadState,
  pub machines_state: LoadState,
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
      k8s_error: None,
      current_view: CurrentView::default(),
      active_detail_tab: 0,
      selection: Selection::None,
      is_loading: true,
      // Per-resource load states
      containers_state: LoadState::NotLoaded,
      images_state: LoadState::NotLoaded,
      volumes_state: LoadState::NotLoaded,
      networks_state: LoadState::NotLoaded,
      pods_state: LoadState::NotLoaded,
      services_state: LoadState::NotLoaded,
      deployments_state: LoadState::NotLoaded,
      machines_state: LoadState::NotLoaded,
    }
  }

  // Selection management
  pub fn set_selection(&mut self, selection: Selection) {
    self.selection = selection;
  }

  // Machines
  pub fn set_machines(&mut self, vms: Vec<ColimaVm>) {
    self.colima_vms = vms;
    self.machines_state = LoadState::Loaded;
  }

  #[allow(dead_code)]
  pub fn set_machines_loading(&mut self) {
    self.machines_state = LoadState::Loading;
  }

  #[allow(dead_code)]
  pub fn set_machines_error(&mut self, error: String) {
    self.machines_state = LoadState::Error(error);
  }

  // Containers
  pub fn set_containers(&mut self, containers: Vec<ContainerInfo>) {
    self.containers = containers;
    self.containers_state = LoadState::Loaded;
  }

  #[allow(dead_code)]
  pub fn set_containers_loading(&mut self) {
    self.containers_state = LoadState::Loading;
  }

  #[allow(dead_code)]
  pub fn set_containers_error(&mut self, error: String) {
    self.containers_state = LoadState::Error(error);
  }

  // Images
  pub fn set_images(&mut self, images: Vec<ImageInfo>) {
    self.images = images;
    self.images_state = LoadState::Loaded;
  }

  #[allow(dead_code)]
  pub fn set_images_loading(&mut self) {
    self.images_state = LoadState::Loading;
  }

  #[allow(dead_code)]
  pub fn set_images_error(&mut self, error: String) {
    self.images_state = LoadState::Error(error);
  }

  // Volumes
  pub fn set_volumes(&mut self, volumes: Vec<VolumeInfo>) {
    self.volumes = volumes;
    self.volumes_state = LoadState::Loaded;
  }

  #[allow(dead_code)]
  pub fn set_volumes_loading(&mut self) {
    self.volumes_state = LoadState::Loading;
  }

  #[allow(dead_code)]
  pub fn set_volumes_error(&mut self, error: String) {
    self.volumes_state = LoadState::Error(error);
  }

  // Networks
  pub fn set_networks(&mut self, networks: Vec<NetworkInfo>) {
    self.networks = networks;
    self.networks_state = LoadState::Loaded;
  }

  #[allow(dead_code)]
  pub fn set_networks_loading(&mut self) {
    self.networks_state = LoadState::Loading;
  }

  #[allow(dead_code)]
  pub fn set_networks_error(&mut self, error: String) {
    self.networks_state = LoadState::Error(error);
  }

  // Pods (Kubernetes)
  pub fn set_pods(&mut self, pods: Vec<PodInfo>) {
    self.pods = pods;
    self.pods_state = LoadState::Loaded;
  }

  pub fn set_pods_loading(&mut self) {
    self.pods_state = LoadState::Loading;
  }

  pub fn set_pods_error(&mut self, error: String) {
    self.pods_state = LoadState::Error(error);
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
    if available {
      self.k8s_error = None;
    }
  }

  pub fn set_k8s_error(&mut self, error: Option<String>) {
    self.k8s_error = error;
    if self.k8s_error.is_some() {
      self.k8s_available = false;
    }
  }

  // Services (Kubernetes)
  pub fn set_services(&mut self, services: Vec<ServiceInfo>) {
    self.services = services;
    self.services_state = LoadState::Loaded;
  }

  pub fn set_services_loading(&mut self) {
    self.services_state = LoadState::Loading;
  }

  pub fn set_services_error(&mut self, error: String) {
    self.services_state = LoadState::Error(error);
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
    self.deployments_state = LoadState::Loaded;
  }

  pub fn set_deployments_loading(&mut self) {
    self.deployments_state = LoadState::Loading;
  }

  pub fn set_deployments_error(&mut self, error: String) {
    self.deployments_state = LoadState::Error(error);
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
