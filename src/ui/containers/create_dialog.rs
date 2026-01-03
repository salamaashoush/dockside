use gpui::{
  App, Context, Entity, FocusHandle, Focusable, Hsla, ParentElement, Render, SharedString, Styled, Window, div,
  prelude::*, px,
};
use gpui_component::{
  IconName, IndexPath, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  select::{Select, SelectItem, SelectState},
  switch::Switch,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

/// Theme colors struct for passing to helper methods
#[derive(Clone)]
struct DialogColors {
  border: Hsla,
  foreground: Hsla,
  muted_foreground: Hsla,
  sidebar: Hsla,
  link: Hsla,
}

/// Platform options for container
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
  #[default]
  Auto,
  LinuxAmd64,
  LinuxArm64,
  LinuxArmV7,
}

impl Platform {
  pub fn label(&self) -> &'static str {
    match self {
      Platform::Auto => "auto",
      Platform::LinuxAmd64 => "linux/amd64",
      Platform::LinuxArm64 => "linux/arm64",
      Platform::LinuxArmV7 => "linux/arm/v7",
    }
  }

  pub fn as_docker_arg(&self) -> Option<&'static str> {
    match self {
      Platform::Auto => None,
      Platform::LinuxAmd64 => Some("linux/amd64"),
      Platform::LinuxArm64 => Some("linux/arm64"),
      Platform::LinuxArmV7 => Some("linux/arm/v7"),
    }
  }

  pub fn all() -> Vec<Platform> {
    vec![
      Platform::Auto,
      Platform::LinuxAmd64,
      Platform::LinuxArm64,
      Platform::LinuxArmV7,
    ]
  }
}

impl SelectItem for Platform {
  type Value = Platform;

  fn title(&self) -> SharedString {
    self.label().into()
  }

  fn value(&self) -> &Self::Value {
    self
  }
}

/// Restart policy options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestartPolicy {
  #[default]
  No,
  Always,
  OnFailure,
  UnlessStopped,
}

impl RestartPolicy {
  pub fn label(&self) -> &'static str {
    match self {
      RestartPolicy::No => "no",
      RestartPolicy::Always => "always",
      RestartPolicy::OnFailure => "on-failure",
      RestartPolicy::UnlessStopped => "unless-stopped",
    }
  }

  pub fn as_docker_arg(&self) -> Option<&'static str> {
    match self {
      RestartPolicy::No => None,
      RestartPolicy::Always => Some("always"),
      RestartPolicy::OnFailure => Some("on-failure"),
      RestartPolicy::UnlessStopped => Some("unless-stopped"),
    }
  }

  pub fn all() -> Vec<RestartPolicy> {
    vec![
      RestartPolicy::No,
      RestartPolicy::Always,
      RestartPolicy::OnFailure,
      RestartPolicy::UnlessStopped,
    ]
  }
}

impl SelectItem for RestartPolicy {
  type Value = RestartPolicy;

  fn title(&self) -> SharedString {
    self.label().into()
  }

  fn value(&self) -> &Self::Value {
    self
  }
}

/// Port mapping configuration
#[derive(Debug, Clone, Default)]
pub struct PortMapping {
  pub host_port: String,
  pub container_port: String,
  pub protocol: String, // tcp or udp
}

/// Volume mount configuration
#[derive(Debug, Clone, Default)]
pub struct VolumeMount {
  pub host_path: String,
  pub container_path: String,
  pub read_only: bool,
}

/// Environment variable
#[derive(Debug, Clone, Default)]
pub struct EnvVar {
  pub key: String,
  pub value: String,
}

/// Options for creating a new container
#[derive(Debug, Clone, Default)]
pub struct CreateContainerOptions {
  pub image: String,
  pub platform: Platform,
  pub name: Option<String>,
  pub remove_after_stop: bool,
  pub restart_policy: RestartPolicy,
  pub command: Option<String>,
  pub entrypoint: Option<String>,
  pub workdir: Option<String>,
  pub privileged: bool,
  pub read_only: bool,
  pub docker_init: bool,
  pub start_after_create: bool,
  // New fields
  pub env_vars: Vec<(String, String)>,
  pub ports: Vec<(String, String, String)>, // (host_port, container_port, protocol)
  pub volumes: Vec<(String, String, bool)>, // (host_path, container_path, read_only)
  pub network: Option<String>,
}

/// Dialog for creating a new container
pub struct CreateContainerDialog {
  focus_handle: FocusHandle,
  active_tab: usize,

