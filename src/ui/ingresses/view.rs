use std::time::Duration;

use gpui::{App, Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::theme::ActiveTheme;

use super::detail::IngressDetail;
use super::list::{IngressList, IngressListEvent};
use crate::kubernetes::IngressInfo;
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state, settings_state};

pub struct IngressesView {
  docker_state: Entity<DockerState>,
  list: Entity<IngressList>,
  detail: Entity<IngressDetail>,
}

impl IngressesView {
  fn selected(&self, cx: &App) -> Option<IngressInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Ingress {
      ref name,
      ref namespace,
    } = state.selection
    {
      state.get_ingress(name, namespace).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let list = cx.new(|cx| IngressList::new(window, cx));
    let detail = cx.new(IngressDetail::new);

    cx.subscribe_in(
      &list,
      window,
      |this, _list, event: &IngressListEvent, _window, cx| match event {
        IngressListEvent::Selected(item) => {
          let already_selected = matches!(
            this.docker_state.read(cx).selection,
            Selection::Ingress { ref name, ref namespace } if *name == item.name && *namespace == item.namespace
          );
          if already_selected {
            return;
          }
          this.detail.update(cx, |detail, cx| {
            detail.set_item(item.clone(), cx);
          });
          this.docker_state.update(cx, |state, _cx| {
            state.set_selection(Selection::Ingress {
              name: item.name.clone(),
              namespace: item.namespace.clone(),
            });
          });
          cx.notify();
        }
      },
    )
    .detach();

    cx.subscribe_in(
      &docker_state,
      window,
      |this, ds, event: &StateChanged, _w, cx| match event {
        StateChanged::IngressTabRequest {
          name,
          namespace,
          tab: _,
        } => {
          let opt = ds.read(cx).get_ingress(name, namespace).cloned();
          if let Some(item) = opt {
            this.docker_state.update(cx, |state, _| {
              state.set_selection(Selection::Ingress {
                name: item.name.clone(),
                namespace: item.namespace.clone(),
              });
            });
            cx.notify();
          }
        }
        StateChanged::IngressesUpdated => {
          let key = if let Selection::Ingress {
            ref name,
            ref namespace,
          } = this.docker_state.read(cx).selection
          {
            Some((name.clone(), namespace.clone()))
          } else {
            None
          };
          if let Some((name, namespace)) = key {
            let opt = ds.read(cx).get_ingress(&name, &namespace).cloned();
            if let Some(item) = opt {
              this.detail.update(cx, |detail, dcx| {
                detail.update_item(item, dcx);
              });
            } else {
              this.docker_state.update(cx, |s, _| s.set_selection(Selection::None));
            }
          }
          cx.notify();
        }
        _ => {}
      },
    )
    .detach();

    let refresh_interval = settings_state(cx).read(cx).settings.container_refresh_interval.max(5);
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh_interval)).await;
        let _ = cx.update(services::refresh_ingresses);
      }
    })
    .detach();

    services::refresh_ingresses(cx);
    services::refresh_namespaces(cx);

    Self {
      docker_state,
      list,
      detail,
    }
  }
}

impl Render for IngressesView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let has_selection = self.selected(cx).is_some();

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
