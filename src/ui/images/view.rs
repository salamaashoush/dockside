use gpui::{div, prelude::*, px, rgb, rgba, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    notification::NotificationType,
    v_flex, Sizable, WindowExt,
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
    pull_dialog: Option<Entity<PullImageDialog>>,
}

impl ImagesView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Create image list entity
        let image_list = cx.new(|cx| ImageList::new(window, cx));

        // Subscribe to image list events
        cx.subscribe(&image_list, |this, _list, event: &ImageListEvent, cx| {
            match event {
                ImageListEvent::Selected(image) => {
                    this.on_select_image(image, cx);
                }
                ImageListEvent::PullImage => {
                    this.show_pull_dialog(cx);
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
            pull_dialog: None,
        }
    }

    fn show_pull_dialog(&mut self, cx: &mut Context<Self>) {
        self.pull_dialog = Some(cx.new(|cx| PullImageDialog::new(cx)));
        cx.notify();
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

    fn render_pull_dialog(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.pull_dialog.clone().map(|dialog_entity| {
            div()
                .id("dialog-overlay")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x00000080))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id("dialog-container")
                        .on_mouse_down_out(cx.listener(|this, _ev, _window, cx| {
                            this.pull_dialog = None;
                            cx.notify();
                        }))
                        .child(
                            v_flex()
                                .w(px(500.))
                                .bg(rgb(0x24283b))
                                .rounded(px(12.))
                                .overflow_hidden()
                                .border_1()
                                .border_color(rgb(0x414868))
                                // Header
                                .child(
                                    div()
                                        .w_full()
                                        .py(px(16.))
                                        .px(px(20.))
                                        .border_b_1()
                                        .border_color(rgb(0x414868))
                                        .child(Label::new("Pull Image").text_color(rgb(0xc0caf5))),
                                )
                                // Form content
                                .child(dialog_entity.clone())
                                // Footer buttons
                                .child(
                                    h_flex()
                                        .w_full()
                                        .py(px(16.))
                                        .px(px(20.))
                                        .justify_end()
                                        .gap(px(12.))
                                        .border_t_1()
                                        .border_color(rgb(0x414868))
                                        .child(
                                            Button::new("cancel")
                                                .label("Cancel")
                                                .ghost()
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    this.pull_dialog = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child({
                                            let dialog = dialog_entity.clone();
                                            Button::new("pull")
                                                .label("Pull")
                                                .primary()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    let options = dialog.read(cx).get_options(cx);
                                                    if !options.image.is_empty() {
                                                        services::pull_image(
                                                            options.image,
                                                            options.platform.as_docker_arg().map(String::from),
                                                            cx,
                                                        );
                                                        this.pull_dialog = None;
                                                        cx.notify();
                                                    }
                                                }))
                                        }),
                                ),
                        ),
                )
        })
    }
}

impl Render for ImagesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Push any pending notifications
        for (notification_type, message) in self.pending_notifications.drain(..) {
            use gpui::SharedString;
            window.push_notification((notification_type, SharedString::from(message)), cx);
        }

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

        // Render dialog overlay if open
        let pull_dialog = self.render_pull_dialog(cx);

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
                    .border_color(rgb(0x414868))
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
            .children(pull_dialog)
    }
}
