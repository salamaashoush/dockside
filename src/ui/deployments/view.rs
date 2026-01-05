use std::time::Duration;

use gpui::{App, Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  theme::ActiveTheme,
};

use super::detail::DeploymentDetail;
use super::list::{DeploymentList, DeploymentListEvent};
use super::scale_dialog::ScaleDialog;
use crate::kubernetes::DeploymentInfo;
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state, settings_state};
use crate::ui::dialogs;

/// Main deployments view with list and detail panels
pub struct DeploymentsView {
  docker_state: Entity<DockerState>,
  list: Entity<DeploymentList>,
  detail: Entity<DeploymentDetail>,
}

impl DeploymentsView {
  /// Get the currently selected deployment from global state
  fn selected_deployment(&self, cx: &App) -> Option<DeploymentInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Deployment {
      ref name,
      ref namespace,
    } = state.selection
    {
      state.get_deployment(name, namespace).cloned()
    } else {
      None
    }
  }

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
          this.detail.update(cx, |detail, cx| {
            detail.set_deployment(deployment.clone(), cx);
          });
          // Update global selection (single source of truth)
          this.docker_state.update(cx, |state, _cx| {
            state.set_selection(Selection::Deployment {
              name: deployment.name.clone(),
              namespace: deployment.namespace.clone(),
            });
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
          // Clone the deployment first to avoid borrow conflict
          let dep_opt = {
            let state = ds.read(cx);
            state.get_deployment(deployment_name, namespace).cloned()
          };
          if let Some(dep) = dep_opt {
            // Update global selection
            this.docker_state.update(cx, |state, _cx| {
              state.set_selection(Selection::Deployment {
                name: dep.name.clone(),
                namespace: dep.namespace.clone(),
              });
            });
            cx.notify();
          }
        }
        StateChanged::DeploymentsUpdated => {
          // Update detail if deployment still exists
          let selected_key = {
            if let Selection::Deployment {
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
            // Extract the deployment data first to avoid borrow conflicts
            let dep_opt = ds.read(cx).get_deployment(&name, &namespace).cloned();
            if let Some(dep) = dep_opt {
              // Use update_deployment_data to preserve tab state during refresh
              this.detail.update(cx, |detail, dcx| {
                detail.update_deployment_data(dep, dcx);
              });
            } else {
              // Deployment was deleted
              this.docker_state.update(cx, |s, _| {
                s.set_selection(Selection::None);
              });
            }
          }
          cx.notify();
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
      docker_state,
      list,
      detail,
    }
  }

  fn show_create_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    dialogs::open_create_deployment_dialog(window, cx);
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
    let has_selection = self.selected_deployment(cx).is_some();

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
