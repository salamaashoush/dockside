use gpui::{
  App, Context, Entity, FocusHandle, Focusable, Hsla, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
  Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  switch::Switch,
  theme::ActiveTheme,
  v_flex,
};

use crate::colima::{ColimaStartOptions, MountType, VmArch, VmRuntime, VmType};

/// Form state for creating a new Colima machine
pub struct CreateMachineDialog {
  focus_handle: FocusHandle,

  // Input states - created lazily
  name_input: Option<Entity<InputState>>,
  cpus_input: Option<Entity<InputState>>,
  memory_input: Option<Entity<InputState>>,
  disk_input: Option<Entity<InputState>>,

  // Selection state
  runtime: VmRuntime,
  vm_type: VmType,
  arch: VmArch,
  mount_type: MountType,
  kubernetes: bool,
  network_address: bool,
}

impl CreateMachineDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      name_input: None,
      cpus_input: None,
      memory_input: None,
      disk_input: None,
      runtime: VmRuntime::Docker,
      vm_type: VmType::default(),
      arch: VmArch::default(),
      mount_type: MountType::default(),
      kubernetes: false,
      network_address: false,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.name_input.is_none() {
      self.name_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Machine name");
        state.insert("default", window, cx);
        state
      }));
    }

    if self.cpus_input.is_none() {
      self.cpus_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("CPUs");
        state.insert("2", window, cx);
        state
      }));
    }

    if self.memory_input.is_none() {
      self.memory_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Memory (GB)");
        state.insert("2", window, cx);
        state
      }));
    }

    if self.disk_input.is_none() {
      self.disk_input = Some(cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Disk (GB)");
        state.insert("60", window, cx);
        state
      }));
    }
  }

  pub fn get_options(&self, cx: &App) -> ColimaStartOptions {
    let name = self
      .name_input
      .as_ref()
      .map_or_else(|| "default".to_string(), |s| s.read(cx).text().to_string());
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
      .map_or(60, |s| s.read(cx).text().to_string().parse().unwrap_or(60));

    ColimaStartOptions::new()
      .with_name(name)
      .with_cpus(cpus)
      .with_memory_gb(memory)
      .with_disk_gb(disk)
      .with_runtime(self.runtime)
      .with_vm_type(self.vm_type)
      .with_arch(self.arch)
      .with_mount_type(self.mount_type)
      .with_kubernetes(self.kubernetes)
      .with_network_address(self.network_address)
  }
}

impl Focusable for CreateMachineDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CreateMachineDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Ensure inputs are created
    self.ensure_inputs(window, cx);
    let colors = cx.theme().colors;

    // Clone state for closures
    let runtime = self.runtime;
    let vm_type = self.vm_type;
    let arch = self.arch;
    let mount_type = self.mount_type;
    let kubernetes = self.kubernetes;
    let network_address = self.network_address;

    let name_input = self.name_input.clone().unwrap();
    let cpus_input = self.cpus_input.clone().unwrap();
    let memory_input = self.memory_input.clone().unwrap();
    let disk_input = self.disk_input.clone().unwrap();

    // Helper to render form row
    let render_form_row = |label: &'static str, content: gpui::AnyElement, border: Hsla, fg: Hsla| {
      h_flex()
        .w_full()
        .py(px(12.))
        .px(px(16.))
        .justify_between()
        .items_center()
        .border_b_1()
        .border_color(border)
        .child(Label::new(label).text_color(fg))
        .child(content)
    };

    // Helper to render section header
    let render_section_header = |title: &'static str, bg: Hsla, muted: Hsla| {
      div()
        .w_full()
        .py(px(8.))
        .px(px(16.))
        .bg(bg)
        .child(div().text_xs().text_color(muted).child(title))
    };

    v_flex()
            .w_full()
            .max_h(px(500.))
            .overflow_y_scrollbar()
            // Name row
            .child(render_form_row(
                "Name",
                div().w(px(200.)).child(Input::new(&name_input).small()).into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // CPUs row
            .child(render_form_row(
                "CPUs",
                div().w(px(100.)).child(Input::new(&cpus_input).small()).into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Memory row
            .child(render_form_row(
                "Memory (GB)",
                div().w(px(100.)).child(Input::new(&memory_input).small()).into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Disk row
            .child(render_form_row(
                "Disk (GB)",
                div().w(px(100.)).child(Input::new(&disk_input).small()).into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Runtime section
            .child(render_section_header("Runtime", colors.sidebar, colors.muted_foreground))
            .child(render_form_row(
                "Container Runtime",
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
                            }))
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
                            }))
                    )
                    .into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // VM Type section
            .child(render_section_header("Virtualization", colors.sidebar, colors.muted_foreground))
            .child(render_form_row(
                "VM Type",
                h_flex()
                    .gap(px(4.))
                    .child(
                        Button::new("vm-qemu")
                            .label("QEMU")
                            .small()
                            .when(vm_type == VmType::Qemu, ButtonVariants::primary)
                            .when(vm_type != VmType::Qemu, ButtonVariants::ghost)
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.vm_type = VmType::Qemu;
                                cx.notify();
                            }))
                    )
                    .child(
                        Button::new("vm-vz")
                            .label("Apple VZ")
                            .small()
                            .when(vm_type == VmType::Vz, ButtonVariants::primary)
                            .when(vm_type != VmType::Vz, ButtonVariants::ghost)
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.vm_type = VmType::Vz;
                                cx.notify();
                            }))
                    )
                    .into_any_element(),
                colors.border,
                colors.foreground,
            ))
            .child(render_form_row(
                "Architecture",
                h_flex()
                    .gap(px(4.))
                    .child(
                        Button::new("arch-aarch64")
                            .label("arm64")
                            .small()
                            .when(arch == VmArch::Aarch64, ButtonVariants::primary)
                            .when(arch != VmArch::Aarch64, ButtonVariants::ghost)
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.arch = VmArch::Aarch64;
                                cx.notify();
                            }))
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
                            }))
                    )
                    .into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Mount section
            .child(render_section_header("Storage", colors.sidebar, colors.muted_foreground))
            .child(render_form_row(
                "Mount Type",
                h_flex()
                    .gap(px(4.))
                    .child(
                        Button::new("mount-sshfs")
                            .label("SSHFS")
                            .small()
                            .when(mount_type == MountType::Sshfs, ButtonVariants::primary)
                            .when(mount_type != MountType::Sshfs, ButtonVariants::ghost)
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.mount_type = MountType::Sshfs;
                                cx.notify();
                            }))
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
                            }))
                    )
                    .child(
                        Button::new("mount-virtiofs")
                            .label("VirtioFS")
                            .small()
                            .when(mount_type == MountType::Virtiofs, ButtonVariants::primary)
                            .when(mount_type != MountType::Virtiofs, ButtonVariants::ghost)
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.mount_type = MountType::Virtiofs;
                                cx.notify();
                            }))
                    )
                    .into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Options section
            .child(render_section_header("Options", colors.sidebar, colors.muted_foreground))
            .child(render_form_row(
                "Kubernetes",
                Switch::new("kubernetes")
                    .checked(kubernetes)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.kubernetes = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
            ))
            .child(render_form_row(
                "Network Address",
                Switch::new("network-address")
                    .checked(network_address)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.network_address = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
            ))
  }
}
