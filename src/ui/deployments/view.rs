use std::time::Duration;

use gpui::{Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  theme::ActiveTheme,
};

use super::create_dialog::CreateDeploymentDialog;
use super::detail::DeploymentDetail;
use super::list::{DeploymentList, DeploymentListEvent};
use super::scale_dialog::ScaleDialog;
use crate::kubernetes::DeploymentInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state, settings_state};

/// Main deployments view with list and detail panels
pub struct DeploymentsView {
  _docker_state: Entity<DockerState>,
  list: Entity<DeploymentList>,
  detail: Entity<DeploymentDetail>,
  selected_deployment: Option<DeploymentInfo>,
}

impl DeploymentsView {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let list = cx.new(|cx| DeploymentList::new(window, cx));
    let detail = cx.new(DeploymentDetail::new);

    // Subscribe to list events
    cx.subscribe_in(
      &list,
      window,
      |this, _list, event: &DeploymentListEvent, window, cx| match event {
        DeploymentListEvent::Selected(deployment) => {
          this.selected_deployment = Some(deployment.clone());
          this.detail.update(cx, |detail, cx| {
            detail.set_deployment(deployment.clone(), cx);
          });
          cx.notify();
        }
        DeploymentListEvent::NewDeployment => {
          Self::show_create_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to docker state changes
    cx.subscribe_in(&docker_state, window, |this, ds, event: &StateChanged, window, cx| {
      match event {
        StateChanged::DeploymentTabRequest {
          deployment_name,
          namespace,
          tab: _,
        } => {
          let state = ds.read(cx);
          if let Some(dep) = state.get_deployment(deployment_name, namespace) {
            this.selected_deployment = Some(dep.clone());
            cx.notify();
          }
        }
        StateChanged::DeploymentsUpdated => {
          // Update selected deployment if it still exists
          if let Some(ref current) = this.selected_deployment {
            let state = ds.read(cx);
            this.selected_deployment = state.get_deployment(&current.name, &current.namespace).cloned();
            cx.notify();
          }
        }
        StateChanged::DeploymentScaleRequest {
          deployment_name,
          namespace,
          current_replicas,
        } => {
          Self::show_scale_dialog(deployment_name, namespace, *current_replicas, window, cx);
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
          services::refresh_deployments(cx);
        });
      }
    })
    .detach();

    // Trigger initial data load
    services::refresh_machines(cx);
    services::refresh_deployments(cx);

    Self {
      _docker_state: docker_state,
      list,
      detail,
      selected_deployment: None,
    }
  }

  fn show_create_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    let dialog_entity = cx.new(CreateDeploymentDialog::new);

    window.open_dialog(cx, move |dialog, _window, cx| {
      let _colors = cx.theme().colors;
      let dialog_clone = dialog_entity.clone();

      dialog
        .title("New Deployment")
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
                  if !options.name.is_empty() && !options.image.is_empty() {
                    services::create_deployment(options, cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn show_scale_dialog(
    deployment_name: &str,
    namespace: &str,
    current_replicas: i32,
    window: &mut Window,
    cx: &mut Context<'_, Self>,
  ) {
    let dialog_entity =
      cx.new(|cx| ScaleDialog::new(deployment_name.to_string(), namespace.to_string(), current_replicas, cx));

    window.open_dialog(cx, move |dialog, _window, cx| {
      let _colors = cx.theme().colors;
      let dialog_clone = dialog_entity.clone();

      dialog
        .title("Scale Deployment")
        .min_w(px(350.))
        .child(dialog_entity.clone())
        .footer(move |_dialog_state, _, _window, _cx| {
          let dialog_for_scale = dialog_clone.clone();

          vec![
            Button::new("scale")
              .label("Scale")
              .primary()
              .on_click({
                let dialog = dialog_for_scale.clone();
                move |_ev, window, cx| {
                  let replicas = dialog.read(cx).get_replicas(cx);
                  let name = dialog.read(cx).deployment_name().to_string();
                  let ns = dialog.read(cx).namespace().to_string();
                  services::scale_deployment(name, ns, replicas, cx);
                  window.close_dialog(cx);
                }
              })
              .into_any_element(),
          ]
        })
    });
  }
}

impl Render for DeploymentsView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let has_selection = self.selected_deployment.is_some();

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Deployment list - fixed width when selected, full width when not
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
