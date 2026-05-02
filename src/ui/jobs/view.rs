use std::time::Duration;

use gpui::{App, Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::theme::ActiveTheme;

use super::detail::JobDetail;
use super::list::{JobList, JobListEvent};
use crate::kubernetes::JobInfo;
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state, settings_state};

pub struct JobsView {
  docker_state: Entity<DockerState>,
  list: Entity<JobList>,
  detail: Entity<JobDetail>,
}

impl JobsView {
  fn selected(&self, cx: &App) -> Option<JobInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Job {
      ref name,
      ref namespace,
    } = state.selection
    {
      state.get_job(name, namespace).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let list = cx.new(|cx| JobList::new(window, cx));
    let detail = cx.new(JobDetail::new);

    cx.subscribe_in(
      &list,
      window,
      |this, _list, event: &JobListEvent, _window, cx| match event {
        JobListEvent::Selected(item) => {
          let already_selected = matches!(
            this.docker_state.read(cx).selection,
            Selection::Job { ref name, ref namespace } if *name == item.name && *namespace == item.namespace
          );
          if already_selected {
            return;
          }
          this.detail.update(cx, |detail, cx| {
            detail.set_item(item.clone(), cx);
          });
          this.docker_state.update(cx, |state, _cx| {
            state.set_selection(Selection::Job {
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
        StateChanged::JobTabRequest {
          name,
          namespace,
          tab: _,
        } => {
          let opt = ds.read(cx).get_job(name, namespace).cloned();
          if let Some(item) = opt {
            this.docker_state.update(cx, |state, _| {
              state.set_selection(Selection::Job {
                name: item.name.clone(),
                namespace: item.namespace.clone(),
              });
            });
            cx.notify();
          }
        }
        StateChanged::JobsUpdated => {
          let key = if let Selection::Job {
            ref name,
            ref namespace,
          } = this.docker_state.read(cx).selection
          {
            Some((name.clone(), namespace.clone()))
          } else {
            None
          };
          if let Some((name, namespace)) = key {
            let opt = ds.read(cx).get_job(&name, &namespace).cloned();
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
        let _ = cx.update(services::refresh_jobs);
      }
    })
    .detach();

    services::refresh_jobs(cx);
    services::refresh_namespaces(cx);

    Self {
      docker_state,
      list,
      detail,
    }
  }
}

impl Render for JobsView {
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
