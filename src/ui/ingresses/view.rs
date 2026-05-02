//! K8s Ingress list view.

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
use crate::services;
use crate::state::{DockerState, LoadState, StateChanged, docker_state, settings_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub struct IngressesView {
  docker_state: Entity<DockerState>,
}

impl IngressesView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::IngressesUpdated | StateChanged::NamespacesUpdated) {
        cx.notify();
      }
    })
    .detach();

    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(services::refresh_ingresses);
      }
    })
    .detach();

    services::refresh_ingresses(cx);
    services::refresh_namespaces(cx);

    Self { docker_state }
  }
}

impl Render for IngressesView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let items = state.ingresses.clone();
    let load_state = state.ingresses_state.clone();
    let count = items.len();

    let toolbar = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .child(
        v_flex().child(Label::new("Ingresses")).child(
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
          .on_click(cx.listener(|_this, _ev, _w, cx| services::refresh_ingresses(cx))),
      );

    let body: gpui::Div = match &load_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("ingresses", cx),
      LoadState::Error(e) => render_k8s_error(
        "ingresses",
        &e.clone(),
        |_ev, _w, cx| services::refresh_ingresses(cx),
        cx,
      ),
      LoadState::Loaded => {
        if items.is_empty() {
          div().size_full().flex().items_center().justify_center().child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No ingresses in selected namespace."),
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
                  .w(px(120.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("CLASS"),
              )
              .child(
                div()
                  .flex_1()
                  .min_w_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("HOSTS"),
              )
              .child(
                div()
                  .w(px(160.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("ADDRESS"),
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
          for i in &items {
            let name = i.name.clone();
            let namespace = i.namespace.clone();
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
                    .child(i.name.clone()),
                )
                .child(
                  div()
                    .w(px(140.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(i.namespace.clone()),
                )
                .child(
                  div()
                    .w(px(120.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(i.class_name.clone().unwrap_or_else(|| "—".to_string())),
                )
                .child(
                  div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .text_color(colors.foreground)
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(if i.hosts.is_empty() {
                      "—".to_string()
                    } else {
                      i.hosts.join(", ")
                    }),
                )
                .child(
                  div()
                    .w(px(160.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(if i.addresses.is_empty() {
                      "—".to_string()
                    } else {
                      i.addresses.join(", ")
                    }),
                )
                .child(
                  div()
                    .w(px(60.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(i.age.clone()),
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
                            services::delete_ingress(delete_name.clone(), delete_namespace.clone(), cx);
                          },
                        ))
                      }
                    }),
                ),
            );
          }
          div().w_full().child(list)
        }
      }
    };

    v_flex().size_full().child(toolbar).child(
      div()
        .id("ingresses-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .child(body),
    )
  }
}
