use gpui::{App, AppContext, Entity, EventEmitter, Global};

use crate::colima::{ColimaVm, Machine, MachineId};
use crate::docker::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo};
use crate::kubernetes::{
  ConfigMapInfo, CronJobInfo, DaemonSetInfo, DeploymentInfo, EventInfo, IngressInfo, JobInfo, NodeInfo, PodInfo,
  PvcInfo, SecretInfo, ServiceInfo, StatefulSetInfo,
};

use super::app_state::CurrentView;

use crate::docker::VolumeFileEntry;

/// Tab indices for machine detail view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum MachineDetailTab {
  #[default]
  Info = 0,
  Config = 1,
  Stats = 2,
  Processes = 3,
  Logs = 4,
  Terminal = 5,
  Files = 6,
}

impl MachineDetailTab {
  /// All tabs for Colima VMs
  pub const ALL: [MachineDetailTab; 7] = [
    MachineDetailTab::Info,
    MachineDetailTab::Config,
    MachineDetailTab::Stats,
    MachineDetailTab::Processes,
    MachineDetailTab::Logs,
    MachineDetailTab::Terminal,
    MachineDetailTab::Files,
  ];

  /// Tabs available for Host Docker (no SSH/VM, native processes only)
  pub const HOST_TABS: [MachineDetailTab; 3] = [
    MachineDetailTab::Info,
    MachineDetailTab::Stats,
    MachineDetailTab::Processes,
  ];

  pub fn label(self) -> &'static str {
    match self {
      MachineDetailTab::Info => "Info",
      MachineDetailTab::Config => "Config",
      MachineDetailTab::Stats => "Stats",
      MachineDetailTab::Processes => "Processes",
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
  Stats = 1,
  Logs = 2,
  Processes = 3,
  Terminal = 4,
  Files = 5,
  Inspect = 6,
}

impl ContainerDetailTab {
  pub const ALL: [ContainerDetailTab; 7] = [
    ContainerDetailTab::Info,
    ContainerDetailTab::Stats,
    ContainerDetailTab::Logs,
    ContainerDetailTab::Processes,
    ContainerDetailTab::Terminal,
    ContainerDetailTab::Files,
    ContainerDetailTab::Inspect,
  ];

