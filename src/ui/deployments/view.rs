use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    theme::ActiveTheme,
    Sizable, WindowExt,
};

use super::create_dialog::CreateDeploymentDialog;
use super::detail::DeploymentDetail;
use super::list::{DeploymentList, DeploymentListEvent};
use crate::kubernetes::DeploymentInfo;
use crate::services;
use crate::state::{docker_state, DockerState, StateChanged};

/// Main deployments view with list and detail panels
pub struct DeploymentsView {
    docker_state: Entity<DockerState>,
    list: Entity<DeploymentList>,
    detail: Entity<DeploymentDetail>,
    selected_deployment: Option<DeploymentInfo>,
}

impl DeploymentsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        let list = cx.new(|cx| DeploymentList::new(window, cx));
        let detail = cx.new(|cx| DeploymentDetail::new(cx));

        // Subscribe to list events
        cx.subscribe_in(&list, window, |this, _list, event: &DeploymentListEvent, window, cx| {
            match event {
                DeploymentListEvent::Selected(deployment) => {
                    this.selected_deployment = Some(deployment.clone());
                    this.detail.update(cx, |detail, cx| {
                        detail.set_deployment(deployment.clone(), cx);
                    });
                    cx.notify();
                }
                DeploymentListEvent::NewDeployment => {
                    this.show_create_dialog(window, cx);
                }
            }
        })
        .detach();

        // Subscribe to docker state changes
        cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
            match event {
                StateChanged::DeploymentTabRequest {
                    deployment_name,
                    namespace,
                    tab: _,
                } => {
                    let state = _state.read(cx);
                    if let Some(dep) = state.get_deployment(deployment_name, namespace) {
                        this.selected_deployment = Some(dep.clone());
                        cx.notify();
                    }
                }
                StateChanged::DeploymentsUpdated => {
                    // Update selected deployment if it still exists
                    if let Some(ref current) = this.selected_deployment {
                        let state = _state.read(cx);
                        this.selected_deployment =
                            state.get_deployment(&current.name, &current.namespace).cloned();
                        cx.notify();
                    }
                }
                _ => {}
            }
        })
        .detach();

        // Trigger initial data load
        services::refresh_deployments(cx);

        Self {
            docker_state,
            list,
            detail,
            selected_deployment: None,
        }
    }

    fn show_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog_entity = cx.new(|cx| CreateDeploymentDialog::new(cx));

        window.open_dialog(cx, move |dialog, _window, cx| {
            let colors = cx.theme().colors.clone();
            let dialog_clone = dialog_entity.clone();

            dialog
                .title("New Deployment")
                .min_w(px(550.))
                .child(dialog_entity.clone())
                .footer(move |_dialog_state, _, _window, _cx| {
                    let dialog_for_create = dialog_clone.clone();

                    vec![Button::new("create")
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
                        .into_any_element()]
                })
        });
    }
}

impl Render for DeploymentsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();

        div()
            .size_full()
            .flex()
            .overflow_hidden()
            .child(
                // Left: Deployment list - fixed width with border
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
