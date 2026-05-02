//! Cluster overview view: Nodes, Events, Namespaces with create/delete.

use std::time::Duration;

use gpui::{Context, Entity, Render, SharedString, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  menu::{DropdownMenu, PopupMenuItem},
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::services;
use crate::state::{DockerState, LoadState, StateChanged, docker_state, settings_state};
use crate::ui::components::{render_k8s_error, render_loading};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClusterTab {
  #[default]
  Nodes,
  Events,
  Namespaces,
}

pub struct ClusterView {
  docker_state: Entity<DockerState>,
  active_tab: ClusterTab,
  new_namespace_input: Option<Entity<InputState>>,
}

impl ClusterView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::NodesUpdated | StateChanged::EventsUpdated | StateChanged::NamespacesUpdated
      ) {
        cx.notify();
      }
    })
    .detach();

    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(|cx| {
          services::refresh_nodes(cx);
          services::refresh_events(cx);
          services::refresh_namespaces(cx);
        });
      }
    })
    .detach();

    services::refresh_nodes(cx);
    services::refresh_events(cx);
    services::refresh_namespaces(cx);

    Self {
      docker_state,
      active_tab: ClusterTab::Nodes,
      new_namespace_input: None,
    }
  }

  fn render_tab_bar(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let active = self.active_tab;
    TabBar::new("cluster-tabs")
      .child(
        Tab::new()
          .label("Nodes")
          .selected(active == ClusterTab::Nodes)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ClusterTab::Nodes;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Events")
          .selected(active == ClusterTab::Events)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ClusterTab::Events;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Namespaces")
          .selected(active == ClusterTab::Namespaces)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ClusterTab::Namespaces;
            cx.notify();
          })),
      )
  }

  fn render_nodes(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let items = state.nodes.clone();
    let load_state = state.nodes_state.clone();

    match &load_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("nodes", cx),
      LoadState::Error(e) => render_k8s_error("nodes", &e.clone(), |_ev, _w, cx| services::refresh_nodes(cx), cx),
      LoadState::Loaded => {
        if items.is_empty() {
          return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.muted_foreground).child("No nodes."));
        }
        let header = h_flex()
          .w_full()
          .min_w(px(960.))
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
              .w(px(80.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("STATUS"),
          )
          .child(
            div()
              .w(px(120.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("ROLES"),
          )
          .child(
            div()
              .w(px(100.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("VERSION"),
          )
          .child(
            div()
              .w(px(80.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("CPU"),
          )
          .child(
            div()
              .w(px(100.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("MEMORY"),
          )
          .child(
            div()
              .w(px(120.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("INTERNAL IP"),
          )
          .child(
            div()
              .w(px(80.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("OS"),
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

        let mut rows = v_flex().w_full().min_w(px(960.));
        for n in &items {
          let status_color = if n.status == "Ready" {
            colors.success
          } else {
            colors.danger
          };
          let node_name = n.name.clone();
          let unschedulable = n.unschedulable;
          let display_status = if unschedulable {
            format!("{} (cordoned)", n.status)
          } else {
            n.status.clone()
          };
          rows =
            rows.child(
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
                    .child(n.name.clone()),
                )
                .child(
                  div()
                    .w(px(80.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(status_color)
                    .child(display_status),
                )
                .child(
                  div()
                    .w(px(120.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(if n.roles.is_empty() {
                      "—".to_string()
                    } else {
                      n.roles.join(",")
                    }),
                )
                .child(
                  div()
                    .w(px(100.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(n.version.clone()),
                )
                .child(
                  div()
                    .w(px(80.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(n.cpu_allocatable.clone()),
                )
                .child(
                  div()
                    .w(px(100.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(n.mem_allocatable.clone()),
                )
                .child(
                  div()
                    .w(px(120.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(n.internal_ip.clone().unwrap_or_else(|| "—".to_string())),
                )
                .child(
                  div()
                    .w(px(80.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(format!("{}/{}", n.os, n.arch)),
                )
                .child(
                  div()
                    .w(px(60.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(n.age.clone()),
                )
                .child(
                  Button::new(SharedString::from(format!("node-menu-{node_name}")))
                    .icon(IconName::Ellipsis)
                    .ghost()
                    .xsmall()
                    .dropdown_menu({
                      let n_name = node_name.clone();
                      move |menu, _, _| {
                        let n_for_cordon = n_name.clone();
                        let n_for_drain = n_name.clone();
                        let cordon_label = if unschedulable { "Uncordon" } else { "Cordon" };
                        menu
                          .item(PopupMenuItem::new(cordon_label).on_click(move |_, _, cx| {
                            if unschedulable {
                              services::uncordon_node(n_for_cordon.clone(), cx);
                            } else {
                              services::cordon_node(n_for_cordon.clone(), cx);
                            }
                          }))
                          .separator()
                          .item(PopupMenuItem::new("Drain").icon(Icon::new(AppIcon::Trash)).on_click(
                            move |_, _, cx| {
                              services::drain_node(n_for_drain.clone(), cx);
                            },
                          ))
                      }
                    }),
                ),
            );
        }
        div().size_full().child(
          div().id("nodes-h-scroll").size_full().overflow_x_scroll().child(
            v_flex().w_full().min_w(px(960.)).size_full().child(header).child(
              div()
                .id("nodes-v-scroll")
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
  }

  fn render_events(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let items = state.events.clone();
    let load_state = state.events_state.clone();

    match &load_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("events", cx),
      LoadState::Error(e) => render_k8s_error("events", &e.clone(), |_ev, _w, cx| services::refresh_events(cx), cx),
      LoadState::Loaded => {
        if items.is_empty() {
          return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.muted_foreground).child("No events."));
        }
        let header = h_flex()
          .w_full()
          .min_w(px(920.))
          .px(px(12.))
          .py(px(6.))
          .gap(px(8.))
          .bg(colors.muted)
          .flex_shrink_0()
          .child(
            div()
              .w(px(60.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("TYPE"),
          )
          .child(
            div()
              .w(px(140.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("REASON"),
          )
          .child(
            div()
              .w(px(100.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("NAMESPACE"),
          )
          .child(
            div()
              .w(px(220.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("OBJECT"),
          )
          .child(
            div()
              .flex_1()
              .min_w(px(220.))
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("MESSAGE"),
          )
          .child(
            div()
              .w(px(40.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("#"),
          )
          .child(
            div()
              .w(px(60.))
              .flex_shrink_0()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("AGE"),
          );

        let mut rows = v_flex().w_full().min_w(px(920.));
        for e in &items {
          let type_color = if e.event_type == "Warning" {
            colors.warning
          } else {
            colors.muted_foreground
          };
          rows = rows.child(
            h_flex()
              .w_full()
              .px(px(12.))
              .py(px(6.))
              .gap(px(8.))
              .items_start()
              .border_b_1()
              .border_color(colors.border)
              .child(
                div()
                  .w(px(60.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(type_color)
                  .child(e.event_type.clone()),
              )
              .child(
                div()
                  .w(px(140.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.foreground)
                  .child(e.reason.clone()),
              )
              .child(
                div()
                  .w(px(100.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(e.namespace.clone()),
              )
              .child(
                div()
                  .w(px(220.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .child(format!("{}/{}", e.object_kind, e.object_name)),
              )
              .child(
                div()
                  .flex_1()
                  .min_w(px(220.))
                  .text_xs()
                  .text_color(colors.foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(e.message.clone()),
              )
              .child(
                div()
                  .w(px(40.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(e.count.to_string()),
              )
              .child(
                div()
                  .w(px(60.))
                  .flex_shrink_0()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(e.age.clone()),
              ),
          );
        }
        div().size_full().child(
          div().id("events-h-scroll").size_full().overflow_x_scroll().child(
            v_flex().w_full().min_w(px(920.)).size_full().child(header).child(
              div()
                .id("events-v-scroll")
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
  }

  fn render_namespaces(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);
    let namespaces = state.namespaces.clone();

    let new_input_row = self.new_namespace_input.as_ref().map(|ns_input| {
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
            .w(px(120.))
            .flex_shrink_0()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child("New namespace:"),
        )
        .child(div().flex_1().child(Input::new(ns_input).w_full()))
        .child(
          Button::new("ns-confirm")
            .label("Create")
            .primary()
            .small()
            .on_click(cx.listener(|this, _, _, cx| {
              if let Some(ref input) = this.new_namespace_input {
                let name = input.read(cx).text().to_string();
                if !name.trim().is_empty() {
                  services::create_namespace(name.trim().to_string(), cx);
                  this.new_namespace_input = None;
                  cx.notify();
                }
              }
            })),
        )
        .child(
          Button::new("ns-cancel")
            .label("Cancel")
            .ghost()
            .small()
            .on_click(cx.listener(|this, _, _, cx| {
              this.new_namespace_input = None;
              cx.notify();
            })),
        )
    });

    let toolbar = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(6.))
      .border_b_1()
      .border_color(colors.border)
      .child(
        Button::new("ns-add")
          .label("New Namespace")
          .icon(Icon::new(AppIcon::Plus))
          .ghost()
          .small()
          .on_click(cx.listener(|this, _, w, cx| {
            this.new_namespace_input = Some(cx.new(|cx| InputState::new(w, cx).placeholder("e.g. team-alpha")));
            cx.notify();
          })),
      );

    let mut list = v_flex().w_full().child(toolbar).children(new_input_row);
    list = list.child(
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
        .child(div().w(px(28.))),
    );
    for ns in &namespaces {
      let name = ns.clone();
      list = list.child(
        h_flex()
          .w_full()
          .px(px(12.))
          .py(px(8.))
          .gap(px(8.))
          .items_center()
          .border_b_1()
          .border_color(colors.border)
          .child(div().flex_1().text_sm().text_color(colors.foreground).child(ns.clone()))
          .child(
            Button::new(SharedString::from(format!("menu-{name}")))
              .icon(IconName::Ellipsis)
              .ghost()
              .xsmall()
              .dropdown_menu({
                let n = name.clone();
                move |menu, _, _| {
                  let n_for = n.clone();
                  menu.item(
                    PopupMenuItem::new("Delete")
                      .icon(Icon::new(AppIcon::Trash))
                      .on_click(move |_, _, cx| {
                        services::delete_namespace(n_for.clone(), cx);
                      }),
                  )
                }
              }),
          ),
      );
    }
    div().size_full().child(
      div()
        .id("namespaces-scroll")
        .size_full()
        .overflow_y_scrollbar()
        .child(list),
    )
  }
}

impl Render for ClusterView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;

    let toolbar = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .child(Label::new("Cluster"))
      .child(
        Button::new("refresh")
          .icon(Icon::new(AppIcon::Refresh))
          .ghost()
          .compact()
          .on_click(cx.listener(|_this, _ev, _w, cx| {
            services::refresh_nodes(cx);
            services::refresh_events(cx);
            services::refresh_namespaces(cx);
          })),
      );

    let body = match self.active_tab {
      ClusterTab::Nodes => self.render_nodes(cx),
      ClusterTab::Events => self.render_events(cx),
      ClusterTab::Namespaces => self.render_namespaces(cx),
    };

    v_flex()
      .size_full()
      .child(toolbar)
      .child(
        div()
          .w_full()
          .border_b_1()
          .border_color(colors.border)
          .child(self.render_tab_bar(cx)),
      )
      .child(div().flex_1().min_h_0().child(body))
  }
}
