use crate::colima::{ColimaVm, VmFileEntry, VmOsInfo};
use crate::docker::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo};
use crate::kubernetes::PodInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentView {
  #[default]
  Containers,
  Compose,
  Volumes,
  Images,
  Networks,
  Pods,
  Services,
  Deployments,
  Machines,
  ActivityMonitor,
  Settings,
}

impl CurrentView {
  pub fn title(&self) -> &'static str {
    match self {
      CurrentView::Containers => "Containers",
      CurrentView::Compose => "Compose",
      CurrentView::Volumes => "Volumes",
      CurrentView::Images => "Images",
      CurrentView::Networks => "Networks",
      CurrentView::Pods => "Pods",
      CurrentView::Services => "Services",
      CurrentView::Deployments => "Deployments",
      CurrentView::Machines => "Machines",
      CurrentView::ActivityMonitor => "Activity Monitor",
      CurrentView::Settings => "Settings",
    }
  }
}

#[derive(Debug, Clone)]
pub enum SelectedItem {
  Container(ContainerInfo),
  Image(ImageInfo),
  Volume(VolumeInfo),
  Network(NetworkInfo),
  Machine(ColimaVm),
  Pod(PodInfo),
}

/// State for machine-specific data (logs, files, terminal)
/// Type of logs to display for a machine
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MachineLogType {
  #[default]
  System,
  Docker,
  Containerd,
}

/// Sub-tab within the Info tab
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MachineInfoSubTab {
  #[default]
  Overview,
  Cpu,
  Memory,
  Disk,
}

#[derive(Debug, Default, Clone)]
pub struct MachineTabState {
  /// Logs content for the selected machine
  pub logs: String,
  /// Whether logs are currently loading
  pub logs_loading: bool,
  /// Type of logs being displayed
  pub log_type: MachineLogType,
  /// Current directory path in file browser
  pub current_path: String,
  /// Files in current directory
  pub files: Vec<VmFileEntry>,
  /// Whether files are currently loading
  pub files_loading: bool,
  /// Selected file path for viewing
  pub selected_file: Option<String>,
  /// Content of selected file
  pub file_content: String,
  /// Whether file content is loading
  pub file_content_loading: bool,
  /// OS information for the machine
  pub os_info: Option<VmOsInfo>,
  /// Real-time disk usage info
  pub disk_usage: String,
  /// Real-time memory info
  pub memory_info: String,
  /// Top processes running in VM
  pub processes: String,
  /// Whether stats are loading
  pub stats_loading: bool,
  /// Colima version
  pub colima_version: String,
  /// Active sub-tab within Info tab
  pub info_sub_tab: MachineInfoSubTab,
  /// CPU usage history for chart (last 30 samples)
  pub cpu_history: Vec<f64>,
  /// Memory usage history for chart (last 30 samples, in MB)
  pub memory_history: Vec<f64>,
  /// Disk usage percentage
  pub disk_usage_percent: f64,
  /// Memory usage percentage
  pub memory_usage_percent: f64,
}

impl MachineTabState {
  pub fn new() -> Self {
    Self {
      current_path: "/".to_string(),
      ..Default::default()
    }
  }
}

#[derive(Debug, Default)]
pub struct AppState {
  pub current_view: CurrentView,
  pub colima_vms: Vec<ColimaVm>,
  pub containers: Vec<ContainerInfo>,
  pub images: Vec<ImageInfo>,
  pub volumes: Vec<VolumeInfo>,
  pub networks: Vec<NetworkInfo>,
  pub selected_item: Option<SelectedItem>,
  pub is_loading: bool,
  pub is_creating_machine: bool,
  pub creating_machine_name: Option<String>,
  pub error_message: Option<String>,
  pub active_detail_tab: usize,
  /// State for the currently selected machine's tabs
  pub machine_tab_state: MachineTabState,
}

impl AppState {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn set_view(&mut self, view: CurrentView) {
    self.current_view = view;
    self.selected_item = None;
    self.active_detail_tab = 0;
  }

  pub fn set_active_tab(&mut self, tab: usize) {
    self.active_detail_tab = tab;
  }

  pub fn select_container(&mut self, container: ContainerInfo) {
    self.selected_item = Some(SelectedItem::Container(container));
    self.active_detail_tab = 0;
  }

  pub fn select_image(&mut self, image: ImageInfo) {
    self.selected_item = Some(SelectedItem::Image(image));
    self.active_detail_tab = 0;
  }

  pub fn select_volume(&mut self, volume: VolumeInfo) {
    self.selected_item = Some(SelectedItem::Volume(volume));
    self.active_detail_tab = 0;
  }

  pub fn select_network(&mut self, network: NetworkInfo) {
    self.selected_item = Some(SelectedItem::Network(network));
    self.active_detail_tab = 0;
  }

  pub fn select_machine(&mut self, machine: ColimaVm) {
    self.selected_item = Some(SelectedItem::Machine(machine));
    self.active_detail_tab = 0;
    self.machine_tab_state = MachineTabState::new();
  }

  pub fn clear_selection(&mut self) {
    self.selected_item = None;
  }
}
