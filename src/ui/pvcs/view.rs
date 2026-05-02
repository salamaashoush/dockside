//! K8s `PersistentVolumeClaim` list view.

use std::time::Duration;

use gpui::{Context, Entity, Render, SharedString, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  menu::{DropdownMenu, PopupMenuItem},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::services;
use crate::state::{DockerState, LoadState, StateChanged, docker_state, settings_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub struct PvcsView {
  docker_state: Entity<DockerState>,
}

impl PvcsView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::PvcsUpdated | StateChanged::NamespacesUpdated) {
        cx.notify();
      }
    })
    .detach();

    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(services::refresh_pvcs);
      }
    })
    .detach();

    services::refresh_pvcs(cx);
    services::refresh_namespaces(cx);

    Self { docker_state }
  }
}

impl Render for PvcsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let items = state.pvcs.clone();
    let load_state = state.pvcs_state.clone();
    let count = items.len();

    let _ = count;
    let _ = state;

    let body: gpui::Div = match &load_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("pvcs", cx),
      LoadState::Error(e) => render_k8s_error("pvcs", &e.clone(), |_ev, _w, cx| services::refresh_pvcs(cx), cx),
      LoadState::Loaded => {
        if items.is_empty() {
          div().size_full().flex().items_center().justify_center().child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No PVCs in selected namespace."),
          )
        } else {
          let header = h_flex()
            .w_full()
            .min_w(px(900.))
            .px(px(12.))
            .py(px(6.))
            .gap(px(8.))
            .bg(colors.muted)
            .flex_shrink_0()
            .child(
              div()
                .flex_1()
                .min_w(px(200.))
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
                .w(px(80.))
                .flex_shrink_0()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("STATUS"),
            )
            .child(
              div()
                .w(px(80.))
                .flex_shrink_0()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("CAPACITY"),
            )
            .child(
              div()
                .w(px(120.))
                .flex_shrink_0()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("ACCESS"),
            )
            .child(
              div()
                .w(px(140.))
                .flex_shrink_0()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("STORAGE CLASS"),
            )
            .child(
              div()
                .w(px(60.))
                .flex_shrink_0()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("AGE"),
            )
            .child(div().w(px(28.)));
          let mut rows = v_flex().w_full().min_w(px(900.));
          for p in &items {
            let name = p.name.clone();
            let namespace = p.namespace.clone();
            let status_color = match p.status.as_str() {
              "Bound" => colors.success,
              "Pending" => colors.warning,
              "Lost" => colors.danger,
              _ => colors.muted_foreground,
            };
            rows = rows.child(
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
                    .min_w(px(200.))
                    .text_sm()
                    .text_color(colors.foreground)
                    .text_ellipsis()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(p.name.clone()),
                )
                .child(
                  div()
                    .w(px(140.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(p.namespace.clone()),
                )
                .child(
                  div()
                    .w(px(80.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(status_color)
                    .child(p.status.clone()),
                )
                .child(
                  div()
                    .w(px(80.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(if p.capacity.is_empty() {
                      "—".to_string()
                    } else {
                      p.capacity.clone()
                    }),
                )
                .child(
                  div()
                    .w(px(120.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(if p.access_modes.is_empty() {
                      "—".to_string()
                    } else {
                      p.access_modes.join(",")
                    }),
                )
                .child(
                  div()
                    .w(px(140.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(p.storage_class.clone().unwrap_or_else(|| "—".to_string())),
                )
                .child(
                  div()
                    .w(px(60.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(p.age.clone()),
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
                        let delete_name = n.clone();
                        let delete_namespace = ns.clone();
                        menu.item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click(
                          move |_, _, cx| {
                            services::delete_pvc(delete_name.clone(), delete_namespace.clone(), cx);
                          },
                        ))
                      }
                    }),
                ),
            );
          }
          div().size_full().child(
            div().id("pvcs-h-scroll").size_full().overflow_x_scroll().child(
              v_flex().w_full().min_w(px(900.)).size_full().child(header).child(
                div()
                  .id("pvcs-v-scroll")
                  .flex_1()
                  .min_h_0()
                  .w_full()
                  .overflow_y_scroll()
                  .child(rows),
              ),
            ),
          )
        }
      }
    };

    v_flex().size_full().child(div().flex_1().min_h_0().child(body))
  }
}
