//! Reusable inline create-form for resources that take a `name`, optional
//! `namespace`, and a list of `(key, value)` entries (Secrets, `ConfigMaps`).

use gpui::{App, Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;

/// Per-entry input pair (key + value editor). Held in the parent so the form
/// remains stateful across re-renders.
pub struct KvFormEntry {
  pub key: Entity<InputState>,
  pub value: Entity<InputState>,
}

/// Backing state for a `(name, namespace, entries)` create form.
pub struct KvFormState {
  pub name: Entity<InputState>,
  pub namespace: Entity<InputState>,
  pub entries: Vec<KvFormEntry>,
  pub value_multiline: bool,
}

impl KvFormState {
  /// Build a fresh form. `default_namespace` seeds the namespace input — the
  /// caller typically passes the currently selected namespace, replacing
  /// `"all"` with `"default"`.
  pub fn new(default_namespace: &str, value_multiline: bool, window: &mut Window, cx: &mut App) -> Self {
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("name"));
    let namespace = {
      let ns_default = if default_namespace == "all" {
        "default".to_string()
      } else {
        default_namespace.to_string()
      };
      cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("namespace");
        state.set_value(ns_default, window, cx);
        state
      })
    };
    let mut form = Self {
      name,
      namespace,
      entries: Vec::new(),
      value_multiline,
    };
    form.add_row(window, cx);
    form
  }

  pub fn add_row(&mut self, window: &mut Window, cx: &mut App) {
    let key = cx.new(|cx| InputState::new(window, cx).placeholder("key"));
    let value = if self.value_multiline {
      cx.new(|cx| InputState::new(window, cx).multi_line(true).placeholder("value"))
    } else {
      cx.new(|cx| InputState::new(window, cx).placeholder("value"))
    };
    self.entries.push(KvFormEntry { key, value });
  }

  pub fn remove_row(&mut self, idx: usize) {
    if idx < self.entries.len() {
      self.entries.remove(idx);
    }
  }

  pub fn name_value(&self, cx: &App) -> String {
    self.name.read(cx).text().to_string().trim().to_string()
  }

  pub fn namespace_value(&self, cx: &App) -> String {
    let ns = self.namespace.read(cx).text().to_string().trim().to_string();
    if ns.is_empty() { "default".to_string() } else { ns }
  }

  /// Collect non-empty key/value pairs.
  pub fn collect(&self, cx: &App) -> Vec<(String, String)> {
    self
      .entries
      .iter()
      .filter_map(|e| {
        let k = e.key.read(cx).text().to_string();
        if k.trim().is_empty() {
          return None;
        }
        let v = e.value.read(cx).text().to_string();
        Some((k, v))
      })
      .collect()
  }
}

/// Render the form. `on_save` receives the collected entries and is called
/// when the user clicks Create. `on_cancel` is invoked from the Cancel button.
pub fn render_kv_create_form<P>(
  state: &KvFormState,
  title: &str,
  on_add_row: impl Fn(&mut P, &mut Window, &mut Context<'_, P>) + 'static,
  on_remove_row: impl Fn(&mut P, usize, &mut Context<'_, P>) + 'static + Clone,
  on_save: impl Fn(&mut P, &mut Window, &mut Context<'_, P>) + 'static,
  on_cancel: impl Fn(&mut P, &mut Window, &mut Context<'_, P>) + 'static,
  cx: &mut Context<'_, P>,
) -> gpui::Div
where
  P: Render,
{
  let colors = &cx.theme().colors;

  let header = h_flex()
    .w_full()
    .items_center()
    .justify_between()
    .child(
      div()
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.foreground)
        .child(title.to_string()),
    )
    .child(
      h_flex()
        .gap(px(6.))
        .child(
          Button::new("kv-form-save")
            .label("Create")
            .primary()
            .small()
            .on_click(cx.listener(move |this, _ev, w, cx| on_save(this, w, cx))),
        )
        .child(
          Button::new("kv-form-cancel")
            .label("Cancel")
            .ghost()
            .small()
            .on_click(cx.listener(move |this, _ev, w, cx| on_cancel(this, w, cx))),
        ),
    );

  let identity = h_flex()
    .w_full()
    .gap(px(8.))
    .child(div().flex_1().child(Input::new(&state.name).w_full().small()))
    .child(div().w(px(180.)).child(Input::new(&state.namespace).w_full().small()));

  let mut entries_block = v_flex().w_full().gap(px(6.));
  for (idx, entry) in state.entries.iter().enumerate() {
    let remove = on_remove_row.clone();
    entries_block = entries_block.child(
      h_flex()
        .w_full()
        .gap(px(6.))
        .items_start()
        .child(
          div()
            .w(px(180.))
            .flex_shrink_0()
            .child(Input::new(&entry.key).w_full().small()),
        )
        .child(div().flex_1().child(Input::new(&entry.value).w_full().small()))
        .child(
          Button::new(SharedString::from(format!("kv-remove-{idx}")))
            .icon(Icon::new(AppIcon::Trash))
            .ghost()
            .xsmall()
            .on_click(cx.listener(move |this, _ev, _w, cx| remove(this, idx, cx))),
        ),
    );
  }

  let footer = h_flex().w_full().child(
    Button::new("kv-add-row")
      .label("Add entry")
      .icon(IconName::Plus)
      .ghost()
      .small()
      .on_click(cx.listener(move |this, _ev, w, cx| on_add_row(this, w, cx))),
  );

  v_flex()
    .w_full()
    .p(px(12.))
    .gap(px(8.))
    .bg(colors.sidebar)
    .border_b_1()
    .border_color(colors.border)
    .child(header)
    .child(identity)
    .child(entries_block)
    .child(footer)
}
