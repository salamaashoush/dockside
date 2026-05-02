use gpui::{App, Context, Entity, FocusHandle, Focusable, Hsla, Render, Styled, Task, Window, div, prelude::*, px};
use gpui_component::{
  Sizable,
  button::ButtonVariants,
  h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};

use crate::docker::RegistrySearchResult;
use crate::services;

pub struct RegistryBrowserDialog {
  focus_handle: FocusHandle,
  query_input: Option<Entity<InputState>>,
  results: Vec<RegistrySearchResult>,
  loading: bool,
  error: Option<String>,
  search_task: Option<Task<()>>,
}

impl RegistryBrowserDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    Self {
      focus_handle,
      query_input: None,
      results: Vec::new(),
      loading: false,
      error: None,
      search_task: None,
    }
  }

  fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.query_input.is_none() {
      self.query_input = Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search Docker Hub")));
    }
  }

  fn run_search(&mut self, cx: &mut Context<'_, Self>) {
    let Some(input) = self.query_input.clone() else {
      return;
    };
    let query = input.read(cx).text().to_string();
    if query.trim().is_empty() {
      return;
    }
    self.results.clear();
    self.error = None;
    self.loading = true;
    self.search_task = None;

    let tokio_handle = services::Tokio::runtime_handle();
    let client = services::docker_client();
    let task = cx.spawn(async move |this, cx| {
      let result = cx
        .background_executor()
        .spawn(async move {
          tokio_handle.block_on(async {
            let guard = client.read().await;
            match guard.as_ref() {
              Some(c) => c.search_images(&query, 25).await,
              None => Err(anyhow::anyhow!("Docker client not connected")),
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.loading = false;
        match result {
          Ok(items) => {
            this.results = items;
            this.error = None;
          }
          Err(e) => {
            this.results.clear();
            this.error = Some(e.to_string());
          }
        }
        cx.notify();
      });
    });
    self.search_task = Some(task);
  }
}

impl Focusable for RegistryBrowserDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for RegistryBrowserDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_inputs(window, cx);
    let colors = cx.theme().colors;
    let query_input = self.query_input.clone().unwrap();

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

    let body: gpui::AnyElement = if self.loading {
      div()
        .w_full()
        .px(px(16.))
        .py(px(16.))
        .text_sm()
        .text_color(colors.muted_foreground)
        .child("Searching...")
        .into_any_element()
    } else if let Some(err) = self.error.as_ref() {
      div()
        .w_full()
        .px(px(16.))
        .py(px(16.))
        .text_sm()
        .text_color(colors.danger)
        .child(err.clone())
        .into_any_element()
    } else if self.results.is_empty() {
      div()
        .w_full()
        .px(px(16.))
        .py(px(16.))
        .text_sm()
        .text_color(colors.muted_foreground)
        .child("Enter a search term and press Search.")
        .into_any_element()
    } else {
      let rows = self.results.iter().enumerate().map(|(i, r)| {
        let zebra = if i % 2 == 0 {
          colors.background
        } else {
          colors.muted.opacity(0.4)
        };
        let name = r.name.clone();
        let pulled = name.clone();
        let stars = r.stars;
        let official = r.official;
        let description = r.description.clone();
        h_flex()
          .w_full()
          .px(px(12.))
          .py(px(8.))
          .gap(px(8.))
          .bg(zebra)
          .child(
            v_flex()
              .flex_1()
              .gap(px(2.))
              .child(
                h_flex()
                  .gap(px(8.))
                  .items_center()
                  .child(
                    div()
                      .text_sm()
                      .font_family("monospace")
                      .text_color(colors.foreground)
                      .child(name),
                  )
                  .when(official, |el| {
                    el.child(
                      div()
                        .px(px(6.))
                        .py(px(1.))
                        .rounded(px(4.))
                        .bg(colors.primary.opacity(0.15))
                        .text_xs()
                        .text_color(colors.primary)
                        .child("official"),
                    )
                  }),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(description),
              ),
          )
          .child(
            div()
              .w(px(80.))
              .text_xs()
              .text_color(colors.muted_foreground)
              .child(format!("⭐ {stars}")),
          )
          .child(
            gpui_component::button::Button::new(("pull", i))
              .label("Pull")
              .small()
              .ghost()
              .on_click(cx.listener(move |_this, _ev, _window, cx| {
                services::pull_image(pulled.clone(), None, cx);
              })),
          )
      });
      v_flex().w_full().children(rows).into_any_element()
    };

    v_flex()
      .w_full()
      .max_h(px(560.))
      .child(row(
        "Query",
        h_flex()
          .gap(px(8.))
          .child(div().w(px(280.)).child(Input::new(&query_input).small()))
          .child(
            gpui_component::button::Button::new("registry-search")
              .label("Search")
              .primary()
              .small()
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.run_search(cx);
              })),
          )
          .into_any_element(),
        colors.border,
        colors.foreground,
      ))
      .child(div().w_full().overflow_y_scrollbar().child(body))
  }
}
