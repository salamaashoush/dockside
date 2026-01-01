use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    notification::NotificationType,
    theme::ActiveTheme,
    WindowExt,
};

use crate::docker::VolumeInfo;
use crate::services::{self, dispatcher, DispatcherEvent};
use crate::state::{docker_state, DockerState, StateChanged};

use super::create_dialog::CreateVolumeDialog;
use super::detail::{VolumeDetail, VolumeTabState};
use super::list::{VolumeList, VolumeListEvent};

/// Self-contained Volumes view - handles list, detail, and all state
pub struct VolumesView {
    docker_state: Entity<DockerState>,
    volume_list: Entity<VolumeList>,
    selected_volume: Option<VolumeInfo>,
    active_tab: usize,
    volume_tab_state: VolumeTabState,
    pending_notifications: Vec<(NotificationType, String)>,
}

impl VolumesView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Create volume list entity
        let volume_list = cx.new(|cx| VolumeList::new(window, cx));

        // Subscribe to volume list events
        cx.subscribe_in(&volume_list, window, |this, _list, event: &VolumeListEvent, window, cx| {
            match event {
                VolumeListEvent::Selected(volume) => {
                    this.on_select_volume(volume, cx);
                }
                VolumeListEvent::NewVolume => {
                    this.show_create_dialog(window, cx);
                }
            }
        })
        .detach();

        // Subscribe to state changes
        cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
            match event {
                StateChanged::VolumesUpdated => {
                    // If selected volume was deleted, clear selection
                    if let Some(ref selected) = this.selected_volume {
                        let state = state.read(cx);
                        if !state.volumes.iter().any(|v| v.name == selected.name) {
                            this.selected_volume = None;
                            this.active_tab = 0;
                            this.volume_tab_state = VolumeTabState::new();
                        } else {
                            // Update the selected volume info
                            if let Some(updated) =
                                state.volumes.iter().find(|v| v.name == selected.name)
                            {
                                this.selected_volume = Some(updated.clone());
                            }
                        }
                    }
                    cx.notify();
                }
                StateChanged::VolumeFilesLoaded { volume_name, path, files } => {
                    // Update file state if this is for the currently selected volume
                    if let Some(ref selected) = this.selected_volume {
                        if &selected.name == volume_name {
                            this.volume_tab_state.files = files.clone();
                            this.volume_tab_state.current_path = path.clone();
                            this.volume_tab_state.files_loading = false;
                            cx.notify();
                        }
                    }
                }
                StateChanged::VolumeFilesError { volume_name, error: _ } => {
                    // Handle error for the currently selected volume
                    if let Some(ref selected) = this.selected_volume {
                        if &selected.name == volume_name {
                            this.volume_tab_state.files_loading = false;
                            this.volume_tab_state.files = vec![];
                            cx.notify();
                        }
                    }
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
            volume_list,
            selected_volume: None,
            active_tab: 0,
            volume_tab_state: VolumeTabState::new(),
            pending_notifications: Vec::new(),
        }
    }

    fn show_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog_entity = cx.new(|cx| CreateVolumeDialog::new(cx));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let dialog_clone = dialog_entity.clone();

            dialog
                .title("New Volume")
                .min_w(px(500.))
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
                                    if !options.name.is_empty() {
                                        services::create_volume(
                                            options.name,
                                            options.driver.as_docker_arg().to_string(),
                                            options.labels,
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

    fn on_select_volume(&mut self, volume: &VolumeInfo, cx: &mut Context<Self>) {
        self.selected_volume = Some(volume.clone());
        self.active_tab = 0;
        // Reset file state when selecting new volume
        self.volume_tab_state = VolumeTabState::new();
        cx.notify();
    }

    fn on_tab_change(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.active_tab = tab;
        // Load files when switching to Files tab
        if tab == 1 {
            self.load_volume_files("/", cx);
        }
        cx.notify();
    }

    fn load_volume_files(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(ref volume) = self.selected_volume {
            self.volume_tab_state.files_loading = true;
            self.volume_tab_state.current_path = path.to_string();
            cx.notify();

            let volume_name = volume.name.clone();
            let path = path.to_string();
            services::list_volume_files(volume_name, path, cx);
        }
    }

    fn on_navigate_path(&mut self, path: &str, cx: &mut Context<Self>) {
        self.load_volume_files(path, cx);
    }
}

impl Render for VolumesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Push any pending notifications
        for (notification_type, message) in self.pending_notifications.drain(..) {
            use gpui::SharedString;
            window.push_notification((notification_type, SharedString::from(message)), cx);
        }

        let colors = cx.theme().colors.clone();
        let selected_volume = self.selected_volume.clone();
        let active_tab = self.active_tab;

        // Build detail panel
        let detail = VolumeDetail::new()
            .volume(selected_volume)
            .active_tab(active_tab)
            .volume_state(self.volume_tab_state.clone())
            .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
                this.on_tab_change(*tab, cx);
            }))
            .on_navigate_path(cx.listener(|this, path: &str, _window, cx| {
                this.on_navigate_path(path, cx);
            }))
            .on_delete(cx.listener(|this, name: &str, _window, cx| {
                services::delete_volume(name.to_string(), cx);
                this.selected_volume = None;
                this.active_tab = 0;
                cx.notify();
            }));

        div()
            .size_full()
            .flex()
            .overflow_hidden()
            .child(
                // Left: Volume list - fixed width with border
                div()
                    .w(px(320.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(colors.border)
                    .child(self.volume_list.clone()),
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
