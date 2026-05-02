use gpui::{App, Context, Entity, FocusHandle, Focusable, Hsla, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Sizable, h_flex,
  input::{Input, InputState},
  label::Label,
  theme::ActiveTheme,
  v_flex,
};

#[derive(Debug, Clone, Default)]
pub struct PushImageOptions {
  pub image: String,
  pub tag: String,
  pub username: Option<String>,
  pub password: Option<String>,
}

pub struct PushImageDialog {
  focus_handle: FocusHandle,
  default_image: String,
  default_tag: String,
  image_input: Option<Entity<InputState>>,
  tag_input: Option<Entity<InputState>>,
  username_input: Option<Entity<InputState>>,
  password_input: Option<Entity<InputState>>,
}

impl PushImageDialog {
  pub fn new(image: String, tag: String) -> impl FnOnce(&mut Context<'_, Self>) -> Self {
    move |cx| Self {
      focus_handle: cx.focus_handle(),
      default_image: image,
      default_tag: tag,
      image_input: None,
      tag_input: None,
      username_input: None,
      password_input: None,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let default_image = self.default_image.clone();
    let default_tag = self.default_tag.clone();
    if self.image_input.is_none() {
      self.image_input = Some(cx.new(|cx| InputState::new(window, cx).default_value(default_image)));
    }
    if self.tag_input.is_none() {
      self.tag_input = Some(cx.new(|cx| InputState::new(window, cx).default_value(default_tag)));
    }
    if self.username_input.is_none() {
      self.username_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("username (optional)")));
    }
    if self.password_input.is_none() {
      self.password_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("password (optional)").masked(true)));
    }
  }

  pub fn get_options(&self, cx: &App) -> PushImageOptions {
    let read = |opt: &Option<Entity<InputState>>| {
      opt
        .as_ref()
        .map(|s| s.read(cx).text().to_string())
        .unwrap_or_default()
    };
    let image = read(&self.image_input);
    let tag = read(&self.tag_input);
    let username = read(&self.username_input);
    let password = read(&self.password_input);
    let auth_username = if username.is_empty() { None } else { Some(username) };
    let auth_password = if password.is_empty() { None } else { Some(password) };
    PushImageOptions {
      image,
      tag,
      username: auth_username,
      password: auth_password,
    }
  }
}

impl Focusable for PushImageDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for PushImageDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);
    let colors = cx.theme().colors;
    let image_input = self.image_input.clone().unwrap();
    let tag_input = self.tag_input.clone().unwrap();
    let username_input = self.username_input.clone().unwrap();
    let password_input = self.password_input.clone().unwrap();

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
          .child("Push the image to its registry. Leave auth blank to use the daemon's stored credentials."),
      )
      .child(row(
        "Image",
        div().w(px(320.)).child(Input::new(&image_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Tag",
        div().w(px(160.)).child(Input::new(&tag_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Username",
        div().w(px(220.)).child(Input::new(&username_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(row(
        "Password",
        div().w(px(220.)).child(Input::new(&password_input).small()).into_any_element(),
        colors.border,
        colors.foreground,
      ))
  }
}