  // Input states - created lazily
  image_input: Option<Entity<InputState>>,
  name_input: Option<Entity<InputState>>,
  command_input: Option<Entity<InputState>>,
  entrypoint_input: Option<Entity<InputState>>,
  workdir_input: Option<Entity<InputState>>,

  // Select states
  platform_select: Option<Entity<SelectState<Vec<Platform>>>>,
  restart_policy_select: Option<Entity<SelectState<Vec<RestartPolicy>>>>,

  // Toggle state
  remove_after_stop: bool,
  privileged: bool,
  read_only: bool,
  docker_init: bool,

  // Environment variables
  env_vars: Vec<EnvVar>,
  env_key_input: Option<Entity<InputState>>,
  env_value_input: Option<Entity<InputState>>,

  // Port mappings
  ports: Vec<PortMapping>,
  port_host_input: Option<Entity<InputState>>,
  port_container_input: Option<Entity<InputState>>,
  port_protocol_tcp: bool,

  // Volume mounts
  volumes: Vec<VolumeMount>,
  volume_host_input: Option<Entity<InputState>>,
  volume_container_input: Option<Entity<InputState>>,
  volume_readonly: bool,

  // Network
  network_input: Option<Entity<InputState>>,
}

impl CreateContainerDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      active_tab: 0,
      image_input: None,
      name_input: None,
      command_input: None,
      entrypoint_input: None,
      workdir_input: None,
      platform_select: None,
      restart_policy_select: None,
      remove_after_stop: false,
      privileged: false,
      read_only: false,
      docker_init: false,
      env_vars: Vec::new(),
      env_key_input: None,
      env_value_input: None,
      ports: Vec::new(),
      port_host_input: None,
      port_container_input: None,
      port_protocol_tcp: true,
      volumes: Vec::new(),
      volume_host_input: None,
      volume_container_input: None,
      volume_readonly: false,
      network_input: None,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.image_input.is_none() {
      self.image_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. nginx:latest")));
    }

    if self.name_input.is_none() {
      self.name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Container name (optional)")));
    }

    if self.command_input.is_none() {
      self.command_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Command (optional)")));
    }

    if self.entrypoint_input.is_none() {
      self.entrypoint_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Entrypoint (optional)")));
    }

    if self.workdir_input.is_none() {
      self.workdir_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Working directory (optional)")));
    }

    if self.platform_select.is_none() {
      self.platform_select = Some(cx.new(|cx| SelectState::new(Platform::all(), Some(IndexPath::new(0)), window, cx)));
    }

    if self.restart_policy_select.is_none() {
      self.restart_policy_select =
        Some(cx.new(|cx| SelectState::new(RestartPolicy::all(), Some(IndexPath::new(0)), window, cx)));
    }

    // Env var inputs
    if self.env_key_input.is_none() {
      self.env_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("KEY")));
    }
    if self.env_value_input.is_none() {
      self.env_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("VALUE")));
    }

    // Port inputs
    if self.port_host_input.is_none() {
      self.port_host_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Host port")));
    }
    if self.port_container_input.is_none() {
      self.port_container_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Container port")));
    }

    // Volume inputs
    if self.volume_host_input.is_none() {
      self.volume_host_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Host path or volume name")));
    }
    if self.volume_container_input.is_none() {
      self.volume_container_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Container path")));
    }

    // Network input
    if self.network_input.is_none() {
      self.network_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Network name (optional)")));
    }
  }

  pub fn get_options(&self, cx: &App, start_after_create: bool) -> CreateContainerOptions {
    let image = self
      .image_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let name = self
      .name_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty());

    let command = self
      .command_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty());

    let entrypoint = self
      .entrypoint_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty());

    let workdir = self
      .workdir_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty());

    let platform = self
      .platform_select
      .as_ref()
      .and_then(|s| s.read(cx).selected_value().copied())
      .unwrap_or_default();

    let restart_policy = self
      .restart_policy_select
      .as_ref()
      .and_then(|s| s.read(cx).selected_value().copied())
      .unwrap_or_default();

    let network = self
      .network_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty());

    // Collect env vars
    let env_vars: Vec<(String, String)> = self
      .env_vars
      .iter()
      .filter(|e| !e.key.is_empty())
      .map(|e| (e.key.clone(), e.value.clone()))
      .collect();

    // Collect ports
    let ports: Vec<(String, String, String)> = self
      .ports
      .iter()
      .filter(|p| !p.container_port.is_empty())
      .map(|p| (p.host_port.clone(), p.container_port.clone(), p.protocol.clone()))
      .collect();

    // Collect volumes
    let volumes: Vec<(String, String, bool)> = self
      .volumes
      .iter()
      .filter(|v| !v.host_path.is_empty() && !v.container_path.is_empty())
      .map(|v| (v.host_path.clone(), v.container_path.clone(), v.read_only))
      .collect();

    CreateContainerOptions {
      image,
      platform,
      name,
      remove_after_stop: self.remove_after_stop,
      restart_policy,
      command,
      entrypoint,
      workdir,
      privileged: self.privileged,
      read_only: self.read_only,
      docker_init: self.docker_init,
      start_after_create,
      env_vars,
      ports,
      volumes,
      network,
    }
  }

  fn render_form_row(&self, label: &'static str, content: impl IntoElement, colors: &DialogColors) -> gpui::Div {
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
    &self,
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

  fn render_section_header(&self, title: &'static str, colors: &DialogColors) -> gpui::Div {
    div()
      .w_full()
      .py(px(8.))
      .px(px(16.))
      .bg(colors.sidebar)
      .child(div().text_xs().text_color(colors.muted_foreground).child(title))
  }

  fn render_general_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let remove_after_stop = self.remove_after_stop;
    let privileged = self.privileged;
    let read_only = self.read_only;
    let docker_init = self.docker_init;

    let image_input = self.image_input.clone().unwrap();
    let name_input = self.name_input.clone().unwrap();
    let command_input = self.command_input.clone().unwrap();
    let entrypoint_input = self.entrypoint_input.clone().unwrap();
    let workdir_input = self.workdir_input.clone().unwrap();
    let platform_select = self.platform_select.clone().unwrap();
    let restart_policy_select = self.restart_policy_select.clone().unwrap();

    v_flex()
            .w_full()
            // Image row (required)
            .child(self.render_form_row(
                "Image",
                div().w(px(250.)).child(Input::new(&image_input).small()),
                colors,
            ))
            // Name row
            .child(self.render_form_row(
                "Name",
                div().w(px(250.)).child(Input::new(&name_input).small()),
                colors,
            ))
            // Platform
            .child(self.render_form_row_with_desc(
                "Platform",
                "Target platform for the container",
                div().w(px(150.)).child(Select::new(&platform_select).small()),
                colors,
            ))
            // Remove after stop
            .child(self.render_form_row_with_desc(
                "Remove after stop",
                "Automatically delete after stop (--rm)",
                Switch::new("remove-after-stop")
                    .checked(remove_after_stop)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.remove_after_stop = *checked;
                        cx.notify();
                    })),
                colors,
            ))
            // Restart policy
            .child(self.render_form_row_with_desc(
                "Restart policy",
                "When to restart the container",
                div().w(px(150.)).child(Select::new(&restart_policy_select).small()),
                colors,
            ))
            // Command section
            .child(self.render_section_header("Command", colors))
            .child(self.render_form_row(
                "Command",
                div().w(px(250.)).child(Input::new(&command_input).small()),
                colors,
            ))
            .child(self.render_form_row(
                "Entrypoint",
                div().w(px(250.)).child(Input::new(&entrypoint_input).small()),
                colors,
            ))
            .child(self.render_form_row(
                "Working dir",
                div().w(px(250.)).child(Input::new(&workdir_input).small()),
                colors,
            ))
            // Advanced section
            .child(self.render_section_header("Advanced", colors))
            .child(self.render_form_row_with_desc(
                "Privileged",
                "Full access to host (--privileged)",
                Switch::new("privileged")
                    .checked(privileged)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.privileged = *checked;
                        cx.notify();
                    })),
                colors,
            ))
            .child(self.render_form_row_with_desc(
                "Read-only",
                "Read-only root filesystem (--read-only)",
                Switch::new("read-only")
                    .checked(read_only)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.read_only = *checked;
                        cx.notify();
                    })),
                colors,
            ))
            .child(self.render_form_row_with_desc(
                "Docker init",
                "Use docker-init process (--init)",
                Switch::new("docker-init")
                    .checked(docker_init)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.docker_init = *checked;
                        cx.notify();
                    })),
                colors,
            ))
  }

  fn render_ports_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let port_host_input = self.port_host_input.clone().unwrap();
    let port_container_input = self.port_container_input.clone().unwrap();
    let port_protocol_tcp = self.port_protocol_tcp;
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    v_flex()
            .w_full()
            .gap(px(8.))
            .p(px(16.))
            // Add port row
            .child(
                h_flex()
                    .w_full()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .w(px(100.))
                            .child(Input::new(&port_host_input).small()),
                    )
                    .child(Label::new(":").text_color(muted_color))
                    .child(
                        div()
                            .w(px(100.))
                            .child(Input::new(&port_container_input).small()),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .child(
                                Button::new("tcp")
                                    .label("TCP")
                                    .xsmall()
                                    .when(port_protocol_tcp, Button::primary)
                                    .when(!port_protocol_tcp, ButtonVariants::ghost)
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.port_protocol_tcp = true;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("udp")
                                    .label("UDP")
                                    .xsmall()
                                    .when(!port_protocol_tcp, Button::primary)
                                    .when(port_protocol_tcp, ButtonVariants::ghost)
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.port_protocol_tcp = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Button::new("add-port")
                            .icon(IconName::Plus)
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                let host = this.port_host_input.as_ref()
                                    .map(|s| s.read(cx).text().to_string())
                                    .unwrap_or_default();
                                let container = this.port_container_input.as_ref()
                                    .map(|s| s.read(cx).text().to_string())
                                    .unwrap_or_default();

                                if !container.is_empty() {
                                    this.ports.push(PortMapping {
                                        host_port: if host.is_empty() { container.clone() } else { host },
                                        container_port: container,
                                        protocol: if this.port_protocol_tcp { "tcp".to_string() } else { "udp".to_string() },
                                    });
                                    // Recreate inputs to clear them
                                    this.port_host_input = Some(cx.new(|cx| {
                                        InputState::new(window, cx).placeholder("Host port")
                                    }));
                                    this.port_container_input = Some(cx.new(|cx| {
                                        InputState::new(window, cx).placeholder("Container port")
                                    }));
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted_color)
                    .child("Host port : Container port"),
            )
            // List of added ports
            .children(self.ports.iter().enumerate().map(|(idx, port)| {
                let protocol = port.protocol.clone();
                h_flex()
                    .w_full()
                    .py(px(8.))
                    .px(px(12.))
                    .gap(px(8.))
                    .items_center()
                    .bg(sidebar_color)
                    .rounded(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(foreground_color)
                            .child(format!("{}:{}/{}", port.host_port, port.container_port, protocol)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("remove-port-{idx}")))
                            .icon(IconName::Minus)
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.ports.remove(idx);
                                cx.notify();
                            })),
                    )
            }))
  }

  fn render_volumes_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let volume_host_input = self.volume_host_input.clone().unwrap();
    let volume_container_input = self.volume_container_input.clone().unwrap();
    let volume_readonly = self.volume_readonly;
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    v_flex()
            .w_full()
            .gap(px(8.))
            .p(px(16.))
            // Add volume row
            .child(
                h_flex()
                    .w_full()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .w(px(150.))
                            .child(Input::new(&volume_host_input).small()),
                    )
                    .child(Label::new(":").text_color(muted_color))
                    .child(
                        div()
                            .w(px(150.))
                            .child(Input::new(&volume_container_input).small()),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .items_center()
                            .child(Label::new("RO").text_color(muted_color).text_xs())
                            .child(
                                Switch::new("volume-ro")
                                    .checked(volume_readonly)
                                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                        this.volume_readonly = *checked;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Button::new("add-volume")
                            .icon(IconName::Plus)
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                let host = this.volume_host_input.as_ref()
                                    .map(|s| s.read(cx).text().to_string())
                                    .unwrap_or_default();
                                let container = this.volume_container_input.as_ref()
                                    .map(|s| s.read(cx).text().to_string())
                                    .unwrap_or_default();

                                if !host.is_empty() && !container.is_empty() {
                                    this.volumes.push(VolumeMount {
                                        host_path: host,
                                        container_path: container,
                                        read_only: this.volume_readonly,
                                    });
                                    // Recreate inputs to clear them
                                    this.volume_host_input = Some(cx.new(|cx| {
                                        InputState::new(window, cx).placeholder("Host path or volume name")
                                    }));
                                    this.volume_container_input = Some(cx.new(|cx| {
                                        InputState::new(window, cx).placeholder("Container path")
                                    }));
                                    this.volume_readonly = false;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted_color)
                    .child("Host path : Container path"),
            )
            // List of added volumes
            .children(self.volumes.iter().enumerate().map(|(idx, vol)| {
                let ro_label = if vol.read_only { " (ro)" } else { "" };
                h_flex()
                    .w_full()
                    .py(px(8.))
                    .px(px(12.))
                    .gap(px(8.))
                    .items_center()
                    .bg(sidebar_color)
                    .rounded(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(foreground_color)
                            .child(format!("{}:{}{}", vol.host_path, vol.container_path, ro_label)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("remove-vol-{idx}")))
                            .icon(IconName::Minus)
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.volumes.remove(idx);
                                cx.notify();
                            })),
                    )
            }))
  }

  fn render_env_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let env_key_input = self.env_key_input.clone().unwrap();
    let env_value_input = self.env_value_input.clone().unwrap();
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;
    let link_color = colors.link;

    v_flex()
            .w_full()
            .gap(px(8.))
            .p(px(16.))
            // Add env var row
            .child(
                h_flex()
                    .w_full()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .w(px(120.))
                            .child(Input::new(&env_key_input).small()),
                    )
                    .child(Label::new("=").text_color(muted_color))
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&env_value_input).small()),
                    )
                    .child(
                        Button::new("add-env")
                            .icon(IconName::Plus)
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                let key = this.env_key_input.as_ref()
                                    .map(|s| s.read(cx).text().to_string())
                                    .unwrap_or_default();
                                let value = this.env_value_input.as_ref()
                                    .map(|s| s.read(cx).text().to_string())
                                    .unwrap_or_default();

                                if !key.is_empty() {
                                    this.env_vars.push(EnvVar { key, value });
                                    // Recreate inputs to clear them
                                    this.env_key_input = Some(cx.new(|cx| {
                                        InputState::new(window, cx).placeholder("KEY")
                                    }));
                                    this.env_value_input = Some(cx.new(|cx| {
                                        InputState::new(window, cx).placeholder("VALUE")
                                    }));
                                    cx.notify();
                                }
                            })),
                    ),
            )
            // List of added env vars
            .children(self.env_vars.iter().enumerate().map(|(idx, env)| {
                h_flex()
                    .w_full()
                    .py(px(8.))
                    .px(px(12.))
                    .gap(px(8.))
                    .items_center()
                    .bg(sidebar_color)
                    .rounded(px(4.))
                    .child(
                        div()
                            .w(px(120.))
                            .text_sm()
                            .text_color(link_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(env.key.clone()),
                    )
                    .child(Label::new("=").text_color(muted_color))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(foreground_color)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(env.value.clone()),
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

  fn render_network_tab(&self, colors: &DialogColors, _cx: &mut Context<'_, Self>) -> impl IntoElement {
    let network_input = self.network_input.clone().unwrap();

    v_flex()
      .w_full()
      .gap(px(8.))
      .p(px(16.))
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(Label::new("Network").text_color(colors.foreground))
          .child(div().flex_1().child(Input::new(&network_input).small())),
      )
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Leave empty for default bridge network"),
      )
  }
}

impl Focusable for CreateContainerDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CreateContainerDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);

    let theme_colors = cx.theme().colors;
    let colors = DialogColors {
      border: theme_colors.border,
      foreground: theme_colors.foreground,
      muted_foreground: theme_colors.muted_foreground,
      sidebar: theme_colors.sidebar,
      link: theme_colors.link,
    };

    let active_tab = self.active_tab;
    let ports_count = self.ports.len();
    let volumes_count = self.volumes.len();
    let env_count = self.env_vars.len();

    let tabs = [
      "General".to_string(),
      format!("Ports ({ports_count})"),
      format!("Volumes ({volumes_count})"),
      format!("Env ({env_count})"),
      "Network".to_string(),
    ];

    let on_tab_change: Rc<dyn Fn(&usize, &mut Window, &mut App)> =
      Rc::new(cx.listener(|this, idx: &usize, _window, cx| {
        this.active_tab = *idx;
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
                    .child(
                        TabBar::new("create-container-tabs")
                            .children(tabs.iter().enumerate().map(|(i, label)| {
                                let on_tab_change = on_tab_change.clone();
                                Tab::new()
                                    .label(label.clone())
                                    .selected(active_tab == i)
                                    .on_click(move |_ev, window, cx| {
                                        on_tab_change(&i, window, cx);
                                    })
                            })),
                    ),
            )
            // Tab content
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .when(active_tab == 0, |el| el.child(self.render_general_tab(&colors, cx)))
                    .when(active_tab == 1, |el| el.child(self.render_ports_tab(&colors, cx)))
                    .when(active_tab == 2, |el| el.child(self.render_volumes_tab(&colors, cx)))
                    .when(active_tab == 3, |el| el.child(self.render_env_tab(&colors, cx)))
                    .when(active_tab == 4, |el| el.child(self.render_network_tab(&colors, cx))),
            )
  }
}
