use gpui::{div, prelude::*, px, App, Entity, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollableElement,
    tab::{Tab, TabBar},
    theme::ActiveTheme,
    v_flex, Icon, IconName, Selectable, Sizable,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::docker::{ContainerFileEntry, ContainerInfo};
use crate::terminal::TerminalView;

type ContainerActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;
type RefreshCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type FileNavigateCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// State for container detail tabs
#[derive(Debug, Clone, Default)]
pub struct ContainerTabState {
    pub logs: String,
    pub logs_loading: bool,
    pub inspect: String,
    pub inspect_loading: bool,
    pub current_path: String,
    pub files: Vec<ContainerFileEntry>,
    pub files_loading: bool,
}

impl ContainerTabState {
    pub fn new() -> Self {
        Self {
            current_path: "/".to_string(),
            ..Default::default()
        }
    }
}

pub struct ContainerDetail {
    container: Option<ContainerInfo>,
    active_tab: usize,
    container_state: Option<ContainerTabState>,
    terminal_view: Option<Entity<TerminalView>>,
    on_start: Option<ContainerActionCallback>,
    on_stop: Option<ContainerActionCallback>,
    on_restart: Option<ContainerActionCallback>,
    on_delete: Option<ContainerActionCallback>,
    on_tab_change: Option<TabChangeCallback>,
    on_refresh_logs: Option<RefreshCallback>,
    on_navigate_path: Option<FileNavigateCallback>,
}

impl ContainerDetail {
    pub fn new() -> Self {
        Self {
            container: None,
            active_tab: 0,
            container_state: None,
            terminal_view: None,
            on_start: None,
            on_stop: None,
            on_restart: None,
            on_delete: None,
            on_tab_change: None,
            on_refresh_logs: None,
            on_navigate_path: None,
        }
    }

    pub fn container(mut self, container: Option<ContainerInfo>) -> Self {
        self.container = container;
        self
    }

    pub fn active_tab(mut self, tab: usize) -> Self {
        self.active_tab = tab;
        self
    }

    pub fn container_state(mut self, state: ContainerTabState) -> Self {
        self.container_state = Some(state);
        self
    }

    pub fn terminal_view(mut self, view: Option<Entity<TerminalView>>) -> Self {
        self.terminal_view = view;
        self
    }

    pub fn on_start<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_start = Some(Rc::new(callback));
        self
    }

    pub fn on_stop<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_stop = Some(Rc::new(callback));
        self
    }

    pub fn on_restart<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_restart = Some(Rc::new(callback));
        self
    }

    pub fn on_delete<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_delete = Some(Rc::new(callback));
        self
    }

    pub fn on_tab_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_tab_change = Some(Rc::new(callback));
        self
    }

    pub fn on_refresh_logs<F>(mut self, callback: F) -> Self
    where
        F: Fn(&(), &mut Window, &mut App) + 'static,
    {
        self.on_refresh_logs = Some(Rc::new(callback));
        self
    }

    pub fn on_navigate_path<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut App) + 'static,
    {
        self.on_navigate_path = Some(Rc::new(callback));
        self
    }

    fn render_empty(&self, cx: &App) -> gpui::Div {
        let colors = &cx.theme().colors;

        div()
            .size_full()
            .bg(colors.sidebar)
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        Icon::new(AppIcon::Container)
                            .size(px(48.))
                            .text_color(colors.muted_foreground),
                    )
                    .child(
                        div()
                            .text_color(colors.muted_foreground)
                            .child("Select a container to view details"),
                    ),
            )
    }

    fn render_info_tab(&self, container: &ContainerInfo, cx: &App) -> gpui::Div {
        let colors = &cx.theme().colors;

        let info_row = |label: &str, value: String| {
            h_flex()
                .w_full()
                .py(px(12.))
                .justify_between()
                .border_b_1()
                .border_color(colors.border)
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.foreground)
                        .child(value),
                )
        };

        let status_text = container.status.clone();
        let is_running = container.state.is_running();
        let status_color = if is_running {
            colors.success
        } else {
            colors.danger
        };

        v_flex()
            .w_full()
            .p(px(16.))
            .gap(px(8.))
            .child(info_row("Name", container.name.clone()))
            .child(info_row("ID", container.short_id().to_string()))
            .child(info_row("Image", container.image.clone()))
            .child(
                h_flex()
                    .w_full()
                    .py(px(12.))
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child("Status"),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .w(px(8.))
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(status_color),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .child(status_text),
                            ),
                    ),
            )
            .child(info_row("Ports", container.display_ports()))
            .when(container.command.is_some(), |el| {
                el.child(info_row(
                    "Command",
                    container.command.clone().unwrap_or_default(),
                ))
            })
            .when(container.created.is_some(), |el| {
                el.child(info_row(
                    "Created",
                    container
                        .created
                        .map(|c| c.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default(),
                ))
            })
    }

    fn render_logs_tab(&self, cx: &App) -> gpui::Div {
        let colors = &cx.theme().colors;
        let state = self.container_state.as_ref();
        let is_loading = state.map(|s| s.logs_loading).unwrap_or(false);
        let logs_content = state
            .map(|s| s.logs.clone())
            .unwrap_or_else(|| "No logs available".to_string());

        let on_refresh = self.on_refresh_logs.clone();

        v_flex()
            .w_full()
            .h_full()
            .p(px(16.))
            .gap(px(12.))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.foreground)
                            .child("Container Logs"),
                    )
                    .child(
                        Button::new("refresh-logs")
                            .icon(IconName::Redo)
                            .ghost()
                            .small()
                            .on_click(move |_ev, window, cx| {
                                if let Some(ref cb) = on_refresh {
                                    cb(&(), window, cx);
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(colors.sidebar)
                    .rounded(px(8.))
                    .p(px(12.))
                    .overflow_y_scrollbar()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(colors.foreground)
                    .when(is_loading, |el| el.child("Loading logs..."))
                    .when(!is_loading, |el| el.child(logs_content)),
            )
    }

    fn render_terminal_tab(&self, cx: &App) -> gpui::Div {
        // If we have a terminal view, render it full size
        if let Some(terminal) = &self.terminal_view {
            return div()
                .size_full()
                .flex_1()
                .min_h_0()
                .p(px(8.))
                .child(terminal.clone());
        }

        let colors = &cx.theme().colors;

        // Fallback: show message
        v_flex()
            .flex_1()
            .w_full()
            .p(px(16.))
            .items_center()
            .justify_center()
            .gap(px(16.))
            .child(
                Icon::new(AppIcon::Terminal)
                    .size(px(48.))
                    .text_color(colors.muted_foreground),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Click Terminal tab to connect to container"),
            )
    }

    fn render_inspect_tab(&self, cx: &App) -> gpui::Div {
        let colors = &cx.theme().colors;
        let state = self.container_state.as_ref();
        let is_loading = state.map(|s| s.inspect_loading).unwrap_or(false);
        let inspect_content = state
            .map(|s| s.inspect.clone())
            .unwrap_or_else(|| "{}".to_string());

        v_flex()
            .w_full()
            .h_full()
            .p(px(16.))
            .gap(px(12.))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .child("Container Inspect"),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(colors.sidebar)
                    .rounded(px(8.))
                    .p(px(12.))
                    .overflow_y_scrollbar()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(colors.foreground)
                    .when(is_loading, |el| el.child("Loading..."))
                    .when(!is_loading, |el| el.child(inspect_content)),
            )
    }

    fn render_files_tab(&self, is_running: bool, cx: &App) -> gpui::Div {
        let colors = &cx.theme().colors;

        // Container must be running to browse files
        if !is_running {
            return v_flex()
                .flex_1()
                .w_full()
                .p(px(16.))
                .items_center()
                .justify_center()
                .gap(px(16.))
                .child(
                    Icon::new(AppIcon::Files)
                        .size(px(48.))
                        .text_color(colors.muted_foreground),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("Container must be running to browse files"),
                );
        }

        let current_path = self
            .container_state
            .as_ref()
            .map(|s| s.current_path.clone())
            .unwrap_or_else(|| "/".to_string());

        let files = self
            .container_state
            .as_ref()
            .map(|s| s.files.clone())
            .unwrap_or_default();

        let is_loading = self
            .container_state
            .as_ref()
            .map(|s| s.files_loading)
            .unwrap_or(false);

        let on_navigate = self.on_navigate_path.clone();
        let on_navigate_up = self.on_navigate_path.clone();

        // Calculate parent path
        let parent_path = if current_path == "/" {
            "/".to_string()
        } else {
            let parts: Vec<&str> = current_path.trim_end_matches('/').split('/').collect();
            if parts.len() <= 2 {
                "/".to_string()
            } else {
                parts[..parts.len() - 1].join("/")
            }
        };

        let mut file_list = v_flex().gap(px(2.));

        for file in files.iter() {
            let file_path = file.path.clone();
            let is_dir = file.is_dir;
            let cb = on_navigate.clone();

            file_list = file_list.child(self.render_file_entry(file, cx).when(is_dir, |el| {
                el.cursor_pointer().when_some(cb, move |el, cb| {
                    let path = file_path.clone();
                    el.on_mouse_down(gpui::MouseButton::Left, move |_ev, window, cx| {
                        cb(&path, window, cx);
                    })
                })
            }));
        }

        v_flex()
            .flex_1()
            .w_full()
            .p(px(16.))
            .gap(px(8.))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("up")
                            .icon(IconName::ArrowUp)
                            .ghost()
                            .compact()
                            .when_some(on_navigate_up, move |btn, cb| {
                                let path = parent_path.clone();
                                btn.on_click(move |_ev, window, cx| {
                                    cb(&path, window, cx);
                                })
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px(px(12.))
                            .py(px(8.))
                            .bg(colors.background)
                            .rounded(px(6.))
                            .text_sm()
                            .text_color(colors.secondary_foreground)
                            .child(current_path),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .p(px(8.))
                    .overflow_y_scrollbar()
                    .when(is_loading, |el| {
                        el.child(
                            div()
                                .p(px(16.))
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("Loading..."),
                        )
                    })
                    .when(!is_loading && files.is_empty(), |el| {
                        el.child(
                            div()
                                .p(px(16.))
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("Directory is empty"),
                        )
                    })
                    .when(!is_loading && !files.is_empty(), |el| el.child(file_list)),
            )
    }

    fn render_file_entry(&self, file: &ContainerFileEntry, cx: &App) -> gpui::Div {
        let colors = &cx.theme().colors;
        let icon = if file.is_dir {
            IconName::Folder
        } else if file.is_symlink {
            IconName::ExternalLink
        } else {
            IconName::File
        };

        let icon_color = if file.is_dir {
            colors.warning
        } else if file.is_symlink {
            colors.info
        } else {
            colors.secondary_foreground
        };

        h_flex()
            .w_full()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(4.))
            .items_center()
            .gap(px(10.))
            .hover(|s| s.bg(colors.sidebar))
            .child(Icon::new(icon).text_color(icon_color))
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.foreground)
                    .child(file.name.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(file.display_size()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .w(px(80.))
                    .child(file.permissions.clone()),
            )
    }

    pub fn render(&self, _window: &mut Window, cx: &App) -> gpui::AnyElement {
        let colors = &cx.theme().colors;

        let Some(container) = &self.container else {
            return self.render_empty(cx).into_any_element();
        };

        let is_running = container.state.is_running();
        let container_id = container.id.clone();
        let container_id_for_stop = container_id.clone();
        let container_id_for_restart = container_id.clone();
        let container_id_for_delete = container_id.clone();

        let on_start = self.on_start.clone();
        let on_stop = self.on_stop.clone();
        let on_restart = self.on_restart.clone();
        let on_delete = self.on_delete.clone();
        let on_tab_change = self.on_tab_change.clone();

        let tabs = vec!["Info", "Logs", "Terminal", "Files", "Inspect"];

        // Toolbar with tabs and actions
        let toolbar = h_flex()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .gap(px(12.))
            .items_center()
            .border_b_1()
            .border_color(colors.border)
            .child(
                TabBar::new("container-tabs")
                    .flex_1()
                    .children(tabs.iter().enumerate().map(|(i, label)| {
                        let on_tab_change = on_tab_change.clone();
                        Tab::new()
                            .label(label.to_string())
                            .selected(self.active_tab == i)
                            .on_click(move |_ev, window, cx| {
                                if let Some(ref cb) = on_tab_change {
                                    cb(&i, window, cx);
                                }
                            })
                    })),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .when(!is_running, |el| {
                        let on_start = on_start.clone();
                        let id = container_id.clone();
                        el.child(
                            Button::new("start")
                                .icon(Icon::new(AppIcon::Play))
                                .ghost()
                                .small()
                                .on_click(move |_ev, window, cx| {
                                    if let Some(ref cb) = on_start {
                                        cb(&id, window, cx);
                                    }
                                }),
                        )
                    })
                    .when(is_running, |el| {
                        let on_stop = on_stop.clone();
                        let id = container_id_for_stop.clone();
                        el.child(
                            Button::new("stop")
                                .icon(Icon::new(AppIcon::Stop))
                                .ghost()
                                .small()
                                .on_click(move |_ev, window, cx| {
                                    if let Some(ref cb) = on_stop {
                                        cb(&id, window, cx);
                                    }
                                }),
                        )
                    })
                    .child({
                        let on_restart = on_restart.clone();
                        let id = container_id_for_restart.clone();
                        Button::new("restart")
                            .icon(Icon::new(AppIcon::Restart))
                            .ghost()
                            .small()
                            .on_click(move |_ev, window, cx| {
                                if let Some(ref cb) = on_restart {
                                    cb(&id, window, cx);
                                }
                            })
                    })
                    .child({
                        let on_delete = on_delete.clone();
                        let id = container_id_for_delete.clone();
                        Button::new("delete")
                            .icon(Icon::new(AppIcon::Trash))
                            .ghost()
                            .small()
                            .on_click(move |_ev, window, cx| {
                                if let Some(ref cb) = on_delete {
                                    cb(&id, window, cx);
                                }
                            })
                    }),
            );

        // Content based on active tab
        let content = match self.active_tab {
            0 => self.render_info_tab(container, cx),
            1 => self.render_logs_tab(cx),
            2 => self.render_terminal_tab(cx),
            3 => self.render_files_tab(is_running, cx),
            4 => self.render_inspect_tab(cx),
            _ => self.render_info_tab(container, cx),
        };

        // Terminal tab needs full height without scroll
        let is_terminal_tab = self.active_tab == 2;

        let mut result = div()
            .size_full()
            .bg(colors.sidebar)
            .flex()
            .flex_col()
            .child(toolbar);

        if is_terminal_tab {
            result = result.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(content),
            );
        } else {
            result = result.child(
                div()
                    .id("container-detail-scroll")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(content)
                    .child(div().h(px(100.))),
            );
        }

        result.into_any_element()
    }
}
