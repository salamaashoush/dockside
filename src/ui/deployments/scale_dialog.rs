use gpui::{App, Context, Entity, FocusHandle, Focusable, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  theme::ActiveTheme,
  v_flex,
};

/// Dialog for scaling a deployment's replica count
pub struct ScaleDialog {
  focus_handle: FocusHandle,
  deployment_name: String,
  namespace: String,
  current_replicas: i32,
  replicas_input: Option<Entity<InputState>>,
}

impl ScaleDialog {
  pub fn new(deployment_name: String, namespace: String, current_replicas: i32, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      deployment_name,
      namespace,
      current_replicas,
      replicas_input: None,
    }
  }

  fn ensure_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.replicas_input.is_none() {
      self.replicas_input =
        Some(cx.new(|cx| InputState::new(window, cx).default_value(self.current_replicas.to_string())));
    }
  }

  pub fn get_replicas(&self, cx: &App) -> i32 {
    self
      .replicas_input
      .as_ref()
      .map_or_else(|| self.current_replicas.to_string(), |s| s.read(cx).text().to_string())
      .parse::<i32>()
      .unwrap_or(self.current_replicas)
  }

  pub fn deployment_name(&self) -> &str {
    &self.deployment_name
  }

  pub fn namespace(&self) -> &str {
    &self.namespace
  }
}

impl Focusable for ScaleDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ScaleDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_input(window, cx);

    let colors = cx.theme().colors;
    let replicas_input = self.replicas_input.clone().unwrap();

    v_flex()
      .w_full()
      .gap(px(16.))
      .p(px(16.))
      .child(div().text_sm().text_color(colors.muted_foreground).child(format!(
        "Scale deployment '{}' in namespace '{}'",
        self.deployment_name, self.namespace
      )))
      .child(
        h_flex()
          .w_full()
          .gap(px(16.))
          .items_center()
          .child(Label::new("Replicas").text_color(colors.foreground))
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(
                Button::new("decrease")
                  .icon(IconName::Minus)
                  .ghost()
                  .small()
                  .on_click(cx.listener(|this, _ev, window, cx| {
                    if let Some(input) = &this.replicas_input {
                      let current: i32 = input.read(cx).text().to_string().parse().unwrap_or(0);
                      if current > 0 {
                        let new_val = current - 1;
                        this.replicas_input =
                          Some(cx.new(|cx| InputState::new(window, cx).default_value(new_val.to_string())));
                        cx.notify();
                      }
                    }
                  })),
              )
              .child(div().w(px(80.)).child(Input::new(&replicas_input).small()))
              .child(
                Button::new("increase")
                  .icon(IconName::Plus)
                  .ghost()
                  .small()
                  .on_click(cx.listener(|this, _ev, window, cx| {
                    if let Some(input) = &this.replicas_input {
                      let current: i32 = input.read(cx).text().to_string().parse().unwrap_or(0);
                      let new_val = current + 1;
                      this.replicas_input =
                        Some(cx.new(|cx| InputState::new(window, cx).default_value(new_val.to_string())));
                      cx.notify();
                    }
                  })),
              ),
          ),
      )
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(format!("Current: {} replicas", self.current_replicas)),
      )
  }
}
