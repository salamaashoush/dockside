//! K8s `ConfigMaps` list view with inline data preview.

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
use crate::kubernetes::ConfigMapInfo;
use crate::services;
use crate::state::{DockerState, LoadState, StateChanged, docker_state, settings_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub struct ConfigMapsView {
  docker_state: Entity<DockerState>,
  expanded: Option<(String, String)>,
  entries: Vec<(String, String)>,
}

impl ConfigMapsView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| match event {
      StateChanged::ConfigMapsUpdated | StateChanged::NamespacesUpdated => cx.notify(),
      StateChanged::ConfigMapEntriesLoaded {
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

    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(services::refresh_configmaps);
      }
    })
    .detach();

    services::refresh_configmaps(cx);
    services::refresh_namespaces(cx);

    Self {
      docker_state,
      expanded: None,
      entries: Vec::new(),
    }
  }

  fn toggle_expand(&mut self, cm: &ConfigMapInfo, cx: &mut Context<'_, Self>) {
    let key = (cm.name.clone(), cm.namespace.clone());
    if self.expanded.as_ref() == Some(&key) {
      self.expanded = None;
      self.entries.clear();
    } else {
      self.expanded = Some(key);
      self.entries.clear();
      services::load_configmap_entries(cm.name.clone(), cm.namespace.clone(), cx);
    }
    cx.notify();
  }

  fn render_row(&self, cm: &ConfigMapInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let expanded = self
      .expanded
      .as_ref()
      .is_some_and(|(n, ns)| n == &cm.name && ns == &cm.namespace);
    let key_count = cm.keys.len();
    let name = cm.name.clone();
    let namespace = cm.namespace.clone();
    let cm_clone = cm.clone();

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
            this.toggle_expand(&cm_clone, cx);
          })),
      )
      .child(
        div()
          .flex_1()
          .text_sm()
          .text_color(colors.foreground)
          .child(cm.name.clone()),
      )
      .child(
        div()
          .w(px(140.))
          .flex_shrink_0()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(cm.namespace.clone()),
      )
      .child(
        div()
          .w(px(60.))
          .flex_shrink_0()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(format!("{key_count} keys")),
      )
      .child(
        div()
          .w(px(60.))
          .flex_shrink_0()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(cm.age.clone()),
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
                move |_, _, cx| services::delete_configmap(n.clone(), ns.clone(), cx)
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
              .flex_shrink_0()
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.foreground)
              .child(k.clone()),
          )
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_xs()
              .font_family("monospace")
              .text_color(colors.foreground)
              .whitespace_normal()
              .child(v.clone()),
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

impl Render for ConfigMapsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let cms = state.configmaps.clone();
    let cm_state = state.configmaps_state.clone();
    let count = cms.len();

    let toolbar = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .child(
        v_flex().child(Label::new("ConfigMaps")).child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(format!("{count} items")),
        ),
      )
      .child(
        Button::new("refresh")
          .icon(Icon::new(AppIcon::Refresh))
          .ghost()
          .compact()
          .on_click(cx.listener(|_this, _ev, _w, cx| services::refresh_configmaps(cx))),
      );

    let body: gpui::Div = match &cm_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("configmaps", cx),
      LoadState::Error(e) => render_k8s_error(
        "configmaps",
        &e.clone(),
        |_ev, _w, cx| services::refresh_configmaps(cx),
        cx,
      ),
      LoadState::Loaded => {
        if cms.is_empty() {
          div().size_full().flex().items_center().justify_center().child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No configmaps in selected namespace."),
          )
        } else {
          let mut list = v_flex().w_full();
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
                  .min_w_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("NAME"),
              )
              .child(
                div()
                  .w(px(140.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("NAMESPACE"),
              )
              .child(
                div()
                  .w(px(60.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("KEYS"),
              )
              .child(
                div()
                  .w(px(60.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("AGE"),
              )
              .child(div().w(px(28.))),
          );
          for cm in &cms {
            list = list.child(self.render_row(cm, cx));
          }
          div().w_full().child(list)
        }
      }
    };

    v_flex().size_full().child(toolbar).child(
      div()
        .id("configmaps-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .child(body),
    )
  }
}
