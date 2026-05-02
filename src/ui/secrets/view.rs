//! K8s Secrets list view with reveal-on-demand entry preview.

use std::collections::HashSet;
use std::time::Duration;

use gpui::{Context, Entity, Render, SharedString, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  menu::{DropdownMenu, PopupMenuItem},
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::kubernetes::SecretInfo;
use crate::services;
use crate::state::{DockerState, LoadState, StateChanged, docker_state, settings_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub struct SecretsView {
  docker_state: Entity<DockerState>,
  /// (name, namespace) of expanded row showing entries.
  expanded: Option<(String, String)>,
  /// Pre-loaded entries for the expanded row.
  entries: Vec<(String, String)>,
  /// Per-key reveal toggle for expanded row.
  revealed: HashSet<String>,
}

impl SecretsView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| match event {
      StateChanged::SecretsUpdated | StateChanged::NamespacesUpdated => cx.notify(),
      StateChanged::SecretEntriesLoaded {
        name,
        namespace,
        entries,
      } => {
        if this
          .expanded
          .as_ref()
          .is_some_and(|(n, ns)| n == name && ns == namespace)
        {
          this.entries.clone_from(entries);
          cx.notify();
        }
      }
      _ => {}
    })
    .detach();

    // Background refresh while view alive.
    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(services::refresh_secrets);
      }
    })
    .detach();

    services::refresh_secrets(cx);
    services::refresh_namespaces(cx);

    Self {
      docker_state,
      expanded: None,
      entries: Vec::new(),
      revealed: HashSet::new(),
    }
  }

  fn toggle_expand(&mut self, secret: &SecretInfo, cx: &mut Context<'_, Self>) {
    let key = (secret.name.clone(), secret.namespace.clone());
    if self.expanded.as_ref() == Some(&key) {
      self.expanded = None;
      self.entries.clear();
      self.revealed.clear();
    } else {
      self.expanded = Some(key);
      self.entries.clear();
      self.revealed.clear();
      services::load_secret_entries(secret.name.clone(), secret.namespace.clone(), cx);
    }
    cx.notify();
  }

  fn toggle_reveal(&mut self, key: &str, cx: &mut Context<'_, Self>) {
    if !self.revealed.remove(key) {
      self.revealed.insert(key.to_string());
    }
    cx.notify();
  }

  fn render_row(&self, secret: &SecretInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let expanded = self
      .expanded
      .as_ref()
      .is_some_and(|(n, ns)| n == &secret.name && ns == &secret.namespace);
    let key_count = secret.keys.len();
    let name = secret.name.clone();
    let namespace = secret.namespace.clone();
    let secret_clone = secret.clone();

    let row_header = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(8.))
      .gap(px(8.))
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(
        Button::new(SharedString::from(format!("expand-{name}-{namespace}")))
          .icon(if expanded {
            IconName::ChevronDown
          } else {
            IconName::ChevronRight
          })
          .ghost()
          .xsmall()
          .on_click(cx.listener(move |this, _, _, cx| {
            this.toggle_expand(&secret_clone, cx);
          })),
      )
      .child(
        div()
          .flex_1()
          .text_sm()
          .text_color(colors.foreground)
          .child(secret.name.clone()),
      )
      .child(
        div()
          .w(px(140.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(secret.namespace.clone()),
      )
      .child(
        div()
          .w(px(180.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(secret.secret_type.clone()),
      )
      .child(
        div()
          .w(px(60.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(format!("{key_count} keys")),
      )
      .child(
        div()
          .w(px(60.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(secret.age.clone()),
      )
      .child(
        Button::new(SharedString::from(format!("menu-{name}-{namespace}")))
          .icon(IconName::Ellipsis)
          .ghost()
          .xsmall()
          .dropdown_menu({
            let n = name.clone();
            let ns = namespace.clone();
            move |menu, _, _| {
              menu.item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click({
                let n = n.clone();
                let ns = ns.clone();
                move |_, _, cx| {
                  services::delete_secret(n.clone(), ns.clone(), cx);
                }
              }))
            }
          }),
      );

    let mut row = v_flex().w_full().child(row_header);
    if expanded {
      row = row.child(self.render_entries(cx));
    }
    row
  }

  fn render_entries(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    if self.entries.is_empty() {
      return div()
        .px(px(12.))
        .py(px(8.))
        .text_xs()
        .text_color(colors.muted_foreground)
        .child("Loading entries…");
    }

    let mut col = v_flex().w_full().gap(px(2.)).py(px(6.)).bg(colors.sidebar);
    for (k, v) in &self.entries {
      let revealed = self.revealed.contains(k);
      let key_for_btn = k.clone();
      col = col.child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(4.))
          .gap(px(8.))
          .items_start()
          .child(
            div()
              .w(px(180.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.foreground)
              .child(k.clone()),
          )
          .child(
            div()
              .flex_1()
              .text_xs()
              .font_family("monospace")
              .text_color(colors.foreground)
              .whitespace_normal()
              .child(if revealed {
                v.clone()
              } else {
                "•".repeat(v.len().min(40))
              }),
          )
          .child(
            Button::new(SharedString::from(format!("reveal-{key_for_btn}")))
              .label(if revealed { "Hide" } else { "Reveal" })
              .ghost()
              .xsmall()
              .on_click(cx.listener(move |this, _, _, cx| this.toggle_reveal(&key_for_btn, cx))),
          )
          .child(
            Button::new(SharedString::from(format!("copy-{k}")))
              .icon(Icon::new(AppIcon::Copy))
              .ghost()
              .xsmall()
              .on_click({
                let v = v.clone();
                move |_, _, cx| {
                  cx.write_to_clipboard(gpui::ClipboardItem::new_string(v.clone()));
                }
              }),
          ),
      );
    }
    col
  }
}

impl Render for SecretsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let secrets = state.secrets.clone();
    let secrets_state = state.secrets_state.clone();
    let count = secrets.len();

    let selected_ns = state.selected_namespace.clone();
    let namespaces = state.namespaces.clone();
    let ns_label = if selected_ns == "all" {
      "All".to_string()
    } else {
      selected_ns
    };

    let toolbar = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .child(
        v_flex().child(Label::new("Secrets")).child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(format!("{count} items")),
        ),
      )
      .child(
        h_flex()
          .gap(px(8.))
          .items_center()
          .child(
            Button::new("namespace-selector")
              .label(ns_label)
              .ghost()
              .compact()
              .dropdown_menu(move |menu, _w, _cx| {
                let mut menu = menu.item(PopupMenuItem::new("All Namespaces").on_click(|_, _, cx| {
                  services::set_namespace("all".to_string(), cx);
                }));
                if !namespaces.is_empty() {
                  menu = menu.separator();
                  for ns in &namespaces {
                    let ns = ns.clone();
                    menu = menu.item(PopupMenuItem::new(ns.clone()).on_click(move |_, _, cx| {
                      services::set_namespace(ns.clone(), cx);
                    }));
                  }
                }
                menu
              }),
          )
          .child(
            Button::new("refresh")
              .icon(Icon::new(AppIcon::Refresh))
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, _w, cx| services::refresh_secrets(cx))),
          ),
      );

    let body: gpui::Div = match &secrets_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("secrets", cx),
      LoadState::Error(e) => render_k8s_error("secrets", &e.clone(), |_ev, _w, cx| services::refresh_secrets(cx), cx),
      LoadState::Loaded => {
        if secrets.is_empty() {
          div().size_full().flex().items_center().justify_center().child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No secrets in selected namespace."),
          )
        } else {
          let mut list = v_flex().w_full();
          // header row
          list = list.child(
            h_flex()
              .w_full()
              .px(px(12.))
              .py(px(6.))
              .gap(px(8.))
              .bg(colors.muted)
              .child(div().w(px(28.)))
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("NAME"),
              )
              .child(
                div()
                  .w(px(140.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("NAMESPACE"),
              )
              .child(
                div()
                  .w(px(180.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("TYPE"),
              )
              .child(
                div()
                  .w(px(60.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("KEYS"),
              )
              .child(
                div()
                  .w(px(60.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("AGE"),
              )
              .child(div().w(px(28.))),
          );
          for s in &secrets {
            list = list.child(self.render_row(s, cx));
          }
          div().w_full().child(list)
        }
      }
    };

    v_flex().size_full().child(toolbar).child(
      div()
        .id("secrets-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .child(body),
    )
  }
}
