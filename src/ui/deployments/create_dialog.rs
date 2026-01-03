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
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::kubernetes::{ContainerPortConfig, CreateDeploymentOptions};

#[derive(Clone)]
struct DialogColors {
  border: Hsla,
  foreground: Hsla,
  muted_foreground: Hsla,
  sidebar: Hsla,
  link: Hsla,
}

/// Image pull policy options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImagePullPolicy {
  #[default]
  IfNotPresent,
  Always,
  Never,
}

impl ImagePullPolicy {
  pub fn label(&self) -> &'static str {
    match self {
      ImagePullPolicy::IfNotPresent => "IfNotPresent",
      ImagePullPolicy::Always => "Always",
      ImagePullPolicy::Never => "Never",
    }
  }

  pub fn all() -> Vec<ImagePullPolicy> {
    vec![
      ImagePullPolicy::IfNotPresent,
      ImagePullPolicy::Always,
      ImagePullPolicy::Never,
    ]
  }
}

impl SelectItem for ImagePullPolicy {
  type Value = ImagePullPolicy;

  fn title(&self) -> SharedString {
    self.label().into()
  }

  fn value(&self) -> &Self::Value {
    self
  }
}

/// Port configuration for the dialog
#[derive(Debug, Clone, Default)]
pub struct PortConfig {
  pub name: String,
  pub container_port: String,
  pub protocol: String,
}

/// Environment variable
#[derive(Debug, Clone, Default)]
pub struct EnvVar {
  pub key: String,
  pub value: String,
}

/// Label pair
#[derive(Debug, Clone, Default)]
pub struct LabelPair {
  pub key: String,
  pub value: String,
}

/// Dialog for creating a new Kubernetes deployment
pub struct CreateDeploymentDialog {
  focus_handle: FocusHandle,
  active_tab: usize,

  // Basic inputs
  name_input: Option<Entity<InputState>>,
  namespace_input: Option<Entity<InputState>>,
  image_input: Option<Entity<InputState>>,
  replicas_input: Option<Entity<InputState>>,

  // Select states
  pull_policy_select: Option<Entity<SelectState<Vec<ImagePullPolicy>>>>,

  // Resource limits
  cpu_limit_input: Option<Entity<InputState>>,
  memory_limit_input: Option<Entity<InputState>>,
  cpu_request_input: Option<Entity<InputState>>,
  memory_request_input: Option<Entity<InputState>>,

  // Command
  command_input: Option<Entity<InputState>>,
  args_input: Option<Entity<InputState>>,

  // Ports
  ports: Vec<PortConfig>,
  port_name_input: Option<Entity<InputState>>,
  port_number_input: Option<Entity<InputState>>,
  port_protocol_tcp: bool,

  // Environment variables
  env_vars: Vec<EnvVar>,
  env_key_input: Option<Entity<InputState>>,
  env_value_input: Option<Entity<InputState>>,

  // Labels
  labels: Vec<LabelPair>,
  label_key_input: Option<Entity<InputState>>,
  label_value_input: Option<Entity<InputState>>,
}

