use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    notification::NotificationType,
    theme::ActiveTheme,
    v_flex, WindowExt,
};

use crate::docker::NetworkInfo;
use crate::services::{self, dispatcher, DispatcherEvent};
use crate::state::{docker_state, DockerState, StateChanged};

use super::create_dialog::CreateNetworkDialog;
use super::detail::NetworkDetail;
use super::list::{NetworkList, NetworkListEvent};

/// Self-contained Networks view - handles list, detail, and all state
pub struct NetworksView {
    docker_state: Entity<DockerState>,
    network_list: Entity<NetworkList>,
    selected_network: Option<NetworkInfo>,
    active_tab: usize,
    pending_notifications: Vec<(NotificationType, String)>,
}

impl NetworksView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Create network list entity
        let network_list = cx.new(|cx| NetworkList::new(window, cx));

        // Subscribe to network list events
        cx.subscribe_in(&network_list, window, |this, _list, event: &NetworkListEvent, window, cx| {
            match event {
                NetworkListEvent::Selected(network) => {
                    this.on_select_network(network, cx);
                }
                NetworkListEvent::CreateNetwork => {
                    this.show_create_dialog(window, cx);
                }
            }
        })
        .detach();

        // Subscribe to state changes
        cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
            match event {
                StateChanged::NetworksUpdated => {
                    // If selected network was deleted, clear selection
                    if let Some(ref selected) = this.selected_network {
                        let state = state.read(cx);
                        if !state.networks.iter().any(|n| n.id == selected.id) {
                            this.selected_network = None;
                            this.active_tab = 0;
                        } else {
                            // Update the selected network info
                            if let Some(updated) =
                                state.networks.iter().find(|n| n.id == selected.id)
                            {
                                this.selected_network = Some(updated.clone());
                            }
                        }
                    }
                    cx.notify();
                }
                _ => {}
            }
        })
        .detach();

        // Subscribe to dispatcher events for notifications
        let disp = dispatcher(cx);
        cx.subscribe(&disp, |this, _disp, event: &DispatcherEvent, cx| {
            match event {
                DispatcherEvent::TaskCompleted { name: _, message } => {
                    this.pending_notifications
                        .push((NotificationType::Success, message.clone()));
                }
                DispatcherEvent::TaskFailed { name: _, error } => {
                    this.pending_notifications
                        .push((NotificationType::Error, error.clone()));
                }
            }
            cx.notify();
        })
        .detach();

        Self {
            docker_state,
            network_list,
            selected_network: None,
            active_tab: 0,
            pending_notifications: Vec::new(),
        }
    }

    fn show_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog_entity = cx.new(|cx| CreateNetworkDialog::new(cx));
        let colors = cx.theme().colors.clone();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let dialog_clone = dialog_entity.clone();

            dialog
                .title("New Network")
                .min_w(px(500.))
                .child(
                    v_flex()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("Networks are groups of containers in the same subnet (IP range) that can communicate with each other."),
                        )
                        .child(dialog_entity.clone()),
                )
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
                                    if !options.name.is_empty() {
                                        services::create_network(
                                            options.name,
                                            options.enable_ipv6,
                                            options.subnet,
                                            cx,
                                        );
                                        window.close_dialog(cx);
                                    }
                                }
                            })
                            .into_any_element(),
                    ]
                })
        });
    }

    fn on_select_network(&mut self, network: &NetworkInfo, cx: &mut Context<Self>) {
        self.selected_network = Some(network.clone());
        self.active_tab = 0;
        cx.notify();
    }

    fn on_tab_change(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }
}

impl Render for NetworksView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Push any pending notifications
        for (notification_type, message) in self.pending_notifications.drain(..) {
            use gpui::SharedString;
            window.push_notification((notification_type, SharedString::from(message)), cx);
        }

        let colors = cx.theme().colors.clone();
        let selected_network = self.selected_network.clone();
        let active_tab = self.active_tab;

        // Build detail panel
        let detail = NetworkDetail::new()
            .network(selected_network)
            .active_tab(active_tab)
            .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
                this.on_tab_change(*tab, cx);
            }))
            .on_delete(cx.listener(|this, id: &str, _window, cx| {
                services::delete_network(id.to_string(), cx);
                this.selected_network = None;
                this.active_tab = 0;
                cx.notify();
            }));

        div()
            .size_full()
            .flex()
            .overflow_hidden()
            .child(
                // Left: Network list - fixed width with border
                div()
                    .w(px(320.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(colors.border)
                    .child(self.network_list.clone()),
            )
            .child(
                // Right: Detail panel - flexible width
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .child(detail.render(window, cx)),
            )
    }
}
