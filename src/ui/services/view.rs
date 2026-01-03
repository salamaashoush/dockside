use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  theme::ActiveTheme,
};

use super::create_dialog::CreateServiceDialog;
use super::detail::ServiceDetail;
use super::list::{ServiceList, ServiceListEvent};
use crate::kubernetes::ServiceInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};

/// Main services view with list and detail panels
pub struct ServicesView {
  _docker_state: Entity<DockerState>,
  list: Entity<ServiceList>,
  detail: Entity<ServiceDetail>,
  selected_service: Option<ServiceInfo>,
}

impl ServicesView {
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
          this.selected_service = Some(service.as_ref().clone());
          this.detail.update(cx, |detail, cx| {
            detail.set_service(service.as_ref().clone(), cx);
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
          let state = ds.read(cx);
          if let Some(svc) = state.get_service(service_name, namespace) {
            this.selected_service = Some(svc.clone());
            cx.notify();
          }
        }
        StateChanged::ServicesUpdated => {
          // Update selected service if it still exists
          if let Some(ref current) = this.selected_service {
            let state = ds.read(cx);
            this.selected_service = state.get_service(&current.name, &current.namespace).cloned();
            cx.notify();
          }
        }
        _ => {}
      }
    })
    .detach();

    // Trigger initial data load
    services::refresh_services(cx);

    Self {
      _docker_state: docker_state,
      list,
      detail,
      selected_service: None,
    }
  }

  fn show_create_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    let dialog_entity = cx.new(CreateServiceDialog::new);

    window.open_dialog(cx, move |dialog, _window, cx| {
      let _colors = cx.theme().colors;
      let dialog_clone = dialog_entity.clone();

      dialog
        .title("New Service")
        .min_w(px(550.))
        .child(dialog_entity.clone())
        .footer(move |_dialog_state, _, _window, _cx| {
          let dialog_for_create = dialog_clone.clone();

          vec![
            Button::new("create")
              .label("Create")
              .primary()
              .on_click({
                let dialog = dialog_for_create.clone();
                move |_ev, window, cx| {
                  let options = dialog.read(cx).get_options(cx);
                  if !options.name.is_empty() && !options.ports.is_empty() {
                    services::create_service(options, cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        })
    });
  }
}

impl Render for ServicesView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Service list - fixed width with border
        div()
          .w(px(320.))
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .border_r_1()
          .border_color(colors.border)
          .child(self.list.clone()),
      )
      .child(
        // Right: Detail panel - flexible width
        div().flex_1().h_full().overflow_hidden().child(self.detail.clone()),
      )
  }
}
