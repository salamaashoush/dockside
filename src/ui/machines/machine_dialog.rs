use std::rc::Rc;

use gpui::{
  App, Context, Entity, FocusHandle, Focusable, Hsla, ParentElement, Render, SharedString, Styled, Window, div,
  prelude::*, px,
};
use gpui_component::{
  IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  switch::Switch,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::colima::{
  ColimaConfig, ColimaVm, KubernetesConfig, MountConfig, MountType, NetworkConfig, NetworkMode, PortForwarder,
  ProvisionMode, ProvisionScript, VmArch, VmRuntime, VmType,
};

/// Tab indices for machine dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum MachineDialogTab {
  #[default]
  Basic = 0,
  Runtime = 1,
  Vm = 2,
  Storage = 3,
  Network = 4,
  Kubernetes = 5,
  Env = 6,
  Docker = 7,
  Provision = 8,
}

/// Mode for the machine dialog - Create new or Edit existing
#[derive(Clone)]
pub enum MachineDialogMode {
  Create,
  Edit(ColimaVm),
}

impl MachineDialogMode {
  pub fn is_edit(&self) -> bool {
    matches!(self, MachineDialogMode::Edit(_))
  }

  pub fn machine(&self) -> Option<&ColimaVm> {
    match self {
      MachineDialogMode::Create => None,
      MachineDialogMode::Edit(m) => Some(m),
    }
  }
}

/// Type alias for tab change callback
type TabChangeCallback = Rc<dyn Fn(&MachineDialogTab, &mut Window, &mut App)>;

/// Theme colors struct for passing to helper methods
#[derive(Clone)]
struct DialogColors {
  border: Hsla,
  foreground: Hsla,
  muted_foreground: Hsla,
  sidebar: Hsla,
}

/// Unified form state for creating or editing a Colima machine
pub struct MachineDialog {
  focus_handle: FocusHandle,
  mode: MachineDialogMode,
  active_tab: MachineDialogTab,

  // Basic inputs
  name_input: Option<Entity<InputState>>,
  cpus_input: Option<Entity<InputState>>,
  memory_input: Option<Entity<InputState>>,
  disk_input: Option<Entity<InputState>>,
  hostname_input: Option<Entity<InputState>>,

  // Selection state
  runtime: VmRuntime,
  vm_type: VmType,
  arch: VmArch,
  mount_type: MountType,
  network_mode: NetworkMode,
  port_forwarder: PortForwarder,

  // Boolean options
  kubernetes: bool,
  network_address: bool,
  network_host_addresses: bool,
  network_preferred_route: bool,
  rosetta: bool,
  ssh_agent: bool,
  ssh_config: bool,
  nested_virtualization: bool,
  binfmt: bool,
  mount_inotify: bool,
  activate: bool,

  // Advanced inputs
  cpu_type_input: Option<Entity<InputState>>,
  disk_image_input: Option<Entity<InputState>>,
  root_disk_input: Option<Entity<InputState>>,

  // Network inputs
  network_interface_input: Option<Entity<InputState>>,
  ssh_port_input: Option<Entity<InputState>>,
  dns_input: Option<Entity<InputState>>,
  dns_host_name_input: Option<Entity<InputState>>,
  dns_host_ip_input: Option<Entity<InputState>>,
  dns_servers: Vec<String>,
  dns_hosts: Vec<(String, String)>,

  // Mount configuration
  mount_location_input: Option<Entity<InputState>>,
  mount_writable: bool,
  mounts: Vec<MountConfig>,

  // Kubernetes inputs
  k8s_version_input: Option<Entity<InputState>>,
  k3s_args_input: Option<Entity<InputState>>,
  k3s_port_input: Option<Entity<InputState>>,

  // Environment variables
  env_key_input: Option<Entity<InputState>>,
  env_value_input: Option<Entity<InputState>>,
  env_vars: Vec<(String, String)>,

  // Docker engine settings
  docker_buildkit: bool,
  insecure_registry_input: Option<Entity<InputState>>,
  insecure_registries: Vec<String>,
  registry_mirror_input: Option<Entity<InputState>>,
  registry_mirrors: Vec<String>,

  // Provision scripts
  provision_script_input: Option<Entity<InputState>>,
  provision_mode: ProvisionMode,
  provision_scripts: Vec<ProvisionScript>,
}

impl MachineDialog {
  /// Create a new dialog for creating a machine
  pub fn new_create(cx: &mut Context<'_, Self>) -> Self {
    Self::new_with_mode(MachineDialogMode::Create, cx)
  }

  /// Create a new dialog for editing an existing machine
  pub fn new_edit(machine: ColimaVm, cx: &mut Context<'_, Self>) -> Self {
    Self::new_with_mode(MachineDialogMode::Edit(machine), cx)
  }

