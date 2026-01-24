use gpui::{App, Context, FocusHandle, Focusable, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
    Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    theme::ActiveTheme,
    v_flex,
};
use std::path::Path;

use crate::platform::Platform;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::utils::colima_cmd;
use crate::utils::{find_binary, kubectl_cmd};

/// K8s diagnostic result
#[derive(Debug, Clone, Default)]
pub struct K8sDiagnostic {
    pub kubectl_installed: bool,
    pub current_context: Option<String>,
    pub expected_context: Option<String>,
    pub context_mismatch: bool,
    pub api_reachable: bool,
    pub error_message: Option<String>,
}

/// Run K8s diagnostics (quick version - NO network calls)
/// Use this for startup checks to avoid blocking the UI
pub fn diagnose_k8s_quick() -> K8sDiagnostic {
    let kubectl_installed = find_binary("kubectl").is_some();

    if !kubectl_installed {
        return K8sDiagnostic {
            kubectl_installed: false,
            error_message: Some("kubectl not installed".to_string()),
            ..Default::default()
        };
    }

    let mut diag = K8sDiagnostic {
        kubectl_installed,
        ..Default::default()
    };

    // Get current context (fast local operation - reads ~/.kube/config)
    if let Ok(output) = kubectl_cmd().args(["config", "current-context"]).output()
        && output.status.success()
    {
        diag.current_context = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    // Check if colima is running and has K8s (fast local operation) - only on supported platforms
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if find_binary("colima").is_some()
            && let Ok(output) = colima_cmd().args(["status", "--json"]).output()
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&stdout) {
                let has_k8s = value["kubernetes"].as_bool().unwrap_or(false);
                if has_k8s {
                    // Expected context is "colima" for default profile
                    diag.expected_context = Some("colima".to_string());
                    // If K8s is enabled on Colima but we can't verify API, assume issue until checked
                    // This ensures dialog shows when K8s is enabled
                    diag.api_reachable = false;
                    diag.error_message = Some("Checking K8s API...".to_string());
                }
            }
        }
    }

    // Check for context mismatch
    if let (Some(current), Some(expected)) = (&diag.current_context, &diag.expected_context) {
        diag.context_mismatch = current != expected && !current.starts_with("colima");
    }

    // NO API check here - it's done async when dialog refreshes

    diag
}

/// Run full K8s API check (may be slow - use in background only)
pub fn check_k8s_api() -> (bool, Option<String>) {
    if find_binary("kubectl").is_some() {
        let api_check = kubectl_cmd()
            .args(["version", "--client=false", "--request-timeout=3s"])
            .output();

        match api_check {
            Ok(output) if output.status.success() => (true, None),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                (
                    false,
                    Some(stderr.lines().next().unwrap_or("API connection failed").to_string()),
                )
            }
            Err(e) => (false, Some(format!("Failed to check API: {e}"))),
        }
    } else {
        (false, Some("kubectl not found".to_string()))
    }
}

