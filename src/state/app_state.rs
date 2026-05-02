use crate::colima::{ColimaConfig, VmFileEntry, VmOsInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentView {
  #[default]
  Dashboard,
  Containers,
  Compose,
  Volumes,
  Images,
  Networks,
  Cluster,
  Workloads,
  Pods,
  Networking,
  Services,
  Ingresses,
  Deployments,
  StatefulSets,
  DaemonSets,
  Jobs,
  CronJobs,
  Config,
  Secrets,
  ConfigMaps,
  Pvcs,
  Machines,
  /// AI Models — only constructed on macOS aarch64 (sidebar + palette
  /// gated), but variant is always defined so match arms compile on
  /// every platform.
  #[cfg_attr(not(all(target_os = "macos", target_arch = "aarch64")), allow(dead_code))]
  Models,
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
  /// Machine configuration (for mounts, env, etc.)
  pub config: Option<ColimaConfig>,
  /// SSH config string
  pub ssh_config: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_current_view_default() {
    assert_eq!(CurrentView::default(), CurrentView::Dashboard);
  }

  #[test]
  fn test_current_view_equality() {
    assert_eq!(CurrentView::Containers, CurrentView::Containers);
    assert_ne!(CurrentView::Containers, CurrentView::Images);
    assert_ne!(CurrentView::Pods, CurrentView::Services);
  }

  #[test]
  fn test_current_view_all_variants() {
    // Ensure all views can be created
    let views = vec![
      CurrentView::Dashboard,
      CurrentView::Containers,
      CurrentView::Compose,
      CurrentView::Volumes,
      CurrentView::Images,
      CurrentView::Networks,
      CurrentView::Cluster,
      CurrentView::Workloads,
      CurrentView::Config,
      CurrentView::Pods,
      CurrentView::Networking,
      CurrentView::Services,
      CurrentView::Ingresses,
      CurrentView::Deployments,
      CurrentView::StatefulSets,
      CurrentView::DaemonSets,
      CurrentView::Jobs,
      CurrentView::CronJobs,
      CurrentView::Secrets,
      CurrentView::ConfigMaps,
      CurrentView::Pvcs,
      CurrentView::Machines,
      CurrentView::Models,
      CurrentView::ActivityMonitor,
      CurrentView::Settings,
    ];
    assert_eq!(views.len(), 25);
  }

  #[test]
  fn test_machine_log_type_default() {
    assert_eq!(MachineLogType::default(), MachineLogType::System);
  }

  #[test]
  fn test_machine_log_type_variants() {
    let types = [
      MachineLogType::System,
      MachineLogType::Docker,
      MachineLogType::Containerd,
    ];
    assert_eq!(types.len(), 3);

    // Verify they are distinct
    assert_ne!(MachineLogType::System, MachineLogType::Docker);
    assert_ne!(MachineLogType::Docker, MachineLogType::Containerd);
  }

  #[test]
  fn test_machine_tab_state_default() {
    let state = MachineTabState::default();
    assert!(state.logs.is_empty());
    assert!(!state.logs_loading);
    assert_eq!(state.log_type, MachineLogType::System);
    assert!(state.current_path.is_empty());
    assert!(state.files.is_empty());
    assert!(!state.files_loading);
    assert!(state.selected_file.is_none());
    assert!(state.file_content.is_empty());
    assert!(!state.file_content_loading);
    assert!(state.os_info.is_none());
    assert!(state.disk_usage.is_empty());
    assert!(state.memory_info.is_empty());
    assert!(state.processes.is_empty());
    assert!(!state.stats_loading);
    assert!(state.colima_version.is_empty());
    assert!(state.config.is_none());
    assert!(state.ssh_config.is_none());
  }

  #[test]
  fn test_machine_tab_state_with_values() {
    let state = MachineTabState {
      logs: "Some log content".to_string(),
      logs_loading: true,
      log_type: MachineLogType::Docker,
      current_path: "/var/log".to_string(),
      selected_file: Some("/var/log/syslog".to_string()),
      colima_version: "0.6.0".to_string(),
      ..Default::default()
    };

    assert_eq!(state.logs, "Some log content");
    assert!(state.logs_loading);
    assert_eq!(state.log_type, MachineLogType::Docker);
    assert_eq!(state.current_path, "/var/log");
    assert_eq!(state.selected_file, Some("/var/log/syslog".to_string()));
    assert_eq!(state.colima_version, "0.6.0");
  }
}
