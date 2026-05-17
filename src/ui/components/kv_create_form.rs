//! Modal create-form for resources that take a `name`, optional
//! `namespace`, and a list of `(key, value)` entries (Secrets, `ConfigMaps`).

use gpui::{App, Context, Entity, FocusHandle, Focusable, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::ui::components::{form_field, form_section};

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

/// Render just the form fields (name/namespace + key/value rows + an
/// "Add entry" button). No header or Create/Cancel — those belong to the
/// surrounding modal dialog frame.
fn render_kv_fields<P>(
  state: &KvFormState,
  on_add_row: impl Fn(&mut P, &mut Window, &mut Context<'_, P>) + 'static,
  on_remove_row: impl Fn(&mut P, usize, &mut Context<'_, P>) + 'static + Clone,
  cx: &mut Context<'_, P>,
) -> gpui::Div
where
  P: Render,
{
  let colors = cx.theme().colors;

  let identity = v_flex()
    .w_full()
    .gap(px(12.))
    .child(form_field("Name", Input::new(&state.name).w_full().small(), None, cx))
    .child(form_field(
      "Namespace",
      Input::new(&state.namespace).w_full().small(),
      None,
      cx,
    ));

  // One header row of column labels rather than repeating a label on
  // every entry — reads as a proper key/value table.
  let col_headers = h_flex()
    .w_full()
    .gap(px(8.))
    .child(
      div()
        .w(px(220.))
        .flex_shrink_0()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.muted_foreground)
        .child("Key"),
    )
    .child(
      div()
        .flex_1()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.muted_foreground)
        .child("Value"),
    )
    .child(div().w(px(28.)).flex_shrink_0());

  let mut rows = v_flex().w_full().gap(px(6.)).child(col_headers);
  for (idx, entry) in state.entries.iter().enumerate() {
    let remove = on_remove_row.clone();
    rows = rows.child(
      h_flex()
        .w_full()
        .gap(px(8.))
        .items_center()
        .child(
          div()
            .w(px(220.))
            .flex_shrink_0()
            .child(Input::new(&entry.key).w_full().small()),
        )
        .child(div().flex_1().child(Input::new(&entry.value).w_full().small()))
        .child(
          div().w(px(28.)).flex_shrink_0().child(
            Button::new(SharedString::from(format!("kv-remove-{idx}")))
              .icon(Icon::new(AppIcon::Trash))
              .ghost()
              .xsmall()
              .on_click(cx.listener(move |this, _ev, _w, cx| remove(this, idx, cx))),
          ),
        ),
    );
  }

  let add = h_flex().w_full().child(
    Button::new("kv-add-row")
      .label("Add entry")
      .icon(IconName::Plus)
      .ghost()
      .small()
      .on_click(cx.listener(move |this, _ev, w, cx| on_add_row(this, w, cx))),
  );

  v_flex()
    .w_full()
    .p(px(16.))
    .gap(px(14.))
    .child(identity)
    .child(form_section("Data", cx))
    .child(rows)
    .child(add)
}

/// Which resource the KV create dialog builds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KvResourceKind {
  Secret,
  ConfigMap,
}

/// `(kind, name, namespace, entries)` collected from the create form.
pub type KvCreateValues = (KvResourceKind, String, String, Vec<(String, String)>);

/// Modal dialog body for creating a Secret / `ConfigMap`. The Create /
/// Cancel buttons live in the dialog footer (see `dialogs::open_*`); this
/// renders only the scrollable field area, matching every other create
/// dialog in the app.
pub struct KvCreateDialog {
  focus_handle: FocusHandle,
  kind: KvResourceKind,
  default_ns: String,
  form: Option<KvFormState>,
}

impl KvCreateDialog {
  pub fn new(kind: KvResourceKind, default_ns: String, cx: &mut Context<'_, Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      kind,
      default_ns,
      form: None,
    }
  }

  fn ensure(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.form.is_none() {
      // Secrets often hold multi-line PEM/cert blobs — give them a
      // multi-line value editor; ConfigMaps stay single-line.
      let multiline = self.kind == KvResourceKind::Secret;
      self.form = Some(KvFormState::new(&self.default_ns, multiline, window, cx));
    }
  }

  /// Collected form values, or `None` when the name is empty (keep the
  /// dialog open so the user can fix it). Returns owned data so the
  /// `cx` read-borrow is released before the caller invokes a service.
  pub fn values(&self, cx: &App) -> Option<KvCreateValues> {
    let form = self.form.as_ref()?;
    let name = form.name_value(cx);
    if name.is_empty() {
      return None;
    }
    Some((self.kind, name, form.namespace_value(cx), form.collect(cx)))
  }
}

impl Focusable for KvCreateDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for KvCreateDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure(window, cx);
    let Some(form) = self.form.as_ref() else {
      return div();
    };
    // SAFETY of borrow: render_kv_fields only reads `form`'s input
    // entities; the listeners re-borrow `self.form` mutably at click time.
    let fields = render_kv_fields(
      form,
      |this: &mut Self, w, cx| {
        if let Some(f) = this.form.as_mut() {
          f.add_row(w, cx);
          cx.notify();
        }
      },
      |this: &mut Self, idx, cx| {
        if let Some(f) = this.form.as_mut() {
          f.remove_row(idx);
          cx.notify();
        }
      },
      cx,
    );
    div().w_full().child(fields)
  }
}