impl CreateDeploymentDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      active_tab: 0,
      name_input: None,
      namespace_input: None,
      image_input: None,
      replicas_input: None,
      pull_policy_select: None,
      cpu_limit_input: None,
      memory_limit_input: None,
      cpu_request_input: None,
      memory_request_input: None,
      command_input: None,
      args_input: None,
      ports: Vec::new(),
      port_name_input: None,
      port_number_input: None,
      port_protocol_tcp: true,
      env_vars: Vec::new(),
      env_key_input: None,
      env_value_input: None,
      labels: Vec::new(),
      label_key_input: None,
      label_value_input: None,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.name_input.is_none() {
      self.name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. my-deployment")));
    }
    if self.namespace_input.is_none() {
      self.namespace_input = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .placeholder("default")
          .default_value("default")
      }));
    }
    if self.image_input.is_none() {
      self.image_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. nginx:latest")));
    }
    if self.replicas_input.is_none() {
      self.replicas_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("1").default_value("1")));
    }
    if self.pull_policy_select.is_none() {
      self.pull_policy_select =
        Some(cx.new(|cx| SelectState::new(ImagePullPolicy::all(), Some(IndexPath::new(0)), window, cx)));
    }
    if self.cpu_limit_input.is_none() {
      self.cpu_limit_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 500m")));
    }
    if self.memory_limit_input.is_none() {
      self.memory_limit_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 256Mi")));
    }
    if self.cpu_request_input.is_none() {
      self.cpu_request_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 100m")));
    }
    if self.memory_request_input.is_none() {
      self.memory_request_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 128Mi")));
    }
    if self.command_input.is_none() {
      self.command_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. /bin/sh,-c")));
    }
    if self.args_input.is_none() {
      self.args_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. echo hello")));
    }
    if self.port_name_input.is_none() {
      self.port_name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("http")));
    }
    if self.port_number_input.is_none() {
      self.port_number_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("80")));
    }
    if self.env_key_input.is_none() {
      self.env_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("KEY")));
    }
    if self.env_value_input.is_none() {
      self.env_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("VALUE")));
    }
    if self.label_key_input.is_none() {
      self.label_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("key")));
    }
    if self.label_value_input.is_none() {
      self.label_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("value")));
    }
  }

  pub fn get_options(&self, cx: &App) -> CreateDeploymentOptions {
    let name = self
      .name_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let namespace = self
      .namespace_input
      .as_ref()
      .map(|s| {
        let text = s.read(cx).text().to_string();
        if text.is_empty() { "default".to_string() } else { text }
      })
      .unwrap_or_else(|| "default".to_string());

    let image = self
      .image_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let replicas: i32 = self
      .replicas_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_else(|| "1".to_string())
      .parse()
      .unwrap_or(1);

    let pull_policy = self
      .pull_policy_select
      .as_ref()
      .and_then(|s| s.read(cx).selected_value().cloned())
      .unwrap_or_default();

    let cpu_limit = self
      .cpu_limit_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let memory_limit = self
      .memory_limit_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let cpu_request = self
      .cpu_request_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let memory_request = self
      .memory_request_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let command: Vec<String> = self
      .command_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty())
      .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
      .unwrap_or_default();

    let args: Vec<String> = self
      .args_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .filter(|s| !s.is_empty())
      .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
      .unwrap_or_default();

    let ports: Vec<ContainerPortConfig> = self
      .ports
      .iter()
      .filter(|p| !p.container_port.is_empty())
      .map(|p| ContainerPortConfig {
        name: p.name.clone(),
        container_port: p.container_port.parse().unwrap_or(80),
        protocol: p.protocol.clone(),
      })
      .collect();

    let env_vars: Vec<(String, String)> = self
      .env_vars
      .iter()
      .filter(|e| !e.key.is_empty())
      .map(|e| (e.key.clone(), e.value.clone()))
      .collect();

    let labels: Vec<(String, String)> = self
      .labels
      .iter()
      .filter(|l| !l.key.is_empty())
      .map(|l| (l.key.clone(), l.value.clone()))
      .collect();

    CreateDeploymentOptions {
      name,
      namespace,
      image,
      replicas,
      ports,
      env_vars,
      labels,
      cpu_limit,
      memory_limit,
      cpu_request,
      memory_request,
      image_pull_policy: pull_policy.label().to_string(),
      command,
      args,
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

  fn render_general_tab(&self, colors: &DialogColors, _cx: &mut Context<'_, Self>) -> impl IntoElement {
    let name_input = self.name_input.clone().unwrap();
    let namespace_input = self.namespace_input.clone().unwrap();
    let image_input = self.image_input.clone().unwrap();
    let replicas_input = self.replicas_input.clone().unwrap();
    let pull_policy_select = self.pull_policy_select.clone().unwrap();

    v_flex()
      .w_full()
      .child(self.render_form_row("Name", div().w(px(250.)).child(Input::new(&name_input).small()), colors))
      .child(self.render_form_row(
        "Namespace",
        div().w(px(250.)).child(Input::new(&namespace_input).small()),
        colors,
      ))
      .child(self.render_form_row(
        "Image",
        div().w(px(250.)).child(Input::new(&image_input).small()),
        colors,
      ))
      .child(self.render_form_row(
        "Replicas",
        div().w(px(100.)).child(Input::new(&replicas_input).small()),
        colors,
      ))
      .child(self.render_form_row_with_desc(
        "Image Pull Policy",
        "When to pull the container image",
        div().w(px(150.)).child(Select::new(&pull_policy_select).small()),
        colors,
      ))
  }

  fn render_resources_tab(&self, colors: &DialogColors, _cx: &mut Context<'_, Self>) -> impl IntoElement {
    let cpu_limit = self.cpu_limit_input.clone().unwrap();
    let memory_limit = self.memory_limit_input.clone().unwrap();
    let cpu_request = self.cpu_request_input.clone().unwrap();
    let memory_request = self.memory_request_input.clone().unwrap();
    let command_input = self.command_input.clone().unwrap();
    let args_input = self.args_input.clone().unwrap();

    v_flex()
      .w_full()
      .child(self.render_section_header("Resource Limits", colors))
      .child(self.render_form_row_with_desc(
        "CPU Limit",
        "Maximum CPU (e.g. 500m, 1)",
        div().w(px(150.)).child(Input::new(&cpu_limit).small()),
        colors,
      ))
      .child(self.render_form_row_with_desc(
        "Memory Limit",
        "Maximum memory (e.g. 256Mi, 1Gi)",
        div().w(px(150.)).child(Input::new(&memory_limit).small()),
        colors,
      ))
      .child(self.render_section_header("Resource Requests", colors))
      .child(self.render_form_row_with_desc(
        "CPU Request",
        "Minimum CPU (e.g. 100m)",
        div().w(px(150.)).child(Input::new(&cpu_request).small()),
        colors,
      ))
      .child(self.render_form_row_with_desc(
        "Memory Request",
        "Minimum memory (e.g. 128Mi)",
        div().w(px(150.)).child(Input::new(&memory_request).small()),
        colors,
      ))
      .child(self.render_section_header("Command", colors))
      .child(self.render_form_row_with_desc(
        "Command",
        "Container command (comma-separated)",
        div().w(px(200.)).child(Input::new(&command_input).small()),
        colors,
      ))
      .child(self.render_form_row_with_desc(
        "Args",
        "Container args (comma-separated)",
        div().w(px(200.)).child(Input::new(&args_input).small()),
        colors,
      ))
  }

  fn render_ports_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let port_name_input = self.port_name_input.clone().unwrap();
    let port_number_input = self.port_number_input.clone().unwrap();
    let port_protocol_tcp = self.port_protocol_tcp;
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    v_flex()
      .w_full()
      .gap(px(8.))
      .p(px(16.))
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(div().w(px(80.)).child(Input::new(&port_name_input).small()))
          .child(div().w(px(80.)).child(Input::new(&port_number_input).small()))
          .child(
            h_flex()
              .gap(px(4.))
              .child(
                Button::new("tcp")
                  .label("TCP")
                  .xsmall()
                  .when(port_protocol_tcp, Button::primary)
                  .when(!port_protocol_tcp, |b| b.ghost())
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
                  .when(port_protocol_tcp, |b| b.ghost())
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
                let name = this
                  .port_name_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();
                let port = this
                  .port_number_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !port.is_empty() {
                  this.ports.push(PortConfig {
                    name,
                    container_port: port,
                    protocol: if this.port_protocol_tcp {
                      "TCP".to_string()
                    } else {
                      "UDP".to_string()
                    },
                  });
                  this.port_name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("http")));
                  this.port_number_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("80")));
                  cx.notify();
                }
              })),
          ),
      )
      .child(
        div()
          .text_xs()
          .text_color(muted_color)
          .child("Name (optional) | Port | Protocol"),
      )
      .children(self.ports.iter().enumerate().map(|(idx, port)| {
        let display = if port.name.is_empty() {
          format!("{}/{}", port.container_port, port.protocol)
        } else {
          format!("{}: {}/{}", port.name, port.container_port, port.protocol)
        };
        h_flex()
          .w_full()
          .py(px(8.))
          .px(px(12.))
          .gap(px(8.))
          .items_center()
          .bg(sidebar_color)
          .rounded(px(4.))
          .child(div().flex_1().text_sm().text_color(foreground_color).child(display))
          .child(
            Button::new(SharedString::from(format!("remove-port-{}", idx)))
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
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
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
                  this.env_vars.push(EnvVar { key, value });
                  this.env_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("KEY")));
                  this.env_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("VALUE")));
                  cx.notify();
                }
              })),
          ),
      )
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
            Button::new(SharedString::from(format!("remove-env-{}", idx)))
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

  fn render_labels_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let label_key_input = self.label_key_input.clone().unwrap();
    let label_value_input = self.label_value_input.clone().unwrap();
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;
    let link_color = colors.link;

    v_flex()
      .w_full()
      .gap(px(8.))
      .p(px(16.))
      .child(
        div()
          .text_xs()
          .text_color(muted_color)
          .child("Labels are applied to the deployment and used as selectors"),
      )
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(div().w(px(120.)).child(Input::new(&label_key_input).small()))
          .child(Label::new("=").text_color(muted_color))
          .child(div().flex_1().child(Input::new(&label_value_input).small()))
          .child(
            Button::new("add-label")
              .icon(IconName::Plus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                let key = this
                  .label_key_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();
                let value = this
                  .label_value_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !key.is_empty() {
                  this.labels.push(LabelPair { key, value });
                  this.label_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("key")));
                  this.label_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("value")));
                  cx.notify();
                }
              })),
          ),
      )
      .children(self.labels.iter().enumerate().map(|(idx, label)| {
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
              .child(label.key.clone()),
          )
          .child(Label::new("=").text_color(muted_color))
          .child(
            div()
              .flex_1()
              .text_sm()
              .text_color(foreground_color)
              .overflow_hidden()
              .text_ellipsis()
              .child(label.value.clone()),
          )
          .child(
            Button::new(SharedString::from(format!("remove-label-{}", idx)))
              .icon(IconName::Minus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.labels.remove(idx);
                cx.notify();
              })),
          )
      }))
  }
}

