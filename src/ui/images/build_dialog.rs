use gpui::{App, AppContext, Context, Entity, FocusHandle, Focusable, Hsla, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  IndexPath, Sizable, h_flex,
  button::{Button, ButtonVariants},
  input::{Input, InputEvent, InputState},
  label::Label,
  scroll::ScrollableElement,
  select::{Select, SelectState},
  switch::Switch,
  theme::ActiveTheme,
  v_flex,
};
use std::time::Duration;

use super::pull_dialog::PullPlatform;
use crate::docker::{ERR_HADOLINT_NOT_INSTALLED, LintReport, hadolint_install_hint};
use crate::ui::components::{render_error_panel, render_install_hint};

/// Debounce window before auto-lint actually fires after the user
/// stops typing in `context_dir` / `dockerfile`. Long enough to feel
/// non-intrusive; short enough that the inline banner updates without
/// the user hunting for the Lint button.
const AUTO_LINT_DEBOUNCE: Duration = Duration::from_millis(750);

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
  lint_loading: bool,
  lint_result: Option<Result<LintReport, String>>,
  lint_dockerfile_path: Option<String>,
  /// Generation counter incremented on every Dockerfile-field edit.
  /// The debounce timer captures the value at scheduling time and the
  /// trailing `run_lint` only fires if it still matches — that lets us
  /// "cancel" stale runs without juggling Task handles.
  lint_gen: u64,
  lint_subscribed: bool,
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
      lint_loading: false,
      lint_result: None,
      lint_dockerfile_path: None,
      lint_gen: 0,
      lint_subscribed: false,
    }
  }

  /// Bump the generation, wait `AUTO_LINT_DEBOUNCE`, and re-run lint
  /// if no further edits arrived in the meantime. Errors and missing
  /// hadolint are routed through the same `lint_result` slot as the
  /// manual Lint button, so the inline banner / install hint updates
  /// without any extra UI.
  fn schedule_auto_lint(&mut self, cx: &mut Context<'_, Self>) {
    self.lint_gen = self.lint_gen.wrapping_add(1);
    let scheduled = self.lint_gen;
    cx.spawn(async move |this, cx| {
      Timer::after(AUTO_LINT_DEBOUNCE).await;
      let _ = cx.update(|cx| {
        if let Some(this) = this.upgrade() {
          let still_latest = this.read(cx).lint_gen == scheduled;
          let context_empty = this
            .read(cx)
            .context_input
            .as_ref()
            .is_none_or(|s| s.read(cx).text().to_string().is_empty());
          if still_latest && !context_empty {
            Self::run_lint(&this, cx);
          }
        }
      });
    })
    .detach();
  }

  /// Resolve `<context_dir>/<dockerfile>` and run hadolint in the
  /// background, then route the result back into `lint_result` so the
  /// inline banner under the form can render it.
  pub fn run_lint(this: &Entity<Self>, cx: &mut App) {
    let opts = this.read(cx).get_options(cx);
    if opts.context_dir.is_empty() {
      return;
    }
    let dockerfile_path = std::path::PathBuf::from(&opts.context_dir).join(&opts.dockerfile);
    let dockerfile_display = dockerfile_path.display().to_string();
    this.update(cx, |dialog, cx| {
      dialog.lint_loading = true;
      dialog.lint_result = None;
      dialog.lint_dockerfile_path = Some(dockerfile_display.clone());
      cx.notify();
    });

    let path_for_task = dockerfile_path.clone();
    let weak = this.downgrade();
    cx.spawn(async move |cx| {
      let result = cx
        .background_executor()
        .spawn(async move { crate::docker::lint_dockerfile(&path_for_task) })
        .await;
      let _ = cx.update(|cx| {
        if let Some(this) = weak.upgrade() {
          this.update(cx, |dialog, cx| {
            dialog.lint_loading = false;
            dialog.lint_result = Some(result.map_err(|e| e.to_string()));
            cx.notify();
          });
        }
      });
    })
    .detach();
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
    // Subscribe once both inputs exist so edits to either field
    // schedule a debounced auto-lint. Idempotent via the `lint_subscribed`
    // latch so this only wires up the first time we render.
    if !self.lint_subscribed
      && let (Some(ctx_input), Some(df_input)) = (self.context_input.clone(), self.dockerfile_input.clone())
    {
      self.lint_subscribed = true;
      cx.subscribe(&ctx_input, |this, _state, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
          this.schedule_auto_lint(cx);
        }
      })
      .detach();
      cx.subscribe(&df_input, |this, _state, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
          this.schedule_auto_lint(cx);
        }
      })
      .detach();
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

    let lint_banner: Option<gpui::Div> = if self.lint_loading {
      Some(
        div()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Running hadolint..."),
      )
    } else {
      match self.lint_result.as_ref() {
        Some(Err(err)) if err == ERR_HADOLINT_NOT_INSTALLED => {
          Some(div().w_full().child(render_install_hint(&hadolint_install_hint(), cx)))
        }
        Some(Err(err)) => Some(div().w_full().child(render_error_panel("Lint failed", err, &colors))),
        Some(Ok(report)) => {
          let path = self.lint_dockerfile_path.clone().unwrap_or_else(|| "Dockerfile".to_string());
          let report_clone = report.clone();
          let path_clone = path.clone();
          let summary_color = if report.error > 0 {
            colors.danger
          } else if report.warning > 0 {
            colors.warning
          } else {
            colors.success
          };
          let summary_text = if report.findings.is_empty() {
            format!("Hadolint: clean — {path}")
          } else {
            format!(
              "Hadolint: {} error / {} warning / {} info / {} style",
              report.error, report.warning, report.info, report.style
            )
          };
          Some(
            h_flex()
              .w_full()
              .px(px(16.))
              .py(px(8.))
              .gap(px(8.))
              .items_center()
              .border_b_1()
              .border_color(colors.border)
              .bg(summary_color.opacity(0.08))
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .text_color(summary_color)
                  .child(summary_text),
              )
              .child(
                Button::new("lint-view")
                  .label("View report")
                  .ghost()
                  .small()
                  .on_click(move |_ev, window, cx| {
                    crate::ui::dialogs::open_lint_report_dialog(path_clone.clone(), report_clone.clone(), window, cx);
                  }),
              ),
          )
        }
        None => None,
      }
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
      .when_some(lint_banner, ParentElement::child)
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
