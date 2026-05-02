//! AI Models view — wraps `colima model` subcommand.

use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Disableable, Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::colima::{ColimaClient, ModelRunner};
use crate::state::{DockerState, docker_state};
use crate::terminal::{TerminalSessionType, TerminalView};

/// Top-level Models view. Holds inputs + last-command output.
pub struct ModelsView {
  _docker_state: Entity<DockerState>,
  runner: ModelRunner,
  name_input: Entity<InputState>,
  port_input: Entity<InputState>,
  output: String,
  busy: bool,
  /// Active `colima model serve` terminal session (one at a time).
  serve_terminal: Option<Entity<TerminalView>>,
}

impl ModelsView {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("ai/smollm2 (or hf.co/…, ollama://…)"));
    let port_input = cx.new(|cx| InputState::new(window, cx).default_value("8080"));
    Self {
      _docker_state: docker_state(cx),
      runner: ModelRunner::default(),
      name_input,
      port_input,
      output: String::new(),
      busy: false,
      serve_terminal: None,
    }
  }

  fn set_output(&mut self, msg: impl Into<String>, cx: &mut Context<'_, Self>) {
    self.output = msg.into();
    self.busy = false;
    cx.notify();
  }

  fn run_blocking<F>(&mut self, label: &str, op: F, cx: &mut Context<'_, Self>)
  where
    F: FnOnce(ModelRunner) -> anyhow::Result<String> + Send + 'static,
  {
    self.busy = true;
    self.output = format!("Running: {label}…");
    cx.notify();

    let runner = self.runner;
    cx.spawn(async move |this, cx| {
      let result = cx.background_executor().spawn(async move { op(runner) }).await;
      let _ = this.update(cx, |this, cx| match result {
        Ok(out) => this.set_output(out, cx),
        Err(e) => this.set_output(format!("Error: {e}"), cx),
      });
    })
    .detach();
  }

  fn on_setup(&mut self, cx: &mut Context<'_, Self>) {
    self.run_blocking("colima model setup", |r| ColimaClient::model_setup(None, r), cx);
  }

  fn on_refresh_list(&mut self, cx: &mut Context<'_, Self>) {
    self.run_blocking("colima model list", |r| ColimaClient::model_list(None, r), cx);
  }

  fn on_pull(&mut self, cx: &mut Context<'_, Self>) {
    let name = self.name_input.read(cx).text().to_string();
    if name.trim().is_empty() {
      self.set_output("Enter a model name first (e.g. ai/smollm2).", cx);
      return;
    }
    self.run_blocking(
      &format!("colima model pull {name}"),
      move |r| ColimaClient::model_pull(None, r, &name),
      cx,
    );
  }

  fn on_serve(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let name = self.name_input.read(cx).text().to_string();
    let port_text = self.port_input.read(cx).text().to_string();
    if name.trim().is_empty() {
      self.set_output("Enter a model name to serve.", cx);
      return;
    }
    let Ok(port) = port_text.trim().parse::<u16>() else {
      self.set_output(format!("Port '{port_text}' is not a valid u16."), cx);
      return;
    };

    let args = ColimaClient::model_serve_args(None, self.runner, &name, port);
    if args.is_empty() {
      self.set_output("colima model serve is not available on this platform.", cx);
      return;
    }

    // Spawn `colima model serve …` in a terminal session so the user
    // sees streaming logs and can stop it with Ctrl-C.
    let session = TerminalSessionType::custom_command("colima".to_string(), args);
    self.serve_terminal = Some(cx.new(|cx| TerminalView::new(session, window, cx)));
    self.output = format!("Serving {name} on http://localhost:{port}. Open in browser when ready.");
    cx.notify();
  }

  fn on_open_browser(&self, cx: &mut App) {
    let port_text = self.port_input.read(cx).text().to_string();
    let Ok(port) = port_text.trim().parse::<u16>() else {
      return;
    };
    cx.open_url(&format!("http://localhost:{port}"));
  }

  fn on_select_runner(&mut self, runner: ModelRunner, cx: &mut Context<'_, Self>) {
    self.runner = runner;
    cx.notify();
  }
}

