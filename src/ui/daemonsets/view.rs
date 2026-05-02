//! K8s `DaemonSets` list view.

use std::time::Duration;

use gpui::{Context, Entity, Render, SharedString, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  menu::{DropdownMenu, PopupMenuItem},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::services;
use crate::state::{DockerState, LoadState, StateChanged, docker_state, settings_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub struct DaemonSetsView {
  docker_state: Entity<DockerState>,
}

impl DaemonSetsView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::DaemonSetsUpdated | StateChanged::NamespacesUpdated) {
        cx.notify();
      }
    })
    .detach();

    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(services::refresh_daemonsets);
      }
    })
    .detach();

    services::refresh_daemonsets(cx);
    services::refresh_namespaces(cx);

    Self { docker_state }
  }
}

impl Render for DaemonSetsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let items = state.daemonsets.clone();
    let load_state = state.daemonsets_state.clone();
    let count = items.len();

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
        v_flex().child(Label::new("DaemonSets")).child(
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
              .on_click(cx.listener(|_this, _ev, _w, cx| services::refresh_daemonsets(cx))),
          ),
      );

    let body: gpui::Div = match &load_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("daemonsets", cx),
      LoadState::Error(e) => render_k8s_error(
        "daemonsets",
        &e.clone(),
        |_ev, _w, cx| services::refresh_daemonsets(cx),
        cx,
      ),
      LoadState::Loaded => {
        if items.is_empty() {
          div().size_full().flex().items_center().justify_center().child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No daemonsets in selected namespace."),
          )
        } else {
          let mut list = v_flex().w_full().child(
            h_flex()
              .w_full()
              .px(px(12.))
              .py(px(6.))
              .gap(px(8.))
              .bg(colors.muted)
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
                  .w(px(60.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("READY"),
              )
              .child(
                div()
                  .w(px(60.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("AGE"),
              )
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("IMAGE"),
              )
              .child(div().w(px(28.))),
          );
          for d in &items {
            let name = d.name.clone();
            let namespace = d.namespace.clone();
            list = list.child(
              h_flex()
                .w_full()
                .px(px(12.))
                .py(px(8.))
                .gap(px(8.))
                .items_center()
                .border_b_1()
                .border_color(colors.border)
                .child(
                  div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.foreground)
                    .child(d.name.clone()),
                )
                .child(
                  div()
                    .w(px(140.))
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(d.namespace.clone()),
                )
                .child(
                  div()
                    .w(px(60.))
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(d.ready_display()),
                )
                .child(
                  div()
                    .w(px(60.))
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(d.age.clone()),
                )
                .child(
                  div()
                    .flex_1()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(d.images.first().cloned().unwrap_or_default()),
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
                        let restart_name = n.clone();
                        let restart_namespace = ns.clone();
                        let delete_name = n.clone();
                        let delete_namespace = ns.clone();
                        menu
                          .item(
                            PopupMenuItem::new("Rolling Restart")
                              .icon(Icon::new(AppIcon::Restart))
                              .on_click(move |_, _, cx| {
                                services::rollout_restart_daemonset(
                                  restart_name.clone(),
                                  restart_namespace.clone(),
                                  cx,
                                );
                              }),
                          )
                          .separator()
                          .item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click(
                            move |_, _, cx| {
                              services::delete_daemonset(delete_name.clone(), delete_namespace.clone(), cx);
                            },
                          ))
                      }
                    }),
                ),
            );
          }
          div().size_full().overflow_hidden().child(list)
        }
      }
    };

    v_flex().size_full().child(toolbar).child(body)
  }
}
