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

use crate::kubernetes::{CreateServiceOptions, ServicePortConfig};

#[derive(Clone)]
struct DialogColors {
  border: Hsla,
  foreground: Hsla,
  muted_foreground: Hsla,
  sidebar: Hsla,
  link: Hsla,
}

/// Service type options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceType {
  #[default]
  ClusterIP,
  NodePort,
  LoadBalancer,
}

impl ServiceType {
  pub fn label(&self) -> &'static str {
    match self {
      ServiceType::ClusterIP => "ClusterIP",
      ServiceType::NodePort => "NodePort",
      ServiceType::LoadBalancer => "LoadBalancer",
    }
  }

  pub fn all() -> Vec<ServiceType> {
    vec![ServiceType::ClusterIP, ServiceType::NodePort, ServiceType::LoadBalancer]
  }
}

impl SelectItem for ServiceType {
  type Value = ServiceType;

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
  pub port: String,
  pub target_port: String,
  pub node_port: String,
  pub protocol: String,
}

/// Selector label pair
#[derive(Debug, Clone, Default)]
pub struct SelectorPair {
  pub key: String,
  pub value: String,
}

/// Dialog for creating a new Kubernetes service
pub struct CreateServiceDialog {
  focus_handle: FocusHandle,
  active_tab: usize,

  // Basic inputs
  name_input: Option<Entity<InputState>>,
  namespace_input: Option<Entity<InputState>>,

  // Service type select
  service_type_select: Option<Entity<SelectState<Vec<ServiceType>>>>,

  // Ports
  ports: Vec<PortConfig>,
  port_name_input: Option<Entity<InputState>>,
  port_input: Option<Entity<InputState>>,
  target_port_input: Option<Entity<InputState>>,
  node_port_input: Option<Entity<InputState>>,
  port_protocol_tcp: bool,

  // Selectors
  selectors: Vec<SelectorPair>,
  selector_key_input: Option<Entity<InputState>>,
  selector_value_input: Option<Entity<InputState>>,
}