impl Render for ModelsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let runner = self.runner;
    let busy = self.busy;

    let header = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .child(
        v_flex().child(Label::new("AI Models")).child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child("colima model — requires krunkit VM type"),
        ),
      )
      .child(
        h_flex()
          .gap(px(4.))
          .child(
            Button::new("runner-docker")
              .label("Docker Model Runner")
              .small()
              .when(runner == ModelRunner::Docker, ButtonVariants::primary)
              .when(runner != ModelRunner::Docker, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _w, cx| {
                this.on_select_runner(ModelRunner::Docker, cx);
              })),
          )
          .child(
            Button::new("runner-ramalama")
              .label("Ramalama")
              .small()
              .when(runner == ModelRunner::Ramalama, ButtonVariants::primary)
              .when(runner != ModelRunner::Ramalama, ButtonVariants::ghost)
              .on_click(cx.listener(|this, _ev, _w, cx| {
                this.on_select_runner(ModelRunner::Ramalama, cx);
              })),
          ),
      );

    let inputs = v_flex()
      .gap(px(8.))
      .px(px(16.))
      .py(px(12.))
      .child(
        h_flex()
          .gap(px(8.))
          .items_center()
          .child(
            div()
              .w(px(80.))
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("Model"),
          )
          .child(div().flex_1().child(Input::new(&self.name_input).w_full())),
      )
      .child(
        h_flex()
          .gap(px(8.))
          .items_center()
          .child(
            div()
              .w(px(80.))
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("Port"),
          )
          .child(div().w(px(120.)).child(Input::new(&self.port_input).w_full())),
      )
      .child(
        h_flex()
          .gap(px(8.))
          .child(
            Button::new("model-setup")
              .label("Setup Runner")
              .icon(IconName::Settings)
              .ghost()
              .small()
              .on_click(cx.listener(|this, _ev, _w, cx| this.on_setup(cx))),
          )
          .child(
            Button::new("model-pull")
              .label("Pull")
              .icon(Icon::new(AppIcon::Plus))
              .small()
              .when(busy, |b| b.disabled(true))
              .on_click(cx.listener(|this, _ev, _w, cx| this.on_pull(cx))),
          )
          .child(
            Button::new("model-list")
              .label("Refresh List")
              .icon(Icon::new(AppIcon::Refresh))
              .ghost()
              .small()
              .on_click(cx.listener(|this, _ev, _w, cx| this.on_refresh_list(cx))),
          )
          .child(
            Button::new("model-serve")
              .label("Serve")
              .icon(Icon::new(AppIcon::Play))
              .primary()
              .small()
              .on_click(cx.listener(|this, _ev, w, cx| this.on_serve(w, cx))),
          )
          .child(
            Button::new("model-open")
              .label("Open Browser")
              .icon(IconName::ExternalLink)
              .ghost()
              .small()
              .on_click(cx.listener(|this, _ev, _w, cx| this.on_open_browser(cx))),
          ),
      );

    let body = if let Some(term) = self.serve_terminal.clone() {
      div().flex_1().min_h_0().child(term).into_any_element()
    } else {
      div()
        .flex_1()
        .min_h_0()
        .p(px(16.))
        .child(
          div()
            .w_full()
            .h_full()
            .p(px(12.))
            .rounded(px(6.))
            .bg(colors.sidebar)
            .font_family("monospace")
            .text_xs()
            .text_color(colors.foreground)
            .whitespace_normal()
            .child(if self.output.is_empty() {
              "Run Setup, Pull, Refresh List, or Serve. Output appears here.".to_string()
            } else {
              self.output.clone()
            }),
        )
        .into_any_element()
    };

    v_flex().size_full().child(header).child(inputs).child(body)
  }
}