/// Check if Colima CLI is installed (only on supported platforms)
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn is_colima_installed() -> bool {
    if find_binary("colima").is_some() {
        colima_cmd()
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn is_colima_installed() -> bool {
    false
}

/// Check if Docker CLI is installed
pub fn is_docker_installed() -> bool {
    if find_binary("docker").is_some() {
        crate::utils::docker_cmd()
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    }
}

/// Check if Docker daemon is accessible (socket exists or TCP available)
pub fn is_docker_running() -> bool {
    let platform = Platform::detect();

    match platform {
        Platform::MacOS | Platform::Linux | Platform::WindowsWsl2 => {
            // Check common socket paths
            let native_socket = "/var/run/docker.sock";
            if Path::new(native_socket).exists() {
                return true;
            }

            // Check Colima socket
            if let Some(home) = dirs::home_dir() {
                let colima_socket = format!("{}/.colima/default/docker.sock", home.display());
                if Path::new(&colima_socket).exists() {
                    return true;
                }
            }

            // Also try docker info as a fallback
            if let Ok(output) = crate::utils::docker_cmd().arg("info").output() {
                return output.status.success();
            }

            false
        }
        Platform::Windows => {
            // On Windows, try TCP connection or docker info
            if let Ok(output) = crate::utils::docker_cmd().arg("info").output() {
                return output.status.success();
            }
            false
        }
    }
}

/// Check if Homebrew is installed (macOS only)
#[cfg(target_os = "macos")]
pub fn is_homebrew_installed() -> bool {
    use std::process::Command;

    const BREW_PATHS: &[&str] = &[
        "/opt/homebrew/bin", // Apple Silicon
        "/usr/local/bin",    // Intel
    ];

    for base in BREW_PATHS {
        let brew_path = Path::new(base).join("brew");
        if brew_path.exists() {
            return Command::new(brew_path)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        }
    }
    false
}

#[cfg(not(target_os = "macos"))]
pub fn is_homebrew_installed() -> bool {
    // Homebrew not required on non-macOS platforms
    true
}

/// Check if any Colima VM is running
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn is_colima_running() -> bool {
    if find_binary("colima").is_none() {
        return false;
    }
    let Ok(output) = colima_cmd().args(["list", "--json"]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|vm| vm["status"].as_str().map(|s| s.eq_ignore_ascii_case("running")))
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn is_colima_running() -> bool {
    false
}

/// Setup dialog shown when Docker/Colima are not detected
pub struct SetupDialog {
    focus_handle: FocusHandle,
    platform: Platform,
    colima_installed: bool,
    colima_running: bool,
    docker_installed: bool,
    docker_running: bool,
    homebrew_installed: bool,
    k8s_diagnostic: K8sDiagnostic,
    action_message: Option<String>,
}

impl SetupDialog {
    pub fn new(cx: &mut Context<'_, Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let platform = Platform::detect();

        let mut dialog = Self {
            focus_handle,
            platform,
            colima_installed: is_colima_installed(),
            colima_running: is_colima_running(),
            docker_installed: is_docker_installed(),
            docker_running: is_docker_running(),
            homebrew_installed: is_homebrew_installed(),
            k8s_diagnostic: diagnose_k8s_quick(),
            action_message: None,
        };

        // If K8s is expected, run async API check
        if dialog.k8s_diagnostic.expected_context.is_some() && !dialog.k8s_diagnostic.context_mismatch {
            dialog.action_message = Some("Checking K8s API...".to_string());
            cx.spawn(async move |this, cx| {
                let (api_ok, error) = cx.background_executor().spawn(async move { check_k8s_api() }).await;

                let _ = this.update(cx, |this, cx| {
                    this.k8s_diagnostic.api_reachable = api_ok;
                    this.k8s_diagnostic.error_message = error;
                    this.action_message = None;
                    cx.notify();
                });
            })
            .detach();
        }

        dialog
    }

    pub fn refresh_status(&mut self, cx: &mut Context<'_, Self>) {
        self.action_message = Some("Checking...".to_string());
        cx.notify();

        // Run all checks in background (including API check)
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let diag = diagnose_k8s_quick();
                    let (api_ok, api_error) = if diag.expected_context.is_some() && !diag.context_mismatch {
                        check_k8s_api()
                    } else {
                        (false, None)
                    };
                    (
                        is_colima_installed(),
                        is_colima_running(),
                        is_docker_installed(),
                        is_docker_running(),
                        is_homebrew_installed(),
                        diag,
                        api_ok,
                        api_error,
                    )
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.colima_installed = result.0;
                this.colima_running = result.1;
                this.docker_installed = result.2;
                this.docker_running = result.3;
                this.homebrew_installed = result.4;
                this.k8s_diagnostic = result.5;
                this.k8s_diagnostic.api_reachable = result.6;
                if result.7.is_some() {
                    this.k8s_diagnostic.error_message = result.7;
                }
                this.action_message = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// Render a requirement row with status
    #[allow(clippy::unused_self)]
    fn render_requirement_row(
        &self,
        name: &'static str,
        installed: bool,
        running: Option<bool>,
        cx: &Context<'_, Self>,
    ) -> impl IntoElement {
        let colors = &cx.theme().colors;

        // Determine status
        let (status_icon, status_color, status_text) = if installed {
            if let Some(is_running) = running {
                if is_running {
                    (IconName::Check, colors.success, "Running")
                } else {
                    (IconName::Minus, colors.warning, "Stopped")
                }
            } else {
                (IconName::Check, colors.success, "Installed")
            }
        } else {
            (IconName::Close, colors.danger, "Missing")
        };

        h_flex()
            .w_full()
            .py(px(6.))
            .px(px(8.))
            .gap(px(8.))
            .items_center()
            .child(Icon::new(status_icon).text_color(status_color))
            .child(Label::new(name).text_color(colors.foreground).text_sm().flex_1())
            .child(div().text_xs().text_color(status_color).child(status_text))
    }

    fn render_action_section(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;

        match self.platform {
            Platform::MacOS => self.render_macos_actions(cx, colors),
            Platform::Linux | Platform::WindowsWsl2 => self.render_linux_actions(cx, colors),
            Platform::Windows => self.render_windows_actions(cx, colors),
        }
    }

    fn render_macos_actions(
        &self,
        cx: &Context<'_, Self>,
        colors: &gpui_component::theme::ThemeColor,
    ) -> gpui::AnyElement {
        let needs_homebrew = !self.homebrew_installed;
        let needs_docker = !self.docker_installed;
        let needs_colima = !self.colima_installed;
        let needs_start = self.colima_installed && !self.colima_running;

        if !needs_homebrew && !needs_docker && !needs_colima && !needs_start {
            return div().into_any_element();
        }

        v_flex()
            .w_full()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(colors.sidebar)
            .when(needs_homebrew || needs_docker || needs_colima, |el| {
                el.child(
                    v_flex()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            Label::new("Install missing dependencies")
                                .text_color(colors.foreground)
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM),
                        )
                        .when(needs_homebrew, |el| {
                            let cmd = "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"";
                            el.child(Self::render_command_row("copy-brew", cmd, cx))
                        })
                        .when(needs_docker, |el| {
                            el.child(Self::render_command_row(
                                "copy-docker",
                                "brew install docker docker-compose",
                                cx,
                            ))
                        })
                        .when(needs_colima, |el| {
                            el.child(Self::render_command_row("copy-colima", "brew install colima", cx))
                        }),
                )
            })
            .when(needs_start, |el| {
                el.child(self.render_colima_start_section(cx, colors))
            })
            .into_any_element()
    }

    fn render_linux_actions(
        &self,
        cx: &Context<'_, Self>,
        colors: &gpui_component::theme::ThemeColor,
    ) -> gpui::AnyElement {
        let needs_docker = !self.docker_installed;
        let docker_not_running = self.docker_installed && !self.docker_running;

        if !needs_docker && !docker_not_running {
            return div().into_any_element();
        }

        v_flex()
            .w_full()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(colors.sidebar)
            .when(needs_docker, |el| {
                el.child(
                    v_flex()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            Label::new("Install Docker")
                                .text_color(colors.foreground)
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("Choose one of the following methods:"),
                        )
                        // Ubuntu/Debian
                        .child(Self::render_command_row(
                            "copy-docker-apt",
                            "sudo apt install docker.io docker-compose",
                            cx,
                        ))
                        // Fedora/RHEL
                        .child(Self::render_command_row(
                            "copy-docker-dnf",
                            "sudo dnf install docker docker-compose",
                            cx,
                        ))
                        // Arch
                        .child(Self::render_command_row(
                            "copy-docker-pacman",
                            "sudo pacman -S docker docker-compose",
                            cx,
                        )),
                )
            })
            .when(docker_not_running, |el| {
                el.child(
                    v_flex()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            Label::new("Start Docker daemon")
                                .text_color(colors.foreground)
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM),
                        )
                        .child(Self::render_command_row(
                            "copy-docker-start",
                            "sudo systemctl start docker",
                            cx,
                        ))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("To enable on boot: sudo systemctl enable docker"),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_windows_actions(
        &self,
        cx: &Context<'_, Self>,
        colors: &gpui_component::theme::ThemeColor,
    ) -> gpui::AnyElement {
        let needs_docker = !self.docker_installed;
        let docker_not_running = self.docker_installed && !self.docker_running;

        if !needs_docker && !docker_not_running {
            return div().into_any_element();
        }

        v_flex()
            .w_full()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(colors.sidebar)
            .when(needs_docker || docker_not_running, |el| {
                el.child(
                    v_flex()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            Label::new("Docker Setup for Windows")
                                .text_color(colors.foreground)
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("Dockside connects to Docker running in WSL2."),
                        )
                        .child(
                            v_flex()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.foreground)
                                        .child("1. Install Docker in WSL2:"),
                                )
                                .child(Self::render_command_row(
                                    "copy-wsl-docker",
                                    "wsl -d Ubuntu -e sudo apt install docker.io",
                                    cx,
                                ))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.foreground)
                                        .child("2. Enable TCP in /etc/docker/daemon.json:"),
                                )
                                .child(Self::render_command_row(
                                    "copy-daemon-json",
                                    "{\"hosts\": [\"unix:///var/run/docker.sock\", \"tcp://0.0.0.0:2375\"]}",
                                    cx,
                                ))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.foreground)
                                        .child("3. Start Docker daemon:"),
                                )
                                .child(Self::render_command_row(
                                    "copy-wsl-start",
                                    "wsl -d Ubuntu -e sudo dockerd &",
                                    cx,
                                )),
                        ),
                )
            })
            .into_any_element()
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn render_colima_start_section(
        &self,
        _cx: &Context<'_, Self>,
        colors: &gpui_component::theme::ThemeColor,
    ) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap(px(8.))
            .child(
                Label::new("Start Colima")
                    .text_color(colors.foreground)
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        Button::new("start-colima")
                            .label("Start")
                            .primary()
                            .small()
                            .on_click(|_ev, _window, cx| {
                                crate::services::start_machine("default".to_string(), cx);
                            }),
                    )
                    .child(
                        Button::new("start-colima-k8s")
                            .label("Start with Kubernetes")
                            .ghost()
                            .small()
                            .on_click(|_ev, _window, cx| {
                                let mut config = crate::colima::ColimaConfig::default();
                                config.kubernetes.enabled = true;
                                crate::services::create_machine("default".to_string(), config, cx);
                            }),
                    ),
            )
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn render_colima_start_section(
        &self,
        _cx: &Context<'_, Self>,
        _colors: &gpui_component::theme::ThemeColor,
    ) -> impl IntoElement {
        div()
    }

    fn render_command_row(id: &'static str, command: &'static str, cx: &Context<'_, Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let cmd = command.to_string();

        h_flex()
            .w_full()
            .gap(px(8.))
            .items_center()
            .child(
                div()
                    .flex_1()
                    .px(px(10.))
                    .py(px(6.))
                    .bg(colors.background)
                    .rounded(px(4.))
                    .font_family("monospace")
                    .text_xs()
                    .text_color(colors.foreground)
                    .overflow_hidden()
                    .child(command),
            )
            .child(
                Button::new(id)
                    .icon(IconName::Copy)
                    .ghost()
                    .xsmall()
                    .on_click(move |_ev, _window, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd.clone()));
                    }),
            )
    }

    fn render_k8s_diagnostic(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let diag = &self.k8s_diagnostic;

        // Determine if there are issues
        let has_issues =
            !diag.kubectl_installed || diag.context_mismatch || (diag.expected_context.is_some() && !diag.api_reachable);

        if !has_issues {
            return div().into_any_element();
        }

        v_flex()
            .w_full()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(colors.warning.opacity(0.5))
            .bg(colors.warning.opacity(0.05))
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(Icon::new(IconName::Info).text_color(colors.warning).size(px(14.)))
                    .child(
                        Label::new("Kubernetes Issue")
                            .text_color(colors.warning)
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    ),
            )
            // Show action message if any
            .when_some(self.action_message.clone(), |el, msg| {
                el.child(div().text_xs().text_color(colors.link).child(msg))
            })
            // Context mismatch
            .when(diag.context_mismatch, |el| {
                let current = diag.current_context.clone().unwrap_or_default();
                let expected = diag.expected_context.clone().unwrap_or_else(|| "colima".to_string());

                el.child(
                    v_flex()
                        .w_full()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(format!("Context: {current} (expected: {expected})")),
                        )
                        .child(
                            Button::new("fix-context")
                                .label(format!("Switch to {expected}"))
                                .primary()
                                .xsmall()
                                .on_click({
                                    let ctx = expected.clone();
                                    move |_ev, _window, cx| {
                                        crate::services::switch_kubectl_context(ctx.clone(), cx);
                                    }
                                }),
                        ),
                )
            })
            // K8s API not reachable - only show Colima-specific actions on supported platforms
            .when(
                diag.expected_context.is_some() && !diag.api_reachable && !diag.context_mismatch,
                |el| {
                    let error = diag.error_message.clone().unwrap_or_else(|| "API not responding".to_string());

                    el.child(
                        v_flex()
                            .w_full()
                            .gap(px(6.))
                            .child(div().text_xs().text_color(colors.muted_foreground).child(error))
                            .when(self.platform.supports_colima(), |el| {
                                el.child(
                                    h_flex()
                                        .gap(px(8.))
                                        .child(
                                            Button::new("reset-k8s")
                                                .label("Reset K8s")
                                                .primary()
                                                .small()
                                                .on_click(move |_ev, _window, cx| {
                                                    crate::services::reset_colima_kubernetes(cx);
                                                }),
                                        )
                                        .child(
                                            Button::new("restart-colima")
                                                .label("Restart Colima")
                                                .ghost()
                                                .small()
                                                .on_click(move |_ev, _window, cx| {
                                                    crate::services::restart_machine("default".to_string(), cx);
                                                }),
                                        ),
                                )
                            }),
                    )
                },
            )
            .into_any_element()
    }
}

