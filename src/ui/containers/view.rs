use gpui::{div, prelude::*, px, rgb, rgba, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    notification::NotificationType,
    v_flex, Sizable, WindowExt,
};

use crate::docker::ContainerInfo;
use crate::services::{self, dispatcher, DispatcherEvent};
use crate::state::{docker_state, DockerState, StateChanged};
use crate::terminal::{TerminalSessionType, TerminalView};

use super::create_dialog::CreateContainerDialog;
use super::detail::{ContainerDetail, ContainerTabState};
use super::list::{ContainerList, ContainerListEvent};

/// Self-contained Containers view - handles list, detail, and all state
pub struct ContainersView {
    docker_state: Entity<DockerState>,
    container_list: Entity<ContainerList>,
    selected_container: Option<ContainerInfo>,
    active_tab: usize,
    terminal_view: Option<Entity<TerminalView>>,
    container_tab_state: ContainerTabState,
    pending_notifications: Vec<(NotificationType, String)>,
    create_dialog: Option<Entity<CreateContainerDialog>>,
}

impl ContainersView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Create container list entity
        let container_list = cx.new(|cx| ContainerList::new(window, cx));

        // Subscribe to container list events
        cx.subscribe(&container_list, |this, _list, event: &ContainerListEvent, cx| {
            match event {
                ContainerListEvent::Selected(container) => {
                    this.on_select_container(container, cx);
                }
                ContainerListEvent::NewContainer => {
                    this.show_create_dialog(cx);
                }
            }
        })
        .detach();

        // Subscribe to state changes
        cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
            match event {
                StateChanged::ContainersUpdated => {
                    // If selected container was deleted, clear selection
                    if let Some(ref selected) = this.selected_container {
                        let state = state.read(cx);
                        if !state.containers.iter().any(|c| c.id == selected.id) {
                            this.selected_container = None;
                            this.active_tab = 0;
                            this.terminal_view = None;
                        } else {
                            // Update the selected container info
                            if let Some(updated) = state.containers.iter().find(|c| c.id == selected.id) {
                                this.selected_container = Some(updated.clone());
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
            container_list,
            selected_container: None,
            active_tab: 0,
            terminal_view: None,
            container_tab_state: ContainerTabState::new(),
            pending_notifications: Vec::new(),
            create_dialog: None,
        }
    }

    fn show_create_dialog(&mut self, cx: &mut Context<Self>) {
        self.create_dialog = Some(cx.new(|cx| CreateContainerDialog::new(cx)));
        cx.notify();
    }

    fn on_select_container(&mut self, container: &ContainerInfo, cx: &mut Context<Self>) {
        self.selected_container = Some(container.clone());
        self.active_tab = 0;
        self.terminal_view = None;

        // Load logs for the selected container
        self.load_container_data(&container.id, cx);

        cx.notify();
    }

    fn on_tab_change(&mut self, tab: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = tab;

        // If switching to terminal tab, create terminal view
        if tab == 2 {
            if self.terminal_view.is_none() {
                if let Some(ref container) = self.selected_container {
                    let container_id = container.id.clone();
                    self.terminal_view = Some(cx.new(|cx| {
                        TerminalView::new(
                            TerminalSessionType::docker_exec(container_id, None),
                            window,
                            cx,
                        )
                    }));
                }
            }
        }

        // If switching to files tab, load files
        if tab == 3 {
            if let Some(ref container) = self.selected_container {
                if container.state.is_running() {
                    let container_id = container.id.clone();
                    let path = self.container_tab_state.current_path.clone();
                    self.load_container_files(&container_id, &path, cx);
                }
            }
        }

        cx.notify();
    }

    fn on_navigate_path(&mut self, path: &str, cx: &mut Context<Self>) {
        self.container_tab_state.current_path = path.to_string();
        if let Some(ref container) = self.selected_container {
            if container.state.is_running() {
                let container_id = container.id.clone();
                self.load_container_files(&container_id, path, cx);
            }
        }
    }

    fn load_container_files(&mut self, container_id: &str, path: &str, cx: &mut Context<Self>) {
        self.container_tab_state.files_loading = true;
        self.container_tab_state.files.clear();
        cx.notify();

        let id = container_id.to_string();
        let path = path.to_string();
        let tokio_handle = services::Tokio::runtime_handle();
        let client = services::docker_client();

        cx.spawn(async move |this, cx| {
            let files = cx
                .background_executor()
                .spawn(async move {
                    tokio_handle.block_on(async {
                        let guard = client.read().await;
                        match guard.as_ref() {
                            Some(c) => c.list_container_files(&id, &path).await.ok(),
                            None => None,
                        }
                    })
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.container_tab_state.files = files.unwrap_or_default();
                this.container_tab_state.files_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn load_container_data(&mut self, container_id: &str, cx: &mut Context<Self>) {
        self.container_tab_state.logs_loading = true;
        self.container_tab_state.inspect_loading = true;

        let id = container_id.to_string();
        let id_for_inspect = id.clone();

        // Get tokio handle and docker client before spawning
        let tokio_handle = services::Tokio::runtime_handle();
        let client = services::docker_client();
        let client_for_inspect = client.clone();
        let tokio_handle_for_inspect = tokio_handle.clone();

        // Load logs in background
        cx.spawn(async move |this, cx| {
            let logs = cx
                .background_executor()
                .spawn(async move {
                    tokio_handle.block_on(async {
                        let guard = client.read().await;
                        match guard.as_ref() {
                            Some(c) => c
                                .container_logs(&id, Some(100))
                                .await
                                .unwrap_or_else(|e| format!("Failed to get logs: {}", e)),
                            None => "Docker client not connected".to_string(),
                        }
                    })
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.container_tab_state.logs = logs;
                this.container_tab_state.logs_loading = false;
                cx.notify();
            });
        })
        .detach();

        // Load inspect data in background
        cx.spawn(async move |this, cx| {
            let inspect = cx
                .background_executor()
                .spawn(async move {
                    tokio_handle_for_inspect.block_on(async {
                        let guard = client_for_inspect.read().await;
                        match guard.as_ref() {
                            Some(c) => c
                                .inspect_container(&id_for_inspect)
                                .await
                                .unwrap_or_else(|e| format!("Failed to inspect: {}", e)),
                            None => "Docker client not connected".to_string(),
                        }
                    })
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.container_tab_state.inspect = inspect;
                this.container_tab_state.inspect_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn on_refresh_logs(&mut self, cx: &mut Context<Self>) {
        if let Some(ref container) = self.selected_container.clone() {
            self.load_container_data(&container.id, cx);
        }
    }

    fn render_create_dialog(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.create_dialog.clone().map(|dialog_entity| {
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
                            this.create_dialog = None;
                            cx.notify();
                        }))
                        .child(
                            v_flex()
                                .w(px(550.))
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
                                        .child(Label::new("New Container").text_color(rgb(0xc0caf5))),
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
                                                    this.create_dialog = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child({
                                            let dialog = dialog_entity.clone();
                                            Button::new("create")
                                                .label("Create")
                                                .ghost()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    let options = dialog.read(cx).get_options(cx, false);
                                                    if !options.image.is_empty() {
                                                        services::create_container(options, cx);
                                                        this.create_dialog = None;
                                                        cx.notify();
                                                    }
                                                }))
                                        })
                                        .child({
                                            let dialog = dialog_entity.clone();
                                            Button::new("create-start")
                                                .label("Create & Start")
                                                .primary()
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    let options = dialog.read(cx).get_options(cx, true);
                                                    if !options.image.is_empty() {
                                                        services::create_container(options, cx);
                                                        this.create_dialog = None;
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

impl Render for ContainersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Push any pending notifications
        for (notification_type, message) in self.pending_notifications.drain(..) {
            use gpui::SharedString;
            window.push_notification((notification_type, SharedString::from(message)), cx);
        }

        let selected_container = self.selected_container.clone();
        let active_tab = self.active_tab;
        let container_tab_state = self.container_tab_state.clone();
        let terminal_view = self.terminal_view.clone();

        // Build detail panel
        let detail = ContainerDetail::new()
            .container(selected_container)
            .active_tab(active_tab)
            .container_state(container_tab_state)
            .terminal_view(terminal_view)
            .on_tab_change(cx.listener(|this, tab: &usize, window, cx| {
                this.on_tab_change(*tab, window, cx);
            }))
            .on_refresh_logs(cx.listener(|this, _: &(), _window, cx| {
                this.on_refresh_logs(cx);
            }))
            .on_navigate_path(cx.listener(|this, path: &str, _window, cx| {
                this.on_navigate_path(path, cx);
            }))
            .on_start(cx.listener(|_this, id: &str, _window, cx| {
                crate::services::start_container(id.to_string(), cx);
            }))
            .on_stop(cx.listener(|_this, id: &str, _window, cx| {
                crate::services::stop_container(id.to_string(), cx);
            }))
            .on_restart(cx.listener(|_this, id: &str, _window, cx| {
                crate::services::restart_container(id.to_string(), cx);
            }))
            .on_delete(cx.listener(|this, id: &str, _window, cx| {
                crate::services::delete_container(id.to_string(), cx);
                this.selected_container = None;
                this.active_tab = 0;
                this.terminal_view = None;
                cx.notify();
            }));

        // Render dialog overlay if open
        let create_dialog = self.render_create_dialog(cx);

        div()
            .size_full()
            .flex()
            .overflow_hidden()
            .child(
                // Left: Container list - fixed width with border
                div()
                    .w(px(320.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(rgb(0x414868))
                    .child(self.container_list.clone()),
            )
            .child(
                // Right: Detail panel - flexible width
                div().flex_1().h_full().overflow_hidden().child(detail.render(window, cx)),
            )
            .children(create_dialog)
    }
}