impl Focusable for CreateDeploymentDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CreateDeploymentDialog {
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
    let env_count = self.env_vars.len();
    let labels_count = self.labels.len();

    let tabs = [
      "General".to_string(),
      "Resources".to_string(),
      format!("Ports ({})", ports_count),
      format!("Env ({})", env_count),
      format!("Labels ({})", labels_count),
    ];

    let on_tab_change: Rc<dyn Fn(&usize, &mut Window, &mut App)> =
      Rc::new(cx.listener(|this, idx: &usize, _window, cx| {
        this.active_tab = *idx;
        cx.notify();
      }));

    v_flex()
      .w_full()
      .max_h(px(500.))
      .child(div().w_full().border_b_1().border_color(colors.border).child(
        TabBar::new("create-deployment-tabs").children(tabs.iter().enumerate().map(|(i, label)| {
          let on_tab_change = on_tab_change.clone();
          Tab::new()
            .label(label.to_string())
            .selected(active_tab == i)
            .on_click(move |_ev, window, cx| {
              on_tab_change(&i, window, cx);
            })
        })),
      ))
      .child(
        div()
          .flex_1()
          .overflow_y_scrollbar()
          .when(active_tab == 0, |el| el.child(self.render_general_tab(&colors, cx)))
          .when(active_tab == 1, |el| el.child(self.render_resources_tab(&colors, cx)))
          .when(active_tab == 2, |el| el.child(self.render_ports_tab(&colors, cx)))
          .when(active_tab == 3, |el| el.child(self.render_env_tab(&colors, cx)))
          .when(active_tab == 4, |el| el.child(self.render_labels_tab(&colors, cx))),
      )
  }
}