  fn new_with_mode(mode: MachineDialogMode, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    // Extract initial values from machine if editing
    let (runtime, vm_type, arch, mount_type, kubernetes, network_address, rosetta, ssh_agent) =
      if let Some(machine) = mode.machine() {
        (
          machine.runtime,
          machine.vm_type.unwrap_or(VmType::Vz),
          machine.arch,
          machine.mount_type.unwrap_or(MountType::Virtiofs),
          machine.kubernetes,
          machine.address.is_some(),
          machine.rosetta,
          machine.ssh_agent,
        )
      } else {
        (
          VmRuntime::Docker,
          VmType::default(),
          VmArch::default(),
          MountType::default(),
          false,
          false,
          false,
          false,
        )
      };

    Self {
      focus_handle,
      mode,
      active_tab: MachineDialogTab::Basic,
      name_input: None,
      cpus_input: None,
      memory_input: None,
      disk_input: None,
      hostname_input: None,
      runtime,
      vm_type,
      arch,
      mount_type,
      network_mode: NetworkMode::default(),
      port_forwarder: PortForwarder::default(),
      kubernetes,
      network_address,
      network_host_addresses: false,
      network_preferred_route: false,
      rosetta,
      ssh_agent,
      ssh_config: true,
      nested_virtualization: false,
      binfmt: true,
      mount_inotify: false,
      activate: true,
      cpu_type_input: None,
      disk_image_input: None,
      root_disk_input: None,
      network_interface_input: None,
      ssh_port_input: None,
      dns_input: None,
      dns_host_name_input: None,
      dns_host_ip_input: None,
      dns_servers: Vec::new(),
      dns_hosts: Vec::new(),
      mount_location_input: None,
      mount_writable: true,
      mounts: Vec::new(),
      k8s_version_input: None,
      k3s_args_input: None,
      k3s_port_input: None,
      env_key_input: None,
      env_value_input: None,
      env_vars: Vec::new(),
      docker_buildkit: true,
      insecure_registry_input: None,
      insecure_registries: Vec::new(),
      registry_mirror_input: None,
      registry_mirrors: Vec::new(),
      provision_script_input: None,
      provision_mode: ProvisionMode::System,
      provision_scripts: Vec::new(),
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let machine = self.mode.machine();

    // Name input (only for create mode)
    if self.name_input.is_none() {
      let default_name = machine.map_or_else(|| "default".to_string(), |m| m.name.clone());
      self.name_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Machine name");
        state.insert(&default_name, window, cx);
        state
      }));
    }

    // Default CPU/Mem/Disk pulled from app settings when creating a
    // brand new machine (no existing machine to clone from).
    let app = crate::state::settings_state(cx).read(cx).settings.clone();

    // CPUs input
    if self.cpus_input.is_none() {
      let default_cpus = machine.map_or_else(|| app.colima_default_cpus.to_string(), |m| m.cpus.to_string());
      self.cpus_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("CPUs");
        state.insert(&default_cpus, window, cx);
        state
      }));
    }

    // Memory input
    if self.memory_input.is_none() {
      let default_memory = machine.map_or_else(
        || app.colima_default_memory_gb.to_string(),
        |m| format!("{:.0}", m.memory_gb()),
      );
      self.memory_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Memory (GB)");
        state.insert(&default_memory, window, cx);
        state
      }));
    }

    // Disk input
    if self.disk_input.is_none() {
      let default_disk = machine.map_or_else(
        || app.colima_default_disk_gb.to_string(),
        |m| format!("{:.0}", m.disk_gb()),
      );
      self.disk_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Disk (GB)");
        state.insert(&default_disk, window, cx);
        state
      }));
    }

    // Hostname input
    if self.hostname_input.is_none() {
      self.hostname_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Hostname (optional)")));
    }

    // Advanced inputs
    if self.cpu_type_input.is_none() {
      self.cpu_type_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("CPU type (optional)")));
    }

    if self.disk_image_input.is_none() {
      self.disk_image_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Path to disk image")));
    }

    if self.root_disk_input.is_none() {
      self.root_disk_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Root disk (GB)");
        state.insert("20", window, cx);
        state
      }));
    }

    // Network inputs
    if self.network_interface_input.is_none() {
      self.network_interface_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Interface (e.g., en0)");
        state.insert("en0", window, cx);
        state
      }));
    }

    if self.ssh_port_input.is_none() {
      self.ssh_port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("SSH port (0 = auto)")));
    }

    if self.dns_input.is_none() {
      self.dns_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("DNS server (e.g., 8.8.8.8)")));
    }

    if self.dns_host_name_input.is_none() {
      self.dns_host_name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Hostname")));
    }

    if self.dns_host_ip_input.is_none() {
      self.dns_host_ip_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("IP address")));
    }

    // Mount inputs
    if self.mount_location_input.is_none() {
      self.mount_location_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Path (e.g., /Users)")));
    }

    // Kubernetes inputs
    if self.k8s_version_input.is_none() {
      self.k8s_version_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("K8s version (e.g., v1.28)")));
    }

    if self.k3s_args_input.is_none() {
      self.k3s_args_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("K3s args (space separated)");
        state.insert("--disable=traefik", window, cx);
        state
      }));
    }

    if self.k3s_port_input.is_none() {
      self.k3s_port_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("K3s port");
        state.insert("6443", window, cx);
        state
      }));
    }

    // Environment variable inputs
    if self.env_key_input.is_none() {
      self.env_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("KEY")));
    }

    if self.env_value_input.is_none() {
      self.env_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("value")));
    }

    // Docker engine inputs
    if self.insecure_registry_input.is_none() {
      self.insecure_registry_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("registry:port")));
    }

    if self.registry_mirror_input.is_none() {
      self.registry_mirror_input =
        Some(cx.new(|cx| InputState::new(window, cx).placeholder("https://mirror.example.com")));
    }

    // Provision script input (multiline with bash syntax)
    if self.provision_script_input.is_none() {
      self.provision_script_input = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .code_editor("bash")
          .placeholder("Enter your bash script here...")
      }));
    }
  }

  /// Get the profile name for this machine
  pub fn get_profile_name(&self, cx: &App) -> String {
    self
      .name_input
      .as_ref()
      .map_or_else(|| "default".to_string(), |s| s.read(cx).text().to_string())
  }

  /// Build a `ColimaConfig` from the dialog state
  pub fn get_config(&self, cx: &App) -> ColimaConfig {
    let cpus: u32 = self
      .cpus_input
      .as_ref()
      .map_or(2, |s| s.read(cx).text().to_string().parse().unwrap_or(2));
    let memory: u32 = self
      .memory_input
      .as_ref()
      .map_or(2, |s| s.read(cx).text().to_string().parse().unwrap_or(2));
    let disk: u32 = self
      .disk_input
      .as_ref()
      .map_or(100, |s| s.read(cx).text().to_string().parse().unwrap_or(100));

    let hostname = self
      .hostname_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let cpu_type = self
      .cpu_type_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty())
      .unwrap_or_else(|| "host".to_string());

    let disk_image = self
      .disk_image_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let root_disk: u32 = self
      .root_disk_input
      .as_ref()
      .and_then(|s| s.read(cx).text().to_string().parse().ok())
      .unwrap_or(20);

    let network_interface = self
      .network_interface_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty())
      .unwrap_or_else(|| "en0".to_string());

    let ssh_port: u32 = self
      .ssh_port_input
      .as_ref()
      .and_then(|s| s.read(cx).text().to_string().parse().ok())
      .unwrap_or(0);

    let kubernetes_version = self
      .k8s_version_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let k3s_args: Vec<String> = self.k3s_args_input.as_ref().map_or_else(
      || vec!["--disable=traefik".to_string()],
      |s| {
        s.read(cx)
          .text()
          .to_string()
          .split_whitespace()
          .map(String::from)
          .collect()
      },
    );

    let k3s_port: u32 = self
      .k3s_port_input
      .as_ref()
      .and_then(|s| s.read(cx).text().to_string().parse().ok())
      .unwrap_or(0);

    // Build DNS hosts map
    let dns_hosts: std::collections::HashMap<String, String> =
      self.dns_hosts.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    // Build env map
    let env: std::collections::HashMap<String, String> =
      self.env_vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    ColimaConfig {
      cpu: cpus,
      disk,
      memory,
      arch: self.arch,
      runtime: self.runtime,
      hostname,
      kubernetes: KubernetesConfig {
        enabled: self.kubernetes,
        version: kubernetes_version,
        k3s_args,
        port: k3s_port,
      },
      auto_activate: self.activate,
      network: NetworkConfig {
        address: self.network_address,
        mode: self.network_mode,
        interface: network_interface,
        preferred_route: self.network_preferred_route,
        dns: self.dns_servers.clone(),
        dns_hosts,
        host_addresses: self.network_host_addresses,
      },
      forward_agent: self.ssh_agent,
      docker: self.get_docker_config(),
      vm_type: self.vm_type,
      port_forwarder: self.port_forwarder,
      rosetta: self.rosetta,
      binfmt: self.binfmt,
      nested_virtualization: self.nested_virtualization,
      mount_type: self.mount_type,
      mount_inotify: self.mount_inotify,
      cpu_type,
      provision: self.provision_scripts.clone(),
      ssh_config: self.ssh_config,
      ssh_port,
      mounts: self.mounts.clone(),
      disk_image,
      root_disk,
      env,
    }
  }

  /// Get Docker engine configuration as JSON for the colima config
  pub fn get_docker_config(&self) -> serde_json::Value {
    let mut config = serde_json::Map::new();

    // BuildKit feature
    let mut features = serde_json::Map::new();
    features.insert("buildkit".to_string(), serde_json::Value::Bool(self.docker_buildkit));
    config.insert("features".to_string(), serde_json::Value::Object(features));

    // Insecure registries
    if !self.insecure_registries.is_empty() {
      config.insert(
        "insecure-registries".to_string(),
        serde_json::Value::Array(
          self
            .insecure_registries
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
        ),
      );
    }

    // Registry mirrors
    if !self.registry_mirrors.is_empty() {
      config.insert(
        "registry-mirrors".to_string(),
        serde_json::Value::Array(
          self
            .registry_mirrors
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
        ),
      );
    }

    serde_json::Value::Object(config)
  }

  fn render_form_row(label: &'static str, content: impl IntoElement, colors: &DialogColors) -> gpui::Div {
    h_flex()
      .w_full()
      .py(px(12.))
      .px(px(16.))
      .justify_between()
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(Label::new(label).text_color(colors.foreground))
      .child(content)
  }

  fn render_form_row_with_desc(
    label: &'static str,
    description: &'static str,
    content: impl IntoElement,
    colors: &DialogColors,
  ) -> gpui::Div {
    h_flex()
      .w_full()
      .py(px(12.))
      .px(px(16.))
      .justify_between()
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(
        v_flex()
          .gap(px(2.))
          .child(Label::new(label).text_color(colors.foreground))
          .child(div().text_xs().text_color(colors.muted_foreground).child(description)),
      )
      .child(content)
  }

  fn render_section_header(title: &'static str, colors: &DialogColors) -> gpui::Div {
    div()
      .w_full()
      .py(px(8.))
      .px(px(16.))
      .bg(colors.sidebar)
      .child(div().text_xs().text_color(colors.muted_foreground).child(title))
  }

  fn render_basic_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let name_input = self.name_input.clone().unwrap();
    let cpus_input = self.cpus_input.clone().unwrap();
    let memory_input = self.memory_input.clone().unwrap();
    let disk_input = self.disk_input.clone().unwrap();
    let hostname_input = self.hostname_input.clone().unwrap();
    let is_edit = self.mode.is_edit();
    let machine_name = self.mode.machine().map(|m| m.name.clone());
    let activate = self.activate;

    v_flex()
      .w_full()
      // Show name field only for create mode, or display machine name for edit
      .when(!is_edit, |el| {
        el.child(Self::render_form_row(
          "Name",
          div().w(px(200.)).child(Input::new(&name_input).small()),
          colors,
        ))
      })
      .when(is_edit, |el| {
        el.child(Self::render_form_row(
          "Machine",
          div()
            .text_sm()
            .text_color(colors.foreground)
            .child(machine_name.unwrap_or_default()),
          colors,
        ))
      })
      .child(Self::render_form_row_with_desc(
        "CPUs",
        "Number of CPU cores",
        div().w(px(100.)).child(Input::new(&cpus_input).small()),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Memory",
        "Memory in gigabytes",
        div().w(px(100.)).child(Input::new(&memory_input).small()),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Disk",
        if is_edit {
          "Disk size (can only increase)"
        } else {
          "Disk size in gigabytes"
        },
        div().w(px(100.)).child(Input::new(&disk_input).small()),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Hostname",
        "Custom VM hostname",
        div().w(px(200.)).child(Input::new(&hostname_input).small()),
        colors,
      ))
      .child(Self::render_section_header("Behavior", colors))
      .child(Self::render_form_row_with_desc(
        "Auto-activate",
        "Set as active Docker/K8s context on startup",
        Switch::new("activate")
          .checked(activate)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.activate = *checked;
            cx.notify();
          })),
        colors,
      ))
  }

  fn render_runtime_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let runtime = self.runtime;

    v_flex().w_full().child(Self::render_form_row_with_desc(
      "Container Runtime",
      "Engine for running containers",
      h_flex()
        .gap(px(4.))
        .child(
          Button::new("runtime-docker")
            .label("Docker")
            .small()
            .when(runtime == VmRuntime::Docker, ButtonVariants::primary)
            .when(runtime != VmRuntime::Docker, ButtonVariants::ghost)
            .on_click(cx.listener(|this, _ev, _window, cx| {
              this.runtime = VmRuntime::Docker;
              cx.notify();
            })),
        )
        .child(
          Button::new("runtime-containerd")
            .label("Containerd")
            .small()
            .when(runtime == VmRuntime::Containerd, ButtonVariants::primary)
            .when(runtime != VmRuntime::Containerd, ButtonVariants::ghost)
            .on_click(cx.listener(|this, _ev, _window, cx| {
              this.runtime = VmRuntime::Containerd;
              cx.notify();
            })),
        )
        .child(
          Button::new("runtime-incus")
            .label("Incus")
            .small()
            .when(runtime == VmRuntime::Incus, ButtonVariants::primary)
            .when(runtime != VmRuntime::Incus, ButtonVariants::ghost)
            .on_click(cx.listener(|this, _ev, _window, cx| {
              this.runtime = VmRuntime::Incus;
              cx.notify();
            })),
        ),
      colors,
    ))
  }

  fn render_virtualization_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let vm_type = self.vm_type;
    let arch = self.arch;
    let rosetta = self.rosetta;
    let nested = self.nested_virtualization;
    let binfmt = self.binfmt;
    let cpu_type_input = self.cpu_type_input.clone().unwrap();

    // Constraint flags
    let is_vz = vm_type == VmType::Vz;
    let is_qemu = vm_type == VmType::Qemu;

    v_flex()
      .w_full()
      .child(Self::render_form_row_with_desc(
        "VM Type",
        "Apple VZ (macOS 13+) or QEMU",
        h_flex()
          .gap(px(4.))
          .child(
            Button::new("vm-vz")
              .label("Apple VZ")
              .small()
              .when(is_vz, ButtonVariants::primary)
              .when(!is_vz, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.vm_type = VmType::Vz;
                // Auto-select best mount type for VZ
                this.mount_type = MountType::Virtiofs;
                cx.notify();
              })),
          )
          .child(
            Button::new("vm-qemu")
              .label("QEMU")
              .small()
              .when(is_qemu, ButtonVariants::primary)
              .when(!is_qemu, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.vm_type = VmType::Qemu;
                // Auto-select best mount type for QEMU
                if this.mount_type == MountType::Virtiofs {
                  this.mount_type = MountType::NineP;
                }
                // Disable VZ-only features
                this.rosetta = false;
                this.nested_virtualization = false;
                cx.notify();
              })),
          ),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Architecture",
        "CPU architecture for the VM",
        h_flex()
          .gap(px(4.))
          .child(
            Button::new("arch-aarch64")
              .label("ARM64")
              .small()
              .when(arch == VmArch::Aarch64, ButtonVariants::primary)
              .when(arch != VmArch::Aarch64, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.arch = VmArch::Aarch64;
                cx.notify();
              })),
          )
          .child(
            Button::new("arch-x86")
              .label("x86_64")
              .small()
              .when(arch == VmArch::X86_64, ButtonVariants::primary)
              .when(arch != VmArch::X86_64, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.arch = VmArch::X86_64;
                cx.notify();
              })),
          ),
        colors,
      ))
      // VZ-specific options section
      .when(is_vz, |el| {
        el.child(Self::render_section_header("Apple Virtualization Options", colors))
          .child(Self::render_form_row_with_desc(
            "Rosetta",
            "Intel binary emulation (Apple Silicon only)",
            Switch::new("rosetta")
              .checked(rosetta)
              .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                this.rosetta = *checked;
                cx.notify();
              })),
            colors,
          ))
          .child(Self::render_form_row_with_desc(
            "Nested Virtualization",
            "Run VMs inside VM (M3+ only)",
            Switch::new("nested")
              .checked(nested)
              .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                this.nested_virtualization = *checked;
                cx.notify();
              })),
            colors,
          ))
      })
      // QEMU-specific options section
      .when(is_qemu, |el| {
        el.child(Self::render_section_header("QEMU Options", colors))
          .child(Self::render_form_row_with_desc(
            "CPU Type",
            "QEMU CPU type (see qemu-system -cpu help)",
            div().w(px(150.)).child(Input::new(&cpu_type_input).small()),
            colors,
          ))
      })
      // Common options
      .child(Self::render_section_header("Emulation", colors))
      .child(Self::render_form_row_with_desc(
        "Binfmt",
        if rosetta {
          "Multi-arch support (no-op: Rosetta enabled)"
        } else {
          "Enable multi-arch binary support"
        },
        Switch::new("binfmt")
          .checked(binfmt)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.binfmt = *checked;
            cx.notify();
          })),
        colors,
      ))
  }

  fn render_storage_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let mount_type = self.mount_type;
    let mount_inotify = self.mount_inotify;
    let mount_writable = self.mount_writable;
    let disk_image_input = self.disk_image_input.clone().unwrap();
    let root_disk_input = self.root_disk_input.clone().unwrap();
    let mount_location_input = self.mount_location_input.clone().unwrap();
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    // Mount type constraints based on VM type
    let is_vz = self.vm_type == VmType::Vz;
    let virtiofs_available = is_vz;

    let mount_desc = if is_vz {
      "VirtioFS (fastest), SSHFS, or 9P"
    } else {
      "9P (recommended) or SSHFS for QEMU"
    };

    v_flex()
      .w_full()
      .child(Self::render_form_row_with_desc(
        "Mount Type",
        mount_desc,
        h_flex()
          .gap(px(4.))
          .when(virtiofs_available, |el| {
            el.child(
              Button::new("mount-virtiofs")
                .label("VirtioFS")
                .small()
                .when(mount_type == MountType::Virtiofs, ButtonVariants::primary)
                .when(mount_type != MountType::Virtiofs, ButtonVariants::ghost)
                .on_click(cx.listener(|this, _ev, _window, cx| {
                  this.mount_type = MountType::Virtiofs;
                  cx.notify();
                })),
            )
          })
          .child(
            Button::new("mount-sshfs")
              .label("SSHFS")
              .small()
              .when(mount_type == MountType::Sshfs, ButtonVariants::primary)
              .when(mount_type != MountType::Sshfs, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.mount_type = MountType::Sshfs;
                cx.notify();
              })),
          )
          .child(
            Button::new("mount-9p")
              .label("9P")
              .small()
              .when(mount_type == MountType::NineP, ButtonVariants::primary)
              .when(mount_type != MountType::NineP, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.mount_type = MountType::NineP;
                cx.notify();
              })),
          ),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Mount Inotify",
        "Propagate file change events (experimental)",
        Switch::new("mount-inotify")
          .checked(mount_inotify)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.mount_inotify = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_section_header("Disk", colors))
      .child(Self::render_form_row_with_desc(
        "Disk Image",
        "Path to existing disk image file",
        div().w(px(200.)).child(Input::new(&disk_image_input).small()),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Root Disk",
        "Size of root filesystem (GB)",
        div().w(px(100.)).child(Input::new(&root_disk_input).small()),
        colors,
      ))
      .child(Self::render_section_header("Mounts", colors))
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .items_center()
          .child(div().flex_1().child(Input::new(&mount_location_input).small()))
          .child(
            h_flex()
              .gap(px(4.))
              .items_center()
              .child(Label::new("Writable").text_color(muted_color).text_xs())
              .child(
                Switch::new("mount-writable")
                  .checked(mount_writable)
                  .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                    this.mount_writable = *checked;
                    cx.notify();
                  })),
              ),
          )
          .child(
            Button::new("add-mount")
              .icon(IconName::Plus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                let location = this
                  .mount_location_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !location.is_empty() {
                  this.mounts.push(MountConfig::new(location, this.mount_writable));
                  this.mount_location_input =
                    Some(cx.new(|cx| InputState::new(window, cx).placeholder("Path (e.g., /Users)")));
                  this.mount_writable = true;
                  cx.notify();
                }
              })),
          ),
      )
      .children(self.mounts.iter().enumerate().map(|(idx, mount)| {
        let rw_label = if mount.writable { " (rw)" } else { " (ro)" };
        h_flex()
          .w_full()
          .py(px(8.))
          .px(px(16.))
          .gap(px(8.))
          .items_center()
          .bg(sidebar_color)
          .child(
            div()
              .flex_1()
              .text_sm()
              .text_color(foreground_color)
              .child(format!("{}{}", mount.location, rw_label)),
          )
          .child(
            Button::new(SharedString::from(format!("remove-mount-{idx}")))
              .icon(IconName::Minus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.mounts.remove(idx);
                cx.notify();
              })),
          )
      }))
  }

  fn render_provision_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let provision_script_input = self.provision_script_input.clone().unwrap();
    let provision_mode = self.provision_mode;
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;
    let border_color = colors.border;

    v_flex()
      .w_full()
      .child(
        div()
          .px(px(16.))
          .py(px(12.))
          .text_xs()
          .text_color(muted_color)
          .child("Provision scripts run on VM startup. Scripts should be idempotent (safe to run multiple times)."),
      )
      // Script editor section
      .child(Self::render_section_header("Add Script", colors))
      .child(
        v_flex()
          .w_full()
          .py(px(8.))
          .px(px(16.))
          .gap(px(8.))
          // Mode selection
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(
                div()
                  .text_sm()
                  .text_color(foreground_color)
                  .child("Run as:"),
              )
              .child(
                h_flex()
                  .gap(px(4.))
                  .child(
                    Button::new("prov-system")
                      .label("System (root)")
                      .small()
                      .when(provision_mode == ProvisionMode::System, ButtonVariants::primary)
                      .when(provision_mode != ProvisionMode::System, ButtonVariants::ghost)
                      .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.provision_mode = ProvisionMode::System;
                        cx.notify();
                      })),
                  )
                  .child(
                    Button::new("prov-user")
                      .label("User")
                      .small()
                      .when(provision_mode == ProvisionMode::User, ButtonVariants::primary)
                      .when(provision_mode != ProvisionMode::User, ButtonVariants::ghost)
                      .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.provision_mode = ProvisionMode::User;
                        cx.notify();
                      })),
                  ),
              ),
          )
          // Script editor (multiline with bash highlighting)
          .child(
            div()
              .w_full()
              .h(px(120.))
              .border_1()
              .border_color(border_color)
              .rounded(px(4.))
              .overflow_hidden()
              .child(Input::new(&provision_script_input).w_full().h_full()),
          )
          // Add button
          .child(
            h_flex()
              .w_full()
              .justify_end()
              .child(
                Button::new("add-provision")
                  .label("Add Script")
                  .icon(IconName::Plus)
                  .small()
                  .primary()
                  .on_click(cx.listener(|this, _ev, window, cx| {
                    let script = this
                      .provision_script_input
                      .as_ref()
                      .map(|s| s.read(cx).text().to_string())
                      .unwrap_or_default();

                    if !script.is_empty() {
                      this.provision_scripts.push(ProvisionScript {
                        mode: this.provision_mode,
                        script,
                      });
                      // Reset input with multiline bash editor
                      this.provision_script_input = Some(cx.new(|cx| {
                        InputState::new(window, cx)
                          .multi_line(true)
                          .code_editor("bash")
                          .placeholder("Enter your bash script here...")
                      }));
                      cx.notify();
                    }
                  })),
              ),
          ),
      )
      // Existing scripts list
      .when(!self.provision_scripts.is_empty(), |el| {
        el.child(Self::render_section_header("Scripts", colors))
      })
      .children(self.provision_scripts.iter().enumerate().map(|(idx, prov)| {
        let mode_label = match prov.mode {
          ProvisionMode::System => "system (root)",
          ProvisionMode::User => "user",
        };
        let script_preview = if prov.script.len() > 80 {
          format!("{}...", &prov.script[..80])
        } else {
          prov.script.clone()
        };

        v_flex()
          .w_full()
          .py(px(8.))
          .px(px(16.))
          .gap(px(4.))
          .bg(sidebar_color)
          .border_b_1()
          .border_color(border_color)
          .child(
            h_flex()
              .w_full()
              .justify_between()
              .items_center()
              .child(
                div()
                  .text_xs()
                  .font_weight(gpui::FontWeight::SEMIBOLD)
                  .text_color(muted_color)
                  .child(format!("Script {} - {}", idx + 1, mode_label)),
              )
              .child(
                Button::new(SharedString::from(format!("remove-prov-{idx}")))
                  .icon(IconName::Minus)
                  .xsmall()
                  .ghost()
                  .on_click(cx.listener(move |this, _ev, _window, cx| {
                    this.provision_scripts.remove(idx);
                    cx.notify();
                  })),
              ),
          )
          .child(
            div()
              .w_full()
              .px(px(8.))
              .py(px(4.))
              .rounded(px(4.))
              .bg(colors.sidebar)
              .text_xs()
              .font_family("monospace")
              .text_color(foreground_color)
              .overflow_hidden()
              .child(script_preview),
          )
      }))
  }

  fn render_network_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let network_mode = self.network_mode;
    let port_forwarder = self.port_forwarder;
    let network_address = self.network_address;
    let network_host_addresses = self.network_host_addresses;
    let network_preferred_route = self.network_preferred_route;
    let ssh_agent = self.ssh_agent;
    let ssh_config = self.ssh_config;
    let network_interface_input = self.network_interface_input.clone().unwrap();
    let ssh_port_input = self.ssh_port_input.clone().unwrap();
    let dns_input = self.dns_input.clone().unwrap();
    let dns_host_name_input = self.dns_host_name_input.clone().unwrap();
    let dns_host_ip_input = self.dns_host_ip_input.clone().unwrap();
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    let is_bridged = network_mode == NetworkMode::Bridged;

    v_flex()
      .w_full()
      .child(Self::render_form_row_with_desc(
        "Network Mode",
        "Shared (NAT) or Bridged (direct)",
        h_flex()
          .gap(px(4.))
          .child(
            Button::new("net-shared")
              .label("Shared")
              .small()
              .when(network_mode == NetworkMode::Shared, ButtonVariants::primary)
              .when(network_mode != NetworkMode::Shared, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.network_mode = NetworkMode::Shared;
                cx.notify();
              })),
          )
          .child(
            Button::new("net-bridged")
              .label("Bridged")
              .small()
              .when(is_bridged, ButtonVariants::primary)
              .when(!is_bridged, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.network_mode = NetworkMode::Bridged;
                cx.notify();
              })),
          ),
        colors,
      ))
      .when(is_bridged, |el| {
        el.child(Self::render_form_row_with_desc(
          "Network Interface",
          "Host interface for bridged networking",
          div().w(px(150.)).child(Input::new(&network_interface_input).small()),
          colors,
        ))
      })
      .child(Self::render_form_row_with_desc(
        "Network Address",
        "Assign routable IP (macOS only)",
        Switch::new("network-address")
          .checked(network_address)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.network_address = *checked;
            if !*checked {
              this.network_preferred_route = false;
            }
            cx.notify();
          })),
        colors,
      ))
      .when(network_address, |el| {
        el.child(Self::render_form_row_with_desc(
          "Preferred Route",
          "Use assigned IP as preferred route",
          Switch::new("network-preferred-route")
            .checked(network_preferred_route)
            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
              this.network_preferred_route = *checked;
              cx.notify();
            })),
          colors,
        ))
      })
      .child(Self::render_form_row_with_desc(
        "Host Addresses",
        "Port forwarding to specific host IPs",
        Switch::new("network-host-addresses")
          .checked(network_host_addresses)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.network_host_addresses = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "Port Forwarder",
        "SSH (stable, TCP) or gRPC (TCP+UDP)",
        h_flex()
          .gap(px(4.))
          .child(
            Button::new("pf-ssh")
              .label("SSH")
              .small()
              .when(port_forwarder == PortForwarder::Ssh, ButtonVariants::primary)
              .when(port_forwarder != PortForwarder::Ssh, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.port_forwarder = PortForwarder::Ssh;
                cx.notify();
              })),
          )
          .child(
            Button::new("pf-grpc")
              .label("gRPC")
              .small()
              .when(port_forwarder == PortForwarder::Grpc, ButtonVariants::primary)
              .when(port_forwarder != PortForwarder::Grpc, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.port_forwarder = PortForwarder::Grpc;
                cx.notify();
              })),
          ),
        colors,
      ))
      .child(Self::render_section_header("SSH", colors))
      .child(Self::render_form_row_with_desc(
        "SSH Agent",
        "Forward SSH agent to VM",
        Switch::new("ssh-agent")
          .checked(ssh_agent)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.ssh_agent = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "SSH Config",
        "Generate SSH config in ~/.ssh/config",
        Switch::new("ssh-config")
          .checked(ssh_config)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.ssh_config = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row_with_desc(
        "SSH Port",
        "Custom SSH port (0 = auto)",
        div().w(px(100.)).child(Input::new(&ssh_port_input).small()),
        colors,
      ))
      .child(Self::render_section_header("DNS", colors))
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .items_center()
          .child(div().flex_1().child(Input::new(&dns_input).small()))
          .child(
            Button::new("add-dns")
              .icon(IconName::Plus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                let server = this
                  .dns_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !server.is_empty() {
                  this.dns_servers.push(server);
                  this.dns_input =
                    Some(cx.new(|cx| InputState::new(window, cx).placeholder("DNS server (e.g., 8.8.8.8)")));
                  cx.notify();
                }
              })),
          ),
      )
      .children(self.dns_servers.iter().enumerate().map(|(idx, server)| {
        h_flex()
          .w_full()
          .py(px(6.))
          .px(px(16.))
          .gap(px(8.))
          .items_center()
          .bg(sidebar_color)
          .child(
            div()
              .flex_1()
              .text_sm()
              .text_color(foreground_color)
              .child(server.clone()),
          )
          .child(
            Button::new(SharedString::from(format!("remove-dns-{idx}")))
              .icon(IconName::Minus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.dns_servers.remove(idx);
                cx.notify();
              })),
          )
      }))
      .child(Self::render_section_header("DNS Hosts", colors))
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .items_center()
          .child(div().w(px(150.)).child(Input::new(&dns_host_name_input).small()))
          .child(Label::new("->").text_color(muted_color))
          .child(div().flex_1().child(Input::new(&dns_host_ip_input).small()))
          .child(
            Button::new("add-dns-host")
              .icon(IconName::Plus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                let hostname = this
                  .dns_host_name_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();
                let ip = this
                  .dns_host_ip_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !hostname.is_empty() && !ip.is_empty() {
                  this.dns_hosts.push((hostname, ip));
                  this.dns_host_name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Hostname")));
                  this.dns_host_ip_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("IP address")));
                  cx.notify();
                }
              })),
          ),
      )
      .children(self.dns_hosts.iter().enumerate().map(|(idx, (hostname, ip))| {
        h_flex()
          .w_full()
          .py(px(6.))
          .px(px(16.))
          .gap(px(8.))
          .items_center()
          .bg(sidebar_color)
          .child(
            div()
              .w(px(150.))
              .text_sm()
              .text_color(foreground_color)
              .child(hostname.clone()),
          )
          .child(Label::new("->").text_color(muted_color))
          .child(div().flex_1().text_sm().text_color(foreground_color).child(ip.clone()))
          .child(
            Button::new(SharedString::from(format!("remove-dns-host-{idx}")))
              .icon(IconName::Minus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.dns_hosts.remove(idx);
                cx.notify();
              })),
          )
      }))
  }

  fn render_kubernetes_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let kubernetes = self.kubernetes;
    let k8s_version_input = self.k8s_version_input.clone().unwrap();
    let k3s_args_input = self.k3s_args_input.clone().unwrap();
    let k3s_port_input = self.k3s_port_input.clone().unwrap();
    let is_edit = self.mode.is_edit();

    v_flex()
      .w_full()
      .child(Self::render_form_row_with_desc(
        "Enable Kubernetes",
        "Install K3s Kubernetes distribution",
        Switch::new("kubernetes")
          .checked(kubernetes)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.kubernetes = *checked;
            cx.notify();
          })),
        colors,
      ))
      .when(kubernetes, |el| {
        el.child(Self::render_section_header("Kubernetes Options", colors))
          .child(Self::render_form_row_with_desc(
            "K8s Version",
            "Kubernetes version (e.g., v1.28.0+k3s1)",
            div().w(px(180.)).child(Input::new(&k8s_version_input).small()),
            colors,
          ))
          .child(Self::render_form_row_with_desc(
            "K3s Arguments",
            "Additional K3s server arguments",
            div().w(px(200.)).child(Input::new(&k3s_args_input).small()),
            colors,
          ))
          .child(Self::render_form_row_with_desc(
            "API Server Port",
            "K3s API server listen port",
            div().w(px(100.)).child(Input::new(&k3s_port_input).small()),
            colors,
          ))
      })
      .when(is_edit, |el| {
        el.child(
          div().w_full().p(px(16.)).child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("Note: Machine will be restarted with new settings."),
          ),
        )
      })
  }

  fn render_environment_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let env_key_input = self.env_key_input.clone().unwrap();
    let env_value_input = self.env_value_input.clone().unwrap();
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    v_flex()
      .w_full()
      .child(
        div()
          .w_full()
          .px(px(16.))
          .py(px(12.))
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Environment variables available inside the VM."),
      )
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .items_center()
          .child(div().w(px(120.)).child(Input::new(&env_key_input).small()))
          .child(Label::new("=").text_color(muted_color))
          .child(div().flex_1().child(Input::new(&env_value_input).small()))
          .child(
            Button::new("add-env")
              .icon(IconName::Plus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                let key = this
                  .env_key_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();
                let value = this
                  .env_value_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !key.is_empty() {
                  this.env_vars.push((key, value));
                  this.env_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("KEY")));
                  this.env_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("value")));
                  cx.notify();
                }
              })),
          ),
      )
      .children(self.env_vars.iter().enumerate().map(|(idx, (key, value))| {
        h_flex()
          .w_full()
          .py(px(8.))
          .px(px(16.))
          .gap(px(8.))
          .items_center()
          .bg(sidebar_color)
          .child(
            div()
              .w(px(120.))
              .text_sm()
              .text_color(foreground_color)
              .child(key.clone()),
          )
          .child(Label::new("=").text_color(muted_color))
          .child(
            div()
              .flex_1()
              .text_sm()
              .text_color(foreground_color)
              .child(value.clone()),
          )
          .child(
            Button::new(SharedString::from(format!("remove-env-{idx}")))
              .icon(IconName::Minus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.env_vars.remove(idx);
                cx.notify();
              })),
          )
      }))
  }

  fn render_docker_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let docker_buildkit = self.docker_buildkit;
    let insecure_registry_input = self.insecure_registry_input.clone().unwrap();
    let registry_mirror_input = self.registry_mirror_input.clone().unwrap();
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let is_docker = self.runtime == VmRuntime::Docker;

    v_flex()
      .w_full()
      .when(!is_docker, |el| {
        el.child(
          div()
            .w_full()
            .px(px(16.))
            .py(px(12.))
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Docker engine settings are only available when using Docker runtime."),
        )
      })
      .when(is_docker, |el| {
        el.child(
          div()
            .w_full()
            .px(px(16.))
            .py(px(12.))
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Configure Docker daemon settings (daemon.json)."),
        )
        .child(Self::render_section_header("Features", colors))
        .child(Self::render_form_row_with_desc(
          "BuildKit",
          "Enable BuildKit for improved builds",
          Switch::new("docker-buildkit")
            .checked(docker_buildkit)
            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
              this.docker_buildkit = *checked;
              cx.notify();
            })),
          colors,
        ))
        .child(Self::render_section_header("Insecure Registries", colors))
        .child(
          h_flex()
            .w_full()
            .gap(px(8.))
            .p(px(16.))
            .items_center()
            .child(div().flex_1().child(Input::new(&insecure_registry_input).small()))
            .child(
              Button::new("add-insecure-registry")
                .icon(IconName::Plus)
                .xsmall()
                .ghost()
                .on_click(cx.listener(|this, _ev, window, cx| {
                  let registry = this
                    .insecure_registry_input
                    .as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default();

                  if !registry.is_empty() {
                    this.insecure_registries.push(registry);
                    this.insecure_registry_input =
                      Some(cx.new(|cx| InputState::new(window, cx).placeholder("registry:port")));
                    cx.notify();
                  }
                })),
            ),
        )
        .children(self.insecure_registries.iter().enumerate().map(|(idx, registry)| {
          h_flex()
            .w_full()
            .py(px(6.))
            .px(px(16.))
            .gap(px(8.))
            .items_center()
            .bg(sidebar_color)
            .child(
              div()
                .flex_1()
                .text_sm()
                .text_color(foreground_color)
                .child(registry.clone()),
            )
            .child(
              Button::new(SharedString::from(format!("remove-insecure-{idx}")))
                .icon(IconName::Minus)
                .xsmall()
                .ghost()
                .on_click(cx.listener(move |this, _ev, _window, cx| {
                  this.insecure_registries.remove(idx);
                  cx.notify();
                })),
            )
        }))
        .child(Self::render_section_header("Registry Mirrors", colors))
        .child(
          h_flex()
            .w_full()
            .gap(px(8.))
            .p(px(16.))
            .items_center()
            .child(div().flex_1().child(Input::new(&registry_mirror_input).small()))
            .child(
              Button::new("add-registry-mirror")
                .icon(IconName::Plus)
                .xsmall()
                .ghost()
                .on_click(cx.listener(|this, _ev, window, cx| {
                  let mirror = this
                    .registry_mirror_input
                    .as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default();

                  if !mirror.is_empty() {
                    this.registry_mirrors.push(mirror);
                    this.registry_mirror_input =
                      Some(cx.new(|cx| InputState::new(window, cx).placeholder("https://mirror.example.com")));
                    cx.notify();
                  }
                })),
            ),
        )
        .children(self.registry_mirrors.iter().enumerate().map(|(idx, mirror)| {
          h_flex()
            .w_full()
            .py(px(6.))
            .px(px(16.))
            .gap(px(8.))
            .items_center()
            .bg(sidebar_color)
            .child(
              div()
                .flex_1()
                .text_sm()
                .text_color(foreground_color)
                .child(mirror.clone()),
            )
            .child(
              Button::new(SharedString::from(format!("remove-mirror-{idx}")))
                .icon(IconName::Minus)
                .xsmall()
                .ghost()
                .on_click(cx.listener(move |this, _ev, _window, cx| {
                  this.registry_mirrors.remove(idx);
                  cx.notify();
                })),
            )
        }))
      })
  }
}