impl Focusable for SetupDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SetupDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let platform = self.platform;

        v_flex()
            .w_full()
            .max_h(px(400.))
            .overflow_y_scrollbar()
            .gap(px(8.))
            // Requirements section - platform specific
            .child(
                v_flex()
                    .w_full()
                    .gap(px(2.))
                    // Docker is always required
                    .child(self.render_requirement_row(
                        "Docker CLI",
                        self.docker_installed,
                        if self.docker_installed {
                            Some(self.docker_running)
                        } else {
                            None
                        },
                        cx,
                    ))
                    // Homebrew only on macOS
                    .when(platform == Platform::MacOS, |el| {
                        el.child(self.render_requirement_row("Homebrew", self.homebrew_installed, None, cx))
                    })
                    // Colima only on macOS and Linux (and optional on Linux)
                    .when(platform.supports_colima(), |el| {
                        let label = if platform == Platform::MacOS {
                            "Colima"
                        } else {
                            "Colima (optional)"
                        };
                        el.child(self.render_requirement_row(
                            label,
                            self.colima_installed,
                            if self.colima_installed {
                                Some(self.colima_running)
                            } else {
                                None
                            },
                            cx,
                        ))
                    }),
            )
            // Action section (only shows if something needs to be done)
            .child(self.render_action_section(cx))
            // K8s diagnostics section
            .child(self.render_k8s_diagnostic(cx))
    }
}
