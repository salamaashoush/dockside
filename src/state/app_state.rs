use crate::colima::{VmFileEntry, VmOsInfo};

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

/// Type of logs to display for a machine
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MachineLogType {
  #[default]
  System,
  Docker,
  Containerd,
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
}
