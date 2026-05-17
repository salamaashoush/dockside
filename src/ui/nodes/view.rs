use std::time::Duration;

use gpui::{App, Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::theme::ActiveTheme;

use super::detail::NodeDetail;
use super::list::{NodeList, NodeListEvent};
use crate::kubernetes::NodeInfo;
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state, settings_state};

/// Nodes view: list + detail split, hosted in the Cluster view's Nodes tab.
pub struct NodesView {
  docker_state: Entity<DockerState>,
  list: Entity<NodeList>,
  detail: Entity<NodeDetail>,
}

impl NodesView {
  fn selected_node(&self, cx: &App) -> Option<NodeInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Node(ref name) = state.selection {
      state.get_node(name).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);
    let list = cx.new(|cx| NodeList::new(window, cx));
    let detail = cx.new(NodeDetail::new);

    cx.subscribe_in(
      &list,
      window,
      |this, _list, event: &NodeListEvent, _window, cx| match event {
        NodeListEvent::Selected(node) => {
          let already = matches!(
            this.docker_state.read(cx).selection,
            Selection::Node(ref n) if *n == node.name
          );
          if already {
            return;
          }
          this.detail.update(cx, |d, cx| d.set_node(node.clone(), cx));
          this.docker_state.update(cx, |s, _cx| {
            s.set_selection(Selection::Node(node.name.clone()));
          });
          cx.notify();
        }
      },
    )
    .detach();

    cx.subscribe_in(
      &docker_state,
      window,
      |this, ds, event: &StateChanged, _window, cx| match event {
        StateChanged::NodeTabRequest { name, .. } => {
          let node = ds.read(cx).get_node(name).cloned();
          if let Some(n) = node {
            this.docker_state.update(cx, |s, _cx| {
              s.set_selection(Selection::Node(n.name.clone()));
            });
            this.detail.update(cx, |d, cx| d.set_node(n, cx));
            cx.notify();
          }
        }
        StateChanged::KubeContextSwitched => {
          this.docker_state.update(cx, |s, _cx| {
            if matches!(s.selection, Selection::Node(_)) {
              s.set_selection(Selection::None);
            }
          });
          cx.notify();
        }
        _ => {}
      },
    )
    .detach();

    let refresh = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh)).await;
        let _ = cx.update(|cx| {
          services::refresh_nodes(cx);
          services::refresh_pods(cx);
          services::refresh_events(cx);
          services::refresh_node_metrics(cx);
        });
      }
    })
    .detach();

    services::refresh_nodes(cx);
    services::refresh_pods(cx);
    services::refresh_events(cx);

    Self {
      docker_state,
      list,
      detail,
    }
  }
}

impl Render for NodesView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let has_selection = self.selected_node(cx).is_some();

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        div()
          .when(has_selection, |el| {
            el.w(px(320.)).border_r_1().border_color(colors.border)
          })
          .when(!has_selection, gpui::Styled::flex_1)
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .child(self.list.clone()),
      )
      .when(has_selection, |el| {
        el.child(div().flex_1().h_full().overflow_hidden().child(self.detail.clone()))
      })
  }
}
