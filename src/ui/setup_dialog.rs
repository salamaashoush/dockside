use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, Render, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    theme::ActiveTheme,
    v_flex, Icon, IconName, Sizable,
};
use std::process::Command;

/// Check if Colima CLI is installed
pub fn is_colima_installed() -> bool {
    Command::new("colima")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Docker CLI is installed
pub fn is_docker_installed() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Homebrew is installed
pub fn is_homebrew_installed() -> bool {
    Command::new("brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Setup dialog shown when Docker/Colima are not detected
pub struct SetupDialog {
    focus_handle: FocusHandle,
    colima_installed: bool,
    docker_installed: bool,
    homebrew_installed: bool,
}

impl SetupDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            colima_installed: is_colima_installed(),
            docker_installed: is_docker_installed(),
            homebrew_installed: is_homebrew_installed(),
        }
    }

    pub fn refresh_status(&mut self, _cx: &mut Context<Self>) {
        self.colima_installed = is_colima_installed();
        self.docker_installed = is_docker_installed();
        self.homebrew_installed = is_homebrew_installed();
    }

    pub fn is_ready(&self) -> bool {
        self.colima_installed && self.docker_installed
    }

    fn render_status_item(
        &self,
        name: &'static str,
        installed: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let colors = &cx.theme().colors;

        h_flex()
            .w_full()
            .py(px(8.))
            .px(px(12.))
            .gap(px(12.))
            .items_center()
            .rounded(px(6.))
            .bg(if installed {
                colors.success.opacity(0.1)
            } else {
                colors.danger.opacity(0.1)
            })
            .child(
                Icon::new(if installed {
                    IconName::Check
                } else {
                    IconName::Close
                })
                .text_color(if installed {
                    colors.success
                } else {
                    colors.danger
                }),
            )
            .child(
                Label::new(name)
                    .text_color(colors.foreground)
                    .font_weight(gpui::FontWeight::MEDIUM),
            )
            .child(
                div()
                    .flex_1()
                    .text_right()
                    .text_sm()
                    .text_color(if installed {
                        colors.success
                    } else {
                        colors.muted_foreground
                    })
                    .child(if installed { "Installed" } else { "Not Found" }),
            )
    }

    fn render_install_step(
        &self,
        step: usize,
        title: &'static str,
        command: &'static str,
        description: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let cmd = command.to_string();

        v_flex()
            .w_full()
            .gap(px(8.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .w(px(24.))
                            .h(px(24.))
                            .rounded_full()
                            .bg(colors.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(colors.background)
                            .child(step.to_string()),
                    )
                    .child(
                        Label::new(title)
                            .text_color(colors.foreground)
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    ),
            )
            .child(
                div()
                    .ml(px(32.))
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(description),
            )
            .child(
                h_flex()
                    .ml(px(32.))
                    .mt(px(4.))
                    .w_full()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .px(px(12.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .rounded(px(6.))
                            .font_family("monospace")
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(command),
                    )
                    .child(
                        Button::new(("copy", step))
                            .icon(IconName::Copy)
                            .ghost()
                            .small()
                            .on_click(move |_ev, _window, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd.clone()));
                            }),
                    ),
            )
    }
}

impl Focusable for SetupDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SetupDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();

        v_flex()
            .w_full()
            .max_h(px(500.))
            .overflow_y_scrollbar()
            .gap(px(16.))
            // Status section
            .child(
                v_flex()
                    .w_full()
                    .gap(px(8.))
                    .child(
                        Label::new("Status")
                            .text_color(colors.muted_foreground)
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .child(self.render_status_item("Homebrew", self.homebrew_installed, cx))
                    .child(self.render_status_item("Docker CLI", self.docker_installed, cx))
                    .child(self.render_status_item("Colima", self.colima_installed, cx)),
            )
            // Installation instructions
            .child(
                v_flex()
                    .w_full()
                    .gap(px(16.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(colors.border)
                    .child(
                        Label::new("Installation Steps")
                            .text_color(colors.muted_foreground)
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .when(!self.homebrew_installed, |el| {
                        el.child(self.render_install_step(
                            1,
                            "Install Homebrew",
                            "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"",
                            "Homebrew is required to install Docker and Colima on macOS.",
                            cx,
                        ))
                    })
                    .when(!self.docker_installed, |el| {
                        el.child(self.render_install_step(
                            if self.homebrew_installed { 1 } else { 2 },
                            "Install Docker CLI",
                            "brew install docker docker-compose",
                            "Install the Docker command-line tools (no Docker Desktop required).",
                            cx,
                        ))
                    })
                    .when(!self.colima_installed, |el| {
                        el.child(self.render_install_step(
                            if self.homebrew_installed && self.docker_installed { 1 }
                            else if self.homebrew_installed || self.docker_installed { 2 }
                            else { 3 },
                            "Install Colima",
                            "brew install colima",
                            "Colima provides the container runtime for Docker on macOS.",
                            cx,
                        ))
                    })
                    .child(self.render_install_step(
                        if self.homebrew_installed && self.docker_installed && self.colima_installed { 1 }
                        else if !self.homebrew_installed && !self.docker_installed && !self.colima_installed { 4 }
                        else if (!self.homebrew_installed && !self.docker_installed) || (!self.homebrew_installed && !self.colima_installed) || (!self.docker_installed && !self.colima_installed) { 3 }
                        else { 2 },
                        "Start Colima",
                        "colima start",
                        "Start the Colima VM to run containers. Use 'colima start --kubernetes' for Kubernetes support.",
                        cx,
                    )),
            )
            // Help text
            .child(
                div()
                    .w_full()
                    .mt(px(8.))
                    .p(px(12.))
                    .bg(colors.sidebar)
                    .rounded(px(8.))
                    .child(
                        v_flex()
                            .gap(px(4.))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .items_center()
                                    .child(Icon::new(IconName::Info).text_color(colors.primary))
                                    .child(
                                        Label::new("Need help?")
                                            .text_color(colors.foreground)
                                            .font_weight(gpui::FontWeight::MEDIUM),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.muted_foreground)
                                    .child("Visit github.com/abiosoft/colima for documentation and troubleshooting."),
                            ),
                    ),
            )
    }
}
