use gpui::{App, Context, Entity, FocusHandle, Focusable, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Sizable, h_flex,
  input::{Input, InputState},
  label::Label,
  switch::Switch,
  theme::ActiveTheme,
  v_flex,
};

/// Options for creating a network
#[derive(Debug, Clone, Default)]
pub struct CreateNetworkOptions {
  pub name: String,
  pub enable_ipv6: bool,
  pub subnet: Option<String>,
}

/// Dialog for creating a new network
pub struct CreateNetworkDialog {
  focus_handle: FocusHandle,
  name_input: Option<Entity<InputState>>,
  subnet_input: Option<Entity<InputState>>,
  enable_ipv6: bool,
}

impl CreateNetworkDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      name_input: None,
      subnet_input: None,
      enable_ipv6: false,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.name_input.is_none() {
      self.name_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Name")));
    }

    if self.subnet_input.is_none() {
      self.subnet_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("172.30.30.0/24")));
    }
  }

  pub fn get_options(&self, cx: &App) -> CreateNetworkOptions {
    let name = self
      .name_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let subnet_text = self
      .subnet_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();

    let subnet = if subnet_text.is_empty() {
      None
    } else {
      Some(subnet_text)
    };

    CreateNetworkOptions {
      name,
      enable_ipv6: self.enable_ipv6,
      subnet,
    }
  }

  fn render_form_row(label: &'static str, content: impl IntoElement, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

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
}

impl Focusable for CreateNetworkDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CreateNetworkDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);

    let colors = &cx.theme().colors;
    let name_input = self.name_input.clone().unwrap();
    let subnet_input = self.subnet_input.clone().unwrap();
    let enable_ipv6 = self.enable_ipv6;

    v_flex()
            .w_full()
            .gap(px(0.))
            // Name input (full width)
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(colors.border)
                    .child(Input::new(&name_input).w_full()),
            )
            // Advanced section header
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.foreground)
                    .child("Advanced"),
            )
            // IPv6 toggle
            .child(
                Self::render_form_row(
                    "IPv6",
                    Switch::new("ipv6")
                        .checked(enable_ipv6)
                        .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                            this.enable_ipv6 = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
            )
            // Subnet input
            .child(
                Self::render_form_row(
                    "Subnet (IPv4)",
                    div().w(px(150.)).child(Input::new(&subnet_input).small()),
                    cx,
                ),
            )
  }
}
