use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    notification::NotificationType,
    theme::ActiveTheme,
    WindowExt,
};

use crate::docker::ImageInfo;
use crate::services::{self, dispatcher, DispatcherEvent};
use crate::state::{docker_state, DockerState, ImageInspectData, StateChanged};

use super::detail::ImageDetail;
use super::list::{ImageList, ImageListEvent};
use super::pull_dialog::PullImageDialog;

/// Self-contained Images view - handles list, detail, and all state
pub struct ImagesView {
    docker_state: Entity<DockerState>,
    image_list: Entity<ImageList>,
    selected_image: Option<ImageInfo>,
    inspect_data: Option<ImageInspectData>,
    active_tab: usize,
    pending_notifications: Vec<(NotificationType, String)>,
}

impl ImagesView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Create image list entity
        let image_list = cx.new(|cx| ImageList::new(window, cx));

        // Subscribe to image list events
        cx.subscribe_in(&image_list, window, |this, _list, event: &ImageListEvent, window, cx| {
            match event {
                ImageListEvent::Selected(image) => {
                    this.on_select_image(image, cx);
                }
                ImageListEvent::PullImage => {
                    this.show_pull_dialog(window, cx);
                }
            }
        })
        .detach();

        // Subscribe to state changes
        cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
            match event {
                StateChanged::ImagesUpdated => {
                    // If selected image was deleted, clear selection
                    if let Some(ref selected) = this.selected_image {
                        let state = state.read(cx);
                        if !state.images.iter().any(|i| i.id == selected.id) {
                            this.selected_image = None;
                            this.inspect_data = None;
                            this.active_tab = 0;
                        } else {
                            // Update the selected image info
                            if let Some(updated) =
                                state.images.iter().find(|i| i.id == selected.id)
                            {
                                this.selected_image = Some(updated.clone());
                            }
                        }
                    }
                    cx.notify();
                }
                StateChanged::ImageInspectLoaded { image_id, data } => {
                    if let Some(ref selected) = this.selected_image {
                        if &selected.id == image_id {
                            this.inspect_data = Some(data.clone());
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
            image_list,
            selected_image: None,
            inspect_data: None,
            active_tab: 0,
            pending_notifications: Vec::new(),
        }
    }

    fn show_pull_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog_entity = cx.new(|cx| PullImageDialog::new(cx));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let dialog_clone = dialog_entity.clone();

            dialog
                .title("Pull Image")
                .min_w(px(500.))
                .child(dialog_entity.clone())
                .footer(move |_dialog_state, _, _window, _cx| {
                    let dialog_for_pull = dialog_clone.clone();

                    vec![
                        Button::new("pull")
                            .label("Pull")
                            .primary()
                            .on_click({
                                let dialog = dialog_for_pull.clone();
                                move |_ev, window, cx| {
                                    let options = dialog.read(cx).get_options(cx);
                                    if !options.image.is_empty() {
                                        services::pull_image(
                                            options.image,
                                            options.platform.as_docker_arg().map(String::from),
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

    fn on_select_image(&mut self, image: &ImageInfo, cx: &mut Context<Self>) {
        self.selected_image = Some(image.clone());
        self.inspect_data = None;
        self.active_tab = 0;

        // Load inspect data
        services::inspect_image(image.id.clone(), cx);

        cx.notify();
    }

    fn on_tab_change(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }
}

impl Render for ImagesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Push any pending notifications
        for (notification_type, message) in self.pending_notifications.drain(..) {
            use gpui::SharedString;
            window.push_notification((notification_type, SharedString::from(message)), cx);
        }

        let colors = cx.theme().colors.clone();
        let selected_image = self.selected_image.clone();
        let inspect_data = self.inspect_data.clone();
        let active_tab = self.active_tab;

        // Build detail panel
        let detail = ImageDetail::new()
            .image(selected_image)
            .inspect_data(inspect_data)
            .active_tab(active_tab)
            .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
                this.on_tab_change(*tab, cx);
            }))
            .on_delete(cx.listener(|this, id: &str, _window, cx| {
                services::delete_image(id.to_string(), cx);
                this.selected_image = None;
                this.inspect_data = None;
                this.active_tab = 0;
                cx.notify();
            }));

        div()
            .size_full()
            .flex()
            .overflow_hidden()
            .child(
                // Left: Image list - fixed width with border
                div()
                    .w(px(320.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(colors.border)
                    .child(self.image_list.clone()),
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
