use gpui::{App, Context, Entity, FocusHandle, Focusable, Hsla, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  IndexPath, Sizable, h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  select::{Select, SelectState},
  switch::Switch,
  theme::ActiveTheme,
  v_flex,
};

use super::pull_dialog::PullPlatform;

#[derive(Debug, Clone, Default)]
pub struct BuildImageOptions {
  /// Build context root (a directory on disk).
  pub context_dir: String,
  /// Dockerfile path relative to the context root.
  pub dockerfile: String,
  /// Image tag (e.g. `myapp:latest`).
  pub tag: String,
  /// Optional multi-stage target.
  pub target: Option<String>,
  /// Build args as `KEY=VALUE` list.
  pub build_args: Vec<(String, String)>,
  /// Target platform (linux/amd64, linux/arm64, …).
  pub platform: PullPlatform,
  pub no_cache: bool,
  pub pull: bool,
}

pub struct BuildImageDialog {
  focus_handle: FocusHandle,
  context_input: Option<Entity<InputState>>,
  dockerfile_input: Option<Entity<InputState>>,
  tag_input: Option<Entity<InputState>>,
  target_input: Option<Entity<InputState>>,
  build_args_input: Option<Entity<InputState>>,
  platform_select: Option<Entity<SelectState<Vec<PullPlatform>>>>,
  no_cache: bool,
  pull: bool,
}

impl BuildImageDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    Self {
      focus_handle,
      context_input: None,
      dockerfile_input: None,
      tag_input: None,
      target_input: None,
      build_args_input: None,
      platform_select: None,
      no_cache: false,
      pull: false,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.context_input.is_none() {
      self.context_input = Some(cx.new(|cx| {
        InputState::new(window, cx).placeholder("Absolute path to build context")
      }));
    }
    if self.dockerfile_input.is_none() {
      self.dockerfile_input = Some(cx.new(|cx| {
        InputState::new(window, cx).default_value("Dockerfile")
      }));
    }
    if self.tag_input.is_none() {
      self.tag_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("e.g. my-app:latest")));
    }
    if self.target_input.is_none() {
      self.target_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Stage name (optional)")));
    }
    if self.build_args_input.is_none() {
      self.build_args_input = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .placeholder("KEY=value, one per line")
      }));
    }
    if self.platform_select.is_none() {
      self.platform_select = Some(cx.new(|cx| {
        SelectState::new(PullPlatform::all(), Some(IndexPath::new(0)), window, cx)
      }));
    }
  }

  pub fn get_options(&self, cx: &App) -> BuildImageOptions {
    let read = |opt: &Option<Entity<InputState>>| {
      opt
        .as_ref()
        .map(|s| s.read(cx).text().to_string())
        .unwrap_or_default()
    };
    let context_dir = read(&self.context_input);
    let dockerfile = {
      let v = read(&self.dockerfile_input);
      if v.is_empty() { "Dockerfile".to_string() } else { v }
    };
    let tag = read(&self.tag_input);
    let target = {
      let v = read(&self.target_input);
      if v.is_empty() { None } else { Some(v) }
    };

    let build_args: Vec<(String, String)> = read(&self.build_args_input)
      .lines()
      .filter_map(|line| {
        let line = line.trim();
        if line.is_empty() {
          return None;
        }
        let (k, v) = line.split_once('=')?;
        Some((k.trim().to_string(), v.trim().to_string()))
      })
      .collect();

    let platform = self
      .platform_select
      .as_ref()
      .and_then(|s| s.read(cx).selected_value().copied())
      .unwrap_or_default();

    BuildImageOptions {
      context_dir,
      dockerfile,
      tag,
      target,
      build_args,
      platform,
      no_cache: self.no_cache,
      pull: self.pull,
    }
  }
}

impl Focusable for BuildImageDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for BuildImageDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);
    let colors = cx.theme().colors;
    let context_input = self.context_input.clone().unwrap();
    let dockerfile_input = self.dockerfile_input.clone().unwrap();
    let tag_input = self.tag_input.clone().unwrap();
    let target_input = self.target_input.clone().unwrap();
    let build_args_input = self.build_args_input.clone().unwrap();
    let platform_select = self.platform_select.clone().unwrap();
    let no_cache = self.no_cache;
    let pull = self.pull;

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
      .max_h(px(520.))
      .overflow_y_scrollbar()
      .child(
        div()
          .w_full()
          .px(px(16.))
          .py(px(12.))
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Build a Docker image from a local Dockerfile + build context."),
      )
      .child(row(
        "Context dir",
        div().w(px(360.)).child(Input::new(&context_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Dockerfile",
        div().w(px(220.)).child(Input::new(&dockerfile_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Tag",
        div().w(px(220.)).child(Input::new(&tag_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Target stage",
        div().w(px(220.)).child(Input::new(&target_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Platform",
        div().w(px(160.)).child(Select::new(&platform_select).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(
        v_flex()
          .w_full()
          .py(px(12.))
          .px(px(16.))
          .gap(px(4.))
          .border_b_1()
          .border_color(colors.border)
          .child(Label::new("Build args").text_color(colors.foreground))
          .child(div().w_full().h(px(120.)).child(Input::new(&build_args_input).small())),
      )
      .child(row(
        "No cache",
        Switch::new("build-nocache")
          .checked(no_cache)
          .on_click(cx.listener(|this, checked: &bool, _w, cx| {
            this.no_cache = *checked;
            cx.notify();
          }))
          .into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Pull base images",
        Switch::new("build-pull")
          .checked(pull)
          .on_click(cx.listener(|this, checked: &bool, _w, cx| {
            this.pull = *checked;
            cx.notify();
          }))
          .into_any_element(),
        colors.border,
        colors.foreground,
      ))
  }
}
