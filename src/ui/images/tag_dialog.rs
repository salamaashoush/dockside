use gpui::{App, Context, Entity, FocusHandle, Focusable, Hsla, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Sizable, h_flex,
  input::{Input, InputState},
  label::Label,
  theme::ActiveTheme,
  v_flex,
};

#[derive(Debug, Clone, Default)]
pub struct TagImageOptions {
  /// Source image (id or repo:tag) being retagged.
  pub source: String,
  /// New repository (e.g. `my/app` or `ghcr.io/me/app`).
  pub repo: String,
  /// New tag (e.g. `v1.2.3`).
  pub tag: String,
}

pub struct TagImageDialog {
  focus_handle: FocusHandle,
  source: String,
  repo_input: Option<Entity<InputState>>,
  tag_input: Option<Entity<InputState>>,
}

impl TagImageDialog {
  pub fn new(source: String) -> impl FnOnce(&mut Context<'_, Self>) -> Self {
    move |cx| Self {
      focus_handle: cx.focus_handle(),
      source,
      repo_input: None,
      tag_input: None,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.repo_input.is_none() {
      self.repo_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. my-org/my-app")));
    }
    if self.tag_input.is_none() {
      self.tag_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. v1.0.0").default_value("latest")));
    }
  }

  pub fn get_options(&self, cx: &App) -> TagImageOptions {
    let repo = self
      .repo_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_default();
    let tag = self
      .tag_input
      .as_ref()
      .map(|s| s.read(cx).text().to_string())
      .unwrap_or_else(|| "latest".to_string());
    TagImageOptions {
      source: self.source.clone(),
      repo,
      tag,
    }
  }
}

impl Focusable for TagImageDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for TagImageDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);
    let colors = cx.theme().colors;
    let repo_input = self.repo_input.clone().unwrap();
    let tag_input = self.tag_input.clone().unwrap();
    let source = self.source.clone();

    let row = |label: &'static str, content: gpui::AnyElement, border: Hsla, fg: Hsla| {
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

    v_flex()
      .w_full()
      .child(
        div()
          .w_full()
          .px(px(16.))
          .py(px(12.))
          .text_sm()
          .text_color(colors.muted_foreground)
          .child(format!("Add a new tag pointing at {source}.")),
      )
      .child(row(
        "Repository",
        div().w(px(300.)).child(Input::new(&repo_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Tag",
        div().w(px(300.)).child(Input::new(&tag_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
  }
}