impl CreateServiceDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      active_tab: 0,
      name_input: None,
      namespace_input: None,
      service_type_select: None,
      ports: Vec::new(),
      port_name_input: None,
      port_input: None,
      target_port_input: None,
      node_port_input: None,
      port_protocol_tcp: true,
      selectors: Vec::new(),
      selector_key_input: None,
      selector_value_input: None,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.name_input.is_none() {
      self.name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. my-service")));
    }
    if self.namespace_input.is_none() {
      self.namespace_input = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .placeholder("default")
          .default_value("default")
      }));
    }
    if self.service_type_select.is_none() {
      self.service_type_select =
        Some(cx.new(|cx| SelectState::new(ServiceType::all(), Some(IndexPath::new(0)), window, cx)));
    }
    if self.port_name_input.is_none() {
      self.port_name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("http")));
    }
    if self.port_input.is_none() {
      self.port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("80")));
    }
    if self.target_port_input.is_none() {
      self.target_port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("8080")));
    }
    if self.node_port_input.is_none() {
      self.node_port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("30000")));
    }
    if self.selector_key_input.is_none() {
      self.selector_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("app")));
    }
    if self.selector_value_input.is_none() {
      self.selector_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("my-app")));
    }
  }

  pub fn get_options(&self, cx: &App) -> CreateServiceOptions {
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

    let service_type = self
      .service_type_select
      .as_ref()
      .and_then(|s| s.read(cx).selected_value().copied())
      .unwrap_or_default();

    let ports: Vec<ServicePortConfig> = self
      .ports
      .iter()
      .filter(|p| !p.port.is_empty())
      .map(|p| ServicePortConfig {
        name: p.name.clone(),
        port: p.port.parse().unwrap_or(80),
        target_port: p.target_port.parse().unwrap_or_else(|_| p.port.parse().unwrap_or(80)),
        node_port: p.node_port.parse().unwrap_or(0),
        protocol: p.protocol.clone(),
      })
      .collect();

    let selector: Vec<(String, String)> = self
      .selectors
      .iter()
      .filter(|s| !s.key.is_empty())
      .map(|s| (s.key.clone(), s.value.clone()))
      .collect();

    CreateServiceOptions {
      name,
      namespace,
      service_type: service_type.label().to_string(),
      ports,
      selector,
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

  fn render_general_tab(&self, colors: &DialogColors, _cx: &mut Context<'_, Self>) -> impl IntoElement {
    let name_input = self.name_input.clone().unwrap();
    let namespace_input = self.namespace_input.clone().unwrap();
    let service_type_select = self.service_type_select.clone().unwrap();

    v_flex()
      .w_full()
      .child(self.render_form_row("Name", div().w(px(250.)).child(Input::new(&name_input).small()), colors))
      .child(self.render_form_row(
        "Namespace",
        div().w(px(250.)).child(Input::new(&namespace_input).small()),
        colors,
      ))
      .child(self.render_form_row_with_desc(
        "Service Type",
        "How the service is exposed",
        div().w(px(150.)).child(Select::new(&service_type_select).small()),
        colors,
      ))
  }

  fn render_ports_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let port_name_input = self.port_name_input.clone().unwrap();
    let port_input = self.port_input.clone().unwrap();
    let target_port_input = self.target_port_input.clone().unwrap();
    let node_port_input = self.node_port_input.clone().unwrap();
    let port_protocol_tcp = self.port_protocol_tcp;
    let sidebar_color = colors.sidebar;
    let foreground_color = colors.foreground;
    let muted_color = colors.muted_foreground;

    let service_type = self
      .service_type_select
      .as_ref()
      .and_then(|s| s.read(cx).selected_value().copied())
      .unwrap_or_default();
    let show_node_port = matches!(service_type, ServiceType::NodePort | ServiceType::LoadBalancer);

    v_flex()
      .w_full()
      .gap(px(8.))
      .p(px(16.))
      .child(
        v_flex().w_full().gap(px(8.)).child(
          h_flex()
            .w_full()
            .gap(px(8.))
            .items_center()
            .child(
              v_flex()
                .gap(px(2.))
                .child(div().text_xs().text_color(muted_color).child("Name"))
                .child(div().w(px(70.)).child(Input::new(&port_name_input).small())),
            )
            .child(
              v_flex()
                .gap(px(2.))
                .child(div().text_xs().text_color(muted_color).child("Port"))
                .child(div().w(px(60.)).child(Input::new(&port_input).small())),
            )
            .child(
              v_flex()
                .gap(px(2.))
                .child(div().text_xs().text_color(muted_color).child("Target"))
                .child(div().w(px(60.)).child(Input::new(&target_port_input).small())),
            )
            .when(show_node_port, |el| {
              el.child(
                v_flex()
                  .gap(px(2.))
                  .child(div().text_xs().text_color(muted_color).child("NodePort"))
                  .child(div().w(px(60.)).child(Input::new(&node_port_input).small())),
              )
            })
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
                  let name = this
                    .port_name_input
                    .as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default();
                  let port = this
                    .port_input
                    .as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default();
                  let target_port = this
                    .target_port_input
                    .as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default();
                  let node_port = this
                    .node_port_input
                    .as_ref()
                    .map(|s| s.read(cx).text().to_string())
                    .unwrap_or_default();

                  if !port.is_empty() {
                    this.ports.push(PortConfig {
                      name,
                      port,
                      target_port,
                      node_port,
                      protocol: if this.port_protocol_tcp {
                        "TCP".to_string()
                      } else {
                        "UDP".to_string()
                      },
                    });
                    this.port_name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("http")));
                    this.port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("80")));
                    this.target_port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("8080")));
                    this.node_port_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("30000")));
                    cx.notify();
                  }
                })),
            ),
        ),
      )
      .child(
        div()
          .text_xs()
          .text_color(muted_color)
          .child("Port: service port, Target: container port"),
      )
      .children(self.ports.iter().enumerate().map(|(idx, port)| {
        let display = if port.name.is_empty() {
          if port.node_port.is_empty() || port.node_port == "0" {
            format!("{}:{}/{}", port.port, port.target_port, port.protocol)
          } else {
            format!(
              "{}:{}:{}/{}",
              port.port, port.target_port, port.node_port, port.protocol
            )
          }
        } else if port.node_port.is_empty() || port.node_port == "0" {
          format!("{}: {}:{}/{}", port.name, port.port, port.target_port, port.protocol)
        } else {
          format!(
            "{}: {}:{}:{}/{}",
            port.name, port.port, port.target_port, port.node_port, port.protocol
          )
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

  fn render_selectors_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let selector_key_input = self.selector_key_input.clone().unwrap();
    let selector_value_input = self.selector_value_input.clone().unwrap();
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
          .child("Selectors determine which pods this service routes traffic to"),
      )
      .child(
        h_flex()
          .w_full()
          .gap(px(8.))
          .items_center()
          .child(div().w(px(120.)).child(Input::new(&selector_key_input).small()))
          .child(Label::new("=").text_color(muted_color))
          .child(div().flex_1().child(Input::new(&selector_value_input).small()))
          .child(
            Button::new("add-selector")
              .icon(IconName::Plus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                let key = this
                  .selector_key_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();
                let value = this
                  .selector_value_input
                  .as_ref()
                  .map(|s| s.read(cx).text().to_string())
                  .unwrap_or_default();

                if !key.is_empty() {
                  this.selectors.push(SelectorPair { key, value });
                  this.selector_key_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("app")));
                  this.selector_value_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("my-app")));
                  cx.notify();
                }
              })),
          ),
      )
      .children(self.selectors.iter().enumerate().map(|(idx, selector)| {
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
              .child(selector.key.clone()),
          )
          .child(Label::new("=").text_color(muted_color))
          .child(
            div()
              .flex_1()
              .text_sm()
              .text_color(foreground_color)
              .overflow_hidden()
              .text_ellipsis()
              .child(selector.value.clone()),
          )
          .child(
            Button::new(SharedString::from(format!("remove-selector-{idx}")))
              .icon(IconName::Minus)
              .xsmall()
              .ghost()
              .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.selectors.remove(idx);
                cx.notify();
              })),
          )
      }))
  }
}

impl Focusable for CreateServiceDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CreateServiceDialog {
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
    let selectors_count = self.selectors.len();

    let tabs = [
      "General".to_string(),
      format!("Ports ({ports_count})"),
      format!("Selectors ({selectors_count})"),
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
        TabBar::new("create-service-tabs").children(tabs.iter().enumerate().map(|(i, label)| {
          let on_tab_change = on_tab_change.clone();
          Tab::new()
            .label(label.clone())
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
          .when(active_tab == 1, |el| el.child(self.render_ports_tab(&colors, cx)))
          .when(active_tab == 2, |el| el.child(self.render_selectors_tab(&colors, cx))),
      )
  }
}
