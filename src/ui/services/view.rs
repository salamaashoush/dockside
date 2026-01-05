use std::time::Duration;

use gpui::{App, Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::theme::ActiveTheme;

use super::detail::ServiceDetail;
use super::list::{ServiceList, ServiceListEvent};
use crate::kubernetes::ServiceInfo;
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state, settings_state};
use crate::ui::dialogs;

/// Main services view with list and detail panels
pub struct ServicesView {
  docker_state: Entity<DockerState>,
  list: Entity<ServiceList>,
  detail: Entity<ServiceDetail>,
}

impl ServicesView {
  /// Get the currently selected service from global state
  fn selected_service(&self, cx: &App) -> Option<ServiceInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Service {
      ref name,
      ref namespace,
    } = state.selection
    {
      state.get_service(name, namespace).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let list = cx.new(|cx| ServiceList::new(window, cx));
    let detail = cx.new(ServiceDetail::new);

    // Subscribe to list events
    cx.subscribe_in(
      &list,
      window,
      |this, _list, event: &ServiceListEvent, window, cx| match event {
        ServiceListEvent::Selected(service) => {
          this.detail.update(cx, |detail, cx| {
            detail.set_service(service.as_ref().clone(), cx);
          });
          // Update global selection (single source of truth)
          this.docker_state.update(cx, |state, _cx| {
            state.set_selection(Selection::Service {
              name: service.name.clone(),
              namespace: service.namespace.clone(),
            });
          });
          cx.notify();
        }
        ServiceListEvent::NewService => {
          Self::show_create_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to docker state changes
    cx.subscribe(&docker_state, |this, ds, event: &StateChanged, cx| {
      match event {
        StateChanged::ServiceTabRequest {
          service_name,
          namespace,
          tab: _,
        } => {
          // Clone the service first to avoid borrow conflict
          let svc_opt = {
            let state = ds.read(cx);
            state.get_service(service_name, namespace).cloned()
          };
          if let Some(svc) = svc_opt {
            // Update global selection
            this.docker_state.update(cx, |state, _cx| {
              state.set_selection(Selection::Service {
                name: svc.name.clone(),
                namespace: svc.namespace.clone(),
              });
            });
            cx.notify();
          }
        }
        StateChanged::ServicesUpdated => {
          // Update detail if service still exists
          let selected_key = {
            if let Selection::Service {
              ref name,
              ref namespace,
            } = this.docker_state.read(cx).selection
            {
              Some((name.clone(), namespace.clone()))
            } else {
              None
            }
          };

          if let Some((name, namespace)) = selected_key {
            // Extract the service data first to avoid borrow conflicts
            let svc_opt = ds.read(cx).get_service(&name, &namespace).cloned();
            if let Some(svc) = svc_opt {
              // Use update_service_data to preserve tab state during refresh
              this.detail.update(cx, |detail, dcx| {
                detail.update_service_data(svc, dcx);
              });
            } else {
              // Service was deleted
              this.docker_state.update(cx, |s, _| {
                s.set_selection(Selection::None);
              });
            }
          }
          cx.notify();
        }
        _ => {}
      }
    })
    .detach();

    // Start periodic refresh
    let refresh_interval = settings_state(cx).read(cx).settings.container_refresh_interval;
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh_interval)).await;
        let _ = cx.update(|cx| {
          services::refresh_machines(cx);
          services::refresh_services(cx);
        });
      }
    })
    .detach();

    // Trigger initial data load
    services::refresh_machines(cx);
    services::refresh_services(cx);

    Self {
      docker_state,
      list,
      detail,
    }
  }

  fn show_create_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    dialogs::open_create_service_dialog(window, cx);
  }
}

impl Render for ServicesView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let has_selection = self.selected_service(cx).is_some();

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Service list - fixed width when selected, full width when not
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
        el.child(
          // Right: Detail panel - only shown when selection exists
          div().flex_1().h_full().overflow_hidden().child(self.detail.clone()),
        )
      })
  }
}