impl Focusable for MachineDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for MachineDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);

    let theme_colors = cx.theme().colors;
    let colors = DialogColors {
      border: theme_colors.border,
      foreground: theme_colors.foreground,
      muted_foreground: theme_colors.muted_foreground,
      sidebar: theme_colors.sidebar,
    };

    let current_tab = self.active_tab;
    let mounts_count = self.mounts.len();
    let dns_count = self.dns_servers.len() + self.dns_hosts.len();
    let env_count = self.env_vars.len();
    let docker_count = self.insecure_registries.len() + self.registry_mirrors.len();
    let provision_count = self.provision_scripts.len();

    // Tab labels with counts
    let tab_labels: [(MachineDialogTab, String); 9] = [
      (MachineDialogTab::Basic, "Basic".to_string()),
      (MachineDialogTab::Runtime, "Runtime".to_string()),
      (MachineDialogTab::Vm, "VM".to_string()),
      (MachineDialogTab::Storage, format!("Storage ({mounts_count})")),
      (MachineDialogTab::Network, format!("Network ({dns_count})")),
      (MachineDialogTab::Kubernetes, "Kubernetes".to_string()),
      (MachineDialogTab::Env, format!("Env ({env_count})")),
      (MachineDialogTab::Docker, format!("Docker ({docker_count})")),
      (MachineDialogTab::Provision, format!("Provision ({provision_count})")),
    ];

    let on_tab_change: TabChangeCallback = Rc::new(cx.listener(|this, tab: &MachineDialogTab, _window, cx| {
      this.active_tab = *tab;
      cx.notify();
    }));

    v_flex()
      .w_full()
      .max_h(px(500.))
      // Tab bar
      .child(
        div()
          .w_full()
          .border_b_1()
          .border_color(colors.border)
          .child(TabBar::new("machine-dialog-tabs").children(tab_labels.iter().map(|(tab, label)| {
            let on_tab_change = on_tab_change.clone();
            let tab_value = *tab;
            Tab::new()
              .label(label.clone())
              .selected(current_tab == *tab)
              .on_click(move |_ev, window, cx| {
                on_tab_change(&tab_value, window, cx);
              })
          }))),
      )
      // Tab content
      .child(
        div()
          .flex_1()
          .overflow_y_scrollbar()
          .pb(px(60.)) // Extra padding at bottom so content isn't hidden by action buttons
          .when(current_tab == MachineDialogTab::Basic, |el| el.child(self.render_basic_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Runtime, |el| el.child(self.render_runtime_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Vm, |el| el.child(self.render_virtualization_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Storage, |el| el.child(self.render_storage_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Network, |el| el.child(self.render_network_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Kubernetes, |el| el.child(self.render_kubernetes_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Env, |el| el.child(self.render_environment_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Docker, |el| el.child(self.render_docker_tab(&colors, cx)))
          .when(current_tab == MachineDialogTab::Provision, |el| el.child(self.render_provision_tab(&colors, cx))),
      )
  }
}