  pub fn label(self) -> &'static str {
    match self {
      ContainerDetailTab::Info => "Info",
      ContainerDetailTab::Stats => "Stats",
      ContainerDetailTab::Logs => "Logs",
      ContainerDetailTab::Processes => "Processes",
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
  Machine(MachineId), // Machine identifier (Host or Colima VM)
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
  pub history: Vec<crate::docker::ImageHistoryEntry>,
  /// Latest Trivy scan result. `None` until the user triggers a scan.
  pub scan: Option<crate::docker::ScanSummary>,
  /// Set while a scan is running so the UI can show a spinner.
  pub scan_loading: bool,
  /// Last scan error, if any.
  pub scan_error: Option<String>,
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
  ImageScanStarted {
    image_id: String,
  },
  ImageScanCompleted {
    image_id: String,
    summary: crate::docker::ScanSummary,
  },
  ImageScanFailed {
    image_id: String,
    error: String,
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
    machine_id: MachineId,
    tab: MachineDetailTab,
  },
  /// Request to open edit dialog for a machine
  EditMachineRequest {
    machine_id: MachineId,
  },
  /// Runtime switched to a different machine
  RuntimeSwitched {
    #[allow(dead_code)]
    machine_id: MachineId,
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
  /// Request to open Host Docker configuration dialog
  ConfigureHostRequest,

  // Secrets
  SecretsUpdated,
  #[allow(dead_code)]
  SecretYamlLoaded {
    name: String,
    namespace: String,
    yaml: String,
  },
  SecretEntriesLoaded {
    name: String,
    namespace: String,
    entries: Vec<(String, String)>,
  },

  // StatefulSets / DaemonSets
  StatefulSetsUpdated,
  DaemonSetsUpdated,
  JobsUpdated,
  CronJobsUpdated,
  IngressesUpdated,
  PvcsUpdated,
  NodesUpdated,
  EventsUpdated,

  // ConfigMaps
  ConfigMapsUpdated,
  #[allow(dead_code)]
  ConfigMapYamlLoaded {
    name: String,
    namespace: String,
    yaml: String,
  },
  ConfigMapEntriesLoaded {
    name: String,
    namespace: String,
    entries: Vec<(String, String)>,
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

/// Global docker state - all views subscribe to this
pub struct DockerState {
  // Machine Data (Host + Colima VMs)
  /// All available machines (native Docker host and/or Colima VMs)
  pub machines: Vec<Machine>,
  /// Currently active machine for Docker operations
  pub active_machine: Option<MachineId>,

  // Docker Data
  pub containers: Vec<ContainerInfo>,
  pub images: Vec<ImageInfo>,
  pub volumes: Vec<VolumeInfo>,
  pub networks: Vec<NetworkInfo>,

  // Kubernetes Data
  pub pods: Vec<PodInfo>,
  pub services: Vec<ServiceInfo>,
  pub deployments: Vec<DeploymentInfo>,
  pub secrets: Vec<SecretInfo>,
  pub configmaps: Vec<ConfigMapInfo>,
  pub statefulsets: Vec<StatefulSetInfo>,
  pub daemonsets: Vec<DaemonSetInfo>,
  pub jobs: Vec<JobInfo>,
  pub cronjobs: Vec<CronJobInfo>,
  pub ingresses: Vec<IngressInfo>,
  pub pvcs: Vec<PvcInfo>,
  pub nodes: Vec<NodeInfo>,
  pub events: Vec<EventInfo>,
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
  /// IDs of containers ticked for bulk start/stop/restart/delete
  pub selected_container_ids: std::collections::HashSet<String>,

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
  pub secrets_state: LoadState,
  pub configmaps_state: LoadState,
  pub statefulsets_state: LoadState,
  pub daemonsets_state: LoadState,
  pub jobs_state: LoadState,
  pub cronjobs_state: LoadState,
  pub ingresses_state: LoadState,
  pub pvcs_state: LoadState,
  pub nodes_state: LoadState,
  pub events_state: LoadState,
  pub machines_state: LoadState,
}

impl DockerState {
  pub fn new() -> Self {
    Self {
      machines: Vec::new(),
      active_machine: None,
      containers: Vec::new(),
      images: Vec::new(),
      volumes: Vec::new(),
      networks: Vec::new(),
      pods: Vec::new(),
      services: Vec::new(),
      deployments: Vec::new(),
      secrets: Vec::new(),
      configmaps: Vec::new(),
      statefulsets: Vec::new(),
      daemonsets: Vec::new(),
      jobs: Vec::new(),
      cronjobs: Vec::new(),
      ingresses: Vec::new(),
      pvcs: Vec::new(),
      nodes: Vec::new(),
      events: Vec::new(),
      namespaces: vec!["default".to_string()],
      selected_namespace: "all".to_string(),
      k8s_available: false,
      k8s_error: None,
      current_view: CurrentView::default(),
      active_detail_tab: 0,
      selection: Selection::None,
      selected_container_ids: std::collections::HashSet::new(),
      is_loading: true,
      // Per-resource load states
      containers_state: LoadState::NotLoaded,
      images_state: LoadState::NotLoaded,
      volumes_state: LoadState::NotLoaded,
      networks_state: LoadState::NotLoaded,
      pods_state: LoadState::NotLoaded,
      services_state: LoadState::NotLoaded,
      secrets_state: LoadState::NotLoaded,
      configmaps_state: LoadState::NotLoaded,
      statefulsets_state: LoadState::NotLoaded,
      daemonsets_state: LoadState::NotLoaded,
      jobs_state: LoadState::NotLoaded,
      cronjobs_state: LoadState::NotLoaded,
      ingresses_state: LoadState::NotLoaded,
      pvcs_state: LoadState::NotLoaded,
      nodes_state: LoadState::NotLoaded,
      events_state: LoadState::NotLoaded,
      deployments_state: LoadState::NotLoaded,
      machines_state: LoadState::NotLoaded,
    }
  }

  // Selection management
  pub fn set_selection(&mut self, selection: Selection) {
    self.selection = selection;
  }

  pub fn toggle_bulk_container(&mut self, id: &str) {
    if !self.selected_container_ids.remove(id) {
      self.selected_container_ids.insert(id.to_string());
    }
  }

  pub fn clear_bulk_container(&mut self) {
    self.selected_container_ids.clear();
  }

  pub fn is_bulk_container_selected(&self, id: &str) -> bool {
    self.selected_container_ids.contains(id)
  }

  // Machines

  /// Set all machines (used during init with Host + Colima VMs)
  pub fn set_machines(&mut self, machines: Vec<Machine>) {
    self.machines = machines;
    self.machines_state = LoadState::Loaded;
  }

  /// Update only Colima VMs while preserving the Host machine
  /// This should be used when refreshing Colima VM list to avoid losing the Host
  pub fn set_colima_vms(&mut self, vms: Vec<ColimaVm>) {
    // Keep existing host machine
    let host = self.machines.iter().find(|m| m.is_host()).cloned();

    // Replace with new Colima VMs
    self.machines = vms.into_iter().map(Machine::Colima).collect();

    // Re-add host at beginning if it existed
    if let Some(h) = host {
      self.machines.insert(0, h);
    }

    self.machines_state = LoadState::Loaded;
  }

  /// Get only Colima VMs (for Colima-specific operations)
  pub fn colima_vms(&self) -> impl Iterator<Item = &ColimaVm> {
    self.machines.iter().filter_map(|m| m.as_colima())
  }

  /// Get the host machine if present
  pub fn host(&self) -> Option<&crate::docker::DockerHostInfo> {
    self.machines.iter().find_map(|m| m.as_host())
  }

  /// Find a machine by its ID
  pub fn get_machine(&self, id: &MachineId) -> Option<&Machine> {
    self.machines.iter().find(|m| m.id() == *id)
  }

  /// Find a machine by name (convenience method)
  #[allow(dead_code)]
  pub fn get_machine_by_name(&self, name: &str) -> Option<&Machine> {
    self.machines.iter().find(|m| m.name() == name)
  }

  /// Get the currently active machine
  #[allow(dead_code)]
  pub fn active(&self) -> Option<&Machine> {
    self.active_machine.as_ref().and_then(|id| self.get_machine(id))
  }

  /// Set the active machine
  pub fn set_active(&mut self, id: MachineId) {
    self.active_machine = Some(id);
  }

  /// Clear the active machine
  #[allow(dead_code)]
  pub fn clear_active(&mut self) {
    self.active_machine = None;
  }

  // Containers
  pub fn set_containers(&mut self, containers: Vec<ContainerInfo>) {
    self.containers = containers;
    self.containers_state = LoadState::Loaded;
  }

  // Images
  pub fn set_images(&mut self, images: Vec<ImageInfo>) {
    self.images = images;
    self.images_state = LoadState::Loaded;
  }

  // Volumes
  pub fn set_volumes(&mut self, volumes: Vec<VolumeInfo>) {
    self.volumes = volumes;
    self.volumes_state = LoadState::Loaded;
  }

  // Networks
  pub fn set_networks(&mut self, networks: Vec<NetworkInfo>) {
    self.networks = networks;
    self.networks_state = LoadState::Loaded;
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

#[cfg(test)]
mod tests {
  use super::super::app_state::CurrentView;
  use super::*;

  #[test]
  fn test_docker_state_initialization() {
    let state = DockerState::new();

    assert!(state.containers.is_empty());
    assert!(state.images.is_empty());
    assert!(state.volumes.is_empty());
    assert!(state.networks.is_empty());
    assert!(state.machines.is_empty());
    assert!(state.active_machine.is_none());
    assert!(matches!(state.selection, Selection::None));
    assert!(state.is_loading);
    assert!(!state.k8s_available);
  }

  #[test]
  fn test_docker_state_load_states() {
    let state = DockerState::new();

    // All states should start as NotLoaded
    assert!(matches!(state.containers_state, LoadState::NotLoaded));
    assert!(matches!(state.images_state, LoadState::NotLoaded));
    assert!(matches!(state.volumes_state, LoadState::NotLoaded));
    assert!(matches!(state.networks_state, LoadState::NotLoaded));
    assert!(matches!(state.pods_state, LoadState::NotLoaded));
    assert!(matches!(state.services_state, LoadState::NotLoaded));
    assert!(matches!(state.deployments_state, LoadState::NotLoaded));
    assert!(matches!(state.machines_state, LoadState::NotLoaded));
  }

  #[test]
  fn test_docker_state_selection() {
    let mut state = DockerState::new();

    // Initial selection is None
    assert!(matches!(state.selection, Selection::None));

    // Set volume selection
    state.set_selection(Selection::Volume("my-volume".to_string()));
    assert!(matches!(state.selection, Selection::Volume(_)));
    if let Selection::Volume(ref name) = state.selection {
      assert_eq!(name, "my-volume");
    }

    // Set network selection
    state.set_selection(Selection::Network("network-123".to_string()));
    assert!(matches!(state.selection, Selection::Network(_)));

    // Set machine selection (Host)
    state.set_selection(Selection::Machine(MachineId::Host));
    assert!(matches!(state.selection, Selection::Machine(MachineId::Host)));

    // Set machine selection (Colima)
    state.set_selection(Selection::Machine(MachineId::Colima("default".to_string())));
    assert!(matches!(state.selection, Selection::Machine(MachineId::Colima(_))));

    // Set pod selection
    state.set_selection(Selection::Pod {
      name: "my-pod".to_string(),
      namespace: "default".to_string(),
    });
    assert!(matches!(state.selection, Selection::Pod { .. }));

    // Clear selection
    state.set_selection(Selection::None);
    assert!(matches!(state.selection, Selection::None));
  }

  #[test]
  fn test_docker_state_kubernetes() {
    let mut state = DockerState::new();

    // Initially k8s should not be available
    assert!(!state.k8s_available);
    assert!(state.k8s_error.is_none());

    // Set k8s as available
    state.set_k8s_available(true);
    assert!(state.k8s_available);
    assert!(state.k8s_error.is_none());

    // Set k8s error - should mark as unavailable
    state.set_k8s_error(Some("Connection refused".to_string()));
    assert!(!state.k8s_available);
    assert_eq!(state.k8s_error, Some("Connection refused".to_string()));

    // Clear error by setting available
    state.set_k8s_available(true);
    assert!(state.k8s_available);
    assert!(state.k8s_error.is_none());
  }

  #[test]
  fn test_docker_state_namespaces() {
    let mut state = DockerState::new();

    // Default namespace should be "default"
    assert_eq!(state.selected_namespace, "default");
    assert_eq!(state.namespaces, vec!["default".to_string()]);

    // Set namespaces
    state.set_namespaces(vec![
      "default".to_string(),
      "kube-system".to_string(),
      "production".to_string(),
    ]);
    assert_eq!(state.namespaces.len(), 3);

    // Change selected namespace
    state.set_selected_namespace("production".to_string());
    assert_eq!(state.selected_namespace, "production");
  }

  #[test]
  fn test_docker_state_view_navigation() {
    let mut state = DockerState::new();

    // Set different views
    state.set_view(CurrentView::Containers);
    assert!(matches!(state.current_view, CurrentView::Containers));
    assert_eq!(state.active_detail_tab, 0); // Tab resets on view change

    state.active_detail_tab = 2;
    state.set_view(CurrentView::Images);
    assert!(matches!(state.current_view, CurrentView::Images));
    assert_eq!(state.active_detail_tab, 0); // Tab resets
  }

  #[test]
  fn test_load_state_enum() {
    let not_loaded = LoadState::NotLoaded;
    let loading = LoadState::Loading;
    let loaded = LoadState::Loaded;
    let error = LoadState::Error("Test error".to_string());

    assert!(matches!(not_loaded, LoadState::NotLoaded));
    assert!(matches!(loading, LoadState::Loading));
    assert!(matches!(loaded, LoadState::Loaded));
    assert!(matches!(error, LoadState::Error(_)));
  }

  #[test]
  fn test_container_detail_tab() {
    assert_eq!(ContainerDetailTab::ALL.len(), 6);
    assert_eq!(ContainerDetailTab::Info.label(), "Info");
    assert_eq!(ContainerDetailTab::Logs.label(), "Logs");
    assert_eq!(ContainerDetailTab::Processes.label(), "Processes");
    assert_eq!(ContainerDetailTab::Terminal.label(), "Terminal");
    assert_eq!(ContainerDetailTab::Files.label(), "Files");
    assert_eq!(ContainerDetailTab::Inspect.label(), "Inspect");
  }

  #[test]
  fn test_machine_detail_tab() {
    assert_eq!(MachineDetailTab::ALL.len(), 7);
    assert_eq!(MachineDetailTab::Info.label(), "Info");
    assert_eq!(MachineDetailTab::Config.label(), "Config");
    assert_eq!(MachineDetailTab::Stats.label(), "Stats");
    assert_eq!(MachineDetailTab::Processes.label(), "Processes");
    assert_eq!(MachineDetailTab::Logs.label(), "Logs");
    assert_eq!(MachineDetailTab::Terminal.label(), "Terminal");
    assert_eq!(MachineDetailTab::Files.label(), "Files");
  }

  #[test]
  fn test_pod_detail_tab() {
    assert_eq!(PodDetailTab::ALL.len(), 5);
    assert_eq!(PodDetailTab::Info.label(), "Info");
    assert_eq!(PodDetailTab::Logs.label(), "Logs");
    assert_eq!(PodDetailTab::Terminal.label(), "Terminal");
    assert_eq!(PodDetailTab::Describe.label(), "Describe");
    assert_eq!(PodDetailTab::Yaml.label(), "YAML");
  }
}
