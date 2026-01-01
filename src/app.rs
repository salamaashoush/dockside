use gpui::{div, prelude::*, px, App, Context, Entity, Render, Styled, Window};
use gpui_component::{
    h_flex,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    theme::{ActiveTheme, Theme, ThemeMode},
    v_flex, IconName, Root,
};

use crate::services::task_manager;
use crate::state::{docker_state, CurrentView, DockerState, StateChanged};
use crate::ui::containers::ContainersView;
use crate::ui::images::ImagesView;
use crate::ui::machines::MachinesView;
use crate::ui::networks::NetworksView;
use crate::ui::volumes::VolumesView;

/// Main application - only handles layout and view switching
pub struct DockerApp {
    docker_state: Entity<DockerState>,
    machines_view: Entity<MachinesView>,
    containers_view: Entity<ContainersView>,
    volumes_view: Entity<VolumesView>,
    images_view: Entity<ImagesView>,
    networks_view: Entity<NetworksView>,
}

impl DockerApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Theme::change(ThemeMode::Dark, Some(window), cx);

        let docker_state = docker_state(cx);

        // Subscribe to state changes for re-rendering on view changes
        cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
            if matches!(event, StateChanged::ViewChanged | StateChanged::Loading(_)) {
                cx.notify();
            }
        })
        .detach();

        // Create self-contained views
        let machines_view = cx.new(|cx| MachinesView::new(window, cx));
        let containers_view = cx.new(|cx| ContainersView::new(window, cx));
        let volumes_view = cx.new(|cx| VolumesView::new(window, cx));
        let images_view = cx.new(|cx| ImagesView::new(window, cx));
        let networks_view = cx.new(|cx| NetworksView::new(window, cx));

        Self {
            docker_state,
            machines_view,
            containers_view,
            volumes_view,
            images_view,
            networks_view,
        }
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.docker_state.read(cx);
        let current_view = state.current_view;

        Sidebar::left()
            .collapsible(false)
            .pt(px(52.)) // Space for traffic lights
            .child(
                SidebarGroup::new("Docker").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Containers")
                                .icon(IconName::SquareTerminal)
                                .active(current_view == CurrentView::Containers)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Containers, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Volumes")
                                .icon(IconName::Folder)
                                .active(current_view == CurrentView::Volumes)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Volumes, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Images")
                                .icon(IconName::GalleryVerticalEnd)
                                .active(current_view == CurrentView::Images)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Images, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Networks")
                                .icon(IconName::Globe)
                                .active(current_view == CurrentView::Networks)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Networks, cx);
                                })),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Kubernetes").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Pods")
                                .icon(IconName::LayoutDashboard)
                                .active(current_view == CurrentView::Pods)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Pods, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Services")
                                .icon(IconName::Settings)
                                .active(current_view == CurrentView::Services)
                                .on_click(cx.listener(|_this, _ev, _window, cx| {
                                    crate::services::set_view(CurrentView::Services, cx);
                                })),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Linux").child(
                    SidebarMenu::new().child(
                        SidebarMenuItem::new("Machines")
                            .icon(IconName::Frame)
                            .active(current_view == CurrentView::Machines)
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                crate::services::set_view(CurrentView::Machines, cx);
                            })),
                    ),
                ),
            )
    }

    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.docker_state.read(cx);

        match state.current_view {
            CurrentView::Machines => div().size_full().child(self.machines_view.clone()),
            CurrentView::Containers => div().size_full().child(self.containers_view.clone()),
            CurrentView::Volumes => div().size_full().child(self.volumes_view.clone()),
            CurrentView::Images => div().size_full().child(self.images_view.clone()),
            CurrentView::Networks => div().size_full().child(self.networks_view.clone()),
            _ => {
                // Placeholder for views not yet implemented
                let colors = &cx.theme().colors;
                div()
                    .size_full()
                    .bg(colors.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(colors.muted_foreground)
                            .child(format!("{:?} view coming soon", state.current_view)),
                    )
            }
        }
    }

    fn render_task_bar(&self, cx: &App) -> Option<impl IntoElement> {
        let tasks = task_manager(cx);
        let running_tasks: Vec<_> = tasks
            .read(cx)
            .running_tasks()
            .into_iter()
            .cloned()
            .collect();

        if running_tasks.is_empty() {
            return None;
        }

        let colors = &cx.theme().colors;

        Some(
            h_flex()
                .w_full()
                .py(px(6.))
                .px(px(16.))
                .bg(colors.background)
                .border_b_1()
                .border_color(colors.border)
                .gap(px(16.))
                .items_center()
                .children(running_tasks.iter().map(|task| {
                    h_flex().gap(px(8.)).items_center().child(
                        div()
                            .text_sm()
                            .text_color(colors.link)
                            .child(task.description.clone()),
                    )
                })),
        )
    }
}

impl Render for DockerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Required layers for gpui-component
        let notification_layer = Root::render_notification_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);

        let sidebar = self.render_sidebar(cx);
        let content = self.render_content(cx);
        let task_bar = self.render_task_bar(cx);

        // Get colors after mutable borrows are done
        let background = cx.theme().colors.background;

        v_flex()
            .size_full()
            .bg(background)
            .overflow_hidden()
            // Main layout: sidebar + content
            .child(
                h_flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(div().w(px(220.)).h_full().flex_shrink_0().overflow_hidden().child(sidebar))
                    .child(div().flex_1().h_full().overflow_hidden().child(content)),
            )
            // Task bar at bottom (if any running tasks)
            .children(task_bar)
            // gpui-component layers
            .children(notification_layer)
            .children(dialog_layer)
    }
}
