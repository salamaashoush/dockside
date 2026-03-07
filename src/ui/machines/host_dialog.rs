//! Host Docker daemon configuration dialog
//!
//! Settings panel for native Docker on Linux, similar to Docker Desktop.
//! Allows configuring daemon.json settings with a user-friendly interface.

use gpui::{App, Context, Entity, FocusHandle, Focusable, Hsla, ParentElement, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Disableable, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  switch::Switch,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::docker::DockerHostInfo;

/// Tab indices for host dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum HostDialogTab {
  #[default]
  DockerEngine = 0,
  Registries = 1,
  Network = 2,
  Advanced = 3,
}

impl HostDialogTab {
  fn all() -> Vec<Self> {
    vec![Self::DockerEngine, Self::Registries, Self::Network, Self::Advanced]
  }

  fn label(&self) -> &'static str {
    match self {
      Self::DockerEngine => "Docker Engine",
      Self::Registries => "Registries",
      Self::Network => "Network",
      Self::Advanced => "Advanced",
    }
  }
}

/// Theme colors struct for passing to helper methods (same pattern as MachineDialog)
#[derive(Clone)]
struct DialogColors {
  border: Hsla,
  foreground: Hsla,
  muted_foreground: Hsla,
  background: Hsla,
  sidebar: Hsla,
  danger: Hsla,
  success: Hsla,
  warning: Hsla,
}

/// Host Docker configuration dialog
pub struct HostDialog {
  focus_handle: FocusHandle,
  #[allow(dead_code)]
  host_info: DockerHostInfo,
  active_tab: HostDialogTab,
  initialized: bool,

  // Daemon.json editor
  daemon_json_editor: Option<Entity<InputState>>,
  daemon_json_content: String,
  daemon_json_loading: bool,
  daemon_json_error: Option<String>,
  daemon_json_saved: bool,
  last_synced_content: String,

  // Registries tab
  insecure_registries_input: Option<Entity<InputState>>,
  registry_mirrors_input: Option<Entity<InputState>>,

  // Network tab
  dns_input: Option<Entity<InputState>>,
  http_proxy_input: Option<Entity<InputState>>,
  https_proxy_input: Option<Entity<InputState>>,
  no_proxy_input: Option<Entity<InputState>>,

  // Advanced settings (from daemon.json)
  experimental: bool,
  debug: bool,
  live_restore: bool,
  userland_proxy: bool,
  ip_forward: bool,
  iptables: bool,

  // UI state
  is_saving: bool,
  is_restarting: bool,
}

impl HostDialog {
  pub fn new(host_info: DockerHostInfo, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    // Load daemon.json synchronously for initial content
    let (initial_content, parsed_json) = Self::load_daemon_json_sync();

    // Create the editor with initial content
    let content_for_editor = initial_content.clone();
    let daemon_json_editor = cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("json")
        .line_number(true)
        .default_value(content_for_editor)
    });

    let mut this = Self {
      focus_handle: cx.focus_handle(),
      host_info,
      active_tab: HostDialogTab::DockerEngine,
      initialized: false,
      daemon_json_editor: Some(daemon_json_editor),
      daemon_json_content: initial_content,
      daemon_json_loading: false,
      daemon_json_error: None,
      daemon_json_saved: false,
      last_synced_content: String::new(),
      insecure_registries_input: None,
      registry_mirrors_input: None,
      dns_input: None,
      http_proxy_input: None,
      https_proxy_input: None,
      no_proxy_input: None,
      experimental: false,
      debug: false,
      live_restore: false,
      userland_proxy: true,
      ip_forward: true,
      iptables: true,
      is_saving: false,
      is_restarting: false,
    };

    // Extract settings from parsed JSON
    if let Some(json) = parsed_json {
      this.extract_settings_from_json(&json);
    }

    this
  }

  fn load_daemon_json_sync() -> (String, Option<serde_json::Value>) {
    let path = std::path::Path::new("/etc/docker/daemon.json");
    if path.exists() {
      match std::fs::read_to_string(path) {
        Ok(content) => {
          if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            (serde_json::to_string_pretty(&value).unwrap_or(content), Some(value))
          } else {
            (content, None)
          }
        }
        Err(_) => ("{}".to_string(), Some(serde_json::json!({})))
      }
    } else {
      ("{}".to_string(), Some(serde_json::json!({})))
    }
  }

  fn extract_settings_from_json(&mut self, json: &serde_json::Value) {
    self.experimental = json.get("experimental").and_then(|v| v.as_bool()).unwrap_or(false);
    self.debug = json.get("debug").and_then(|v| v.as_bool()).unwrap_or(false);
    self.live_restore = json.get("live-restore").and_then(|v| v.as_bool()).unwrap_or(false);
    self.userland_proxy = json.get("userland-proxy").and_then(|v| v.as_bool()).unwrap_or(true);
    self.ip_forward = json.get("ip-forward").and_then(|v| v.as_bool()).unwrap_or(true);
    self.iptables = json.get("iptables").and_then(|v| v.as_bool()).unwrap_or(true);
  }

  fn ensure_initialized(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.initialized || self.daemon_json_loading {
      return;
    }

    // Parse current JSON to extract values for form fields
    let json: serde_json::Value = serde_json::from_str(&self.daemon_json_content)
      .unwrap_or_else(|_| serde_json::json!({}));

    // Initialize registries inputs
    let registries = json
      .get("insecure-registries")
      .and_then(|v| v.as_array())
      .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
      .unwrap_or_default();
    self.insecure_registries_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("e.g., localhost:5000, 192.168.1.100:5000")
        .default_value(registries)
    }));

    let mirrors = json
      .get("registry-mirrors")
      .and_then(|v| v.as_array())
      .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
      .unwrap_or_default();
    self.registry_mirrors_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("e.g., https://mirror.gcr.io")
        .default_value(mirrors)
    }));

    // Initialize network inputs
    let dns = json
      .get("dns")
      .and_then(|v| v.as_array())
      .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
      .unwrap_or_default();
    self.dns_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("e.g., 8.8.8.8, 8.8.4.4")
        .default_value(dns)
    }));

    self.http_proxy_input = Some(cx.new(|cx| {
      InputState::new(window, cx).placeholder("http://proxy.example.com:8080")
    }));

    self.https_proxy_input = Some(cx.new(|cx| {
      InputState::new(window, cx).placeholder("https://proxy.example.com:8080")
    }));

    self.no_proxy_input = Some(cx.new(|cx| {
      InputState::new(window, cx).placeholder("localhost,127.0.0.1,.example.com")
    }));

    self.initialized = true;
  }

  fn build_json_from_settings(&self, cx: &App) -> Result<String, String> {
    // Start with current JSON content or empty object
    let mut json: serde_json::Value = serde_json::from_str(&self.daemon_json_content)
      .unwrap_or_else(|_| serde_json::json!({}));

    let obj = json.as_object_mut().ok_or("Invalid JSON structure")?;

    // Update registries
    if let Some(input) = &self.insecure_registries_input {
      let text = input.read(cx).text().to_string();
      let registries: Vec<String> = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
      if registries.is_empty() {
        obj.remove("insecure-registries");
      } else {
        obj.insert("insecure-registries".to_string(), serde_json::json!(registries));
      }
    }

    if let Some(input) = &self.registry_mirrors_input {
      let text = input.read(cx).text().to_string();
      let mirrors: Vec<String> = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
      if mirrors.is_empty() {
        obj.remove("registry-mirrors");
      } else {
        obj.insert("registry-mirrors".to_string(), serde_json::json!(mirrors));
      }
    }

    // Update DNS
    if let Some(input) = &self.dns_input {
      let text = input.read(cx).text().to_string();
      let dns: Vec<String> = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
      if dns.is_empty() {
        obj.remove("dns");
      } else {
        obj.insert("dns".to_string(), serde_json::json!(dns));
      }
    }

    // Update advanced settings - only add non-default values
    if self.experimental {
      obj.insert("experimental".to_string(), serde_json::json!(true));
    } else {
      obj.remove("experimental");
    }

    if self.debug {
      obj.insert("debug".to_string(), serde_json::json!(true));
    } else {
      obj.remove("debug");
    }

    if self.live_restore {
      obj.insert("live-restore".to_string(), serde_json::json!(true));
    } else {
      obj.remove("live-restore");
    }

    if !self.userland_proxy {
      obj.insert("userland-proxy".to_string(), serde_json::json!(false));
    } else {
      obj.remove("userland-proxy");
    }

    if !self.ip_forward {
      obj.insert("ip-forward".to_string(), serde_json::json!(false));
    } else {
      obj.remove("ip-forward");
    }

    if !self.iptables {
      obj.insert("iptables".to_string(), serde_json::json!(false));
    } else {
      obj.remove("iptables");
    }

    serde_json::to_string_pretty(&json).map_err(|e| format!("Failed to serialize: {}", e))
  }

  fn save_config(&mut self, restart: bool, cx: &mut Context<'_, Self>) {
    self.is_saving = true;
    self.daemon_json_error = None;
    self.daemon_json_saved = false;
    cx.notify();

    // Get content from editor if on Docker Engine tab, otherwise build from settings
    let content = if self.active_tab == HostDialogTab::DockerEngine {
      if let Some(editor) = &self.daemon_json_editor {
        editor.read(cx).text().to_string()
      } else {
        self.daemon_json_content.clone()
      }
    } else {
      match self.build_json_from_settings(cx) {
        Ok(json) => json,
        Err(e) => {
          self.daemon_json_error = Some(e);
          self.is_saving = false;
          cx.notify();
          return;
        }
      }
    };

    cx.spawn(async move |this, cx| {
      let result = cx
        .background_executor()
        .spawn(async move {
          // Validate JSON
          if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
            return Err(format!("Invalid JSON: {}", e));
          }

          // Write to daemon.json using sudo tee
          let mut child = std::process::Command::new("sudo")
            .args(["tee", "/etc/docker/daemon.json"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start sudo: {}", e))?;

          {
            use std::io::Write;
            let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
            stdin.write_all(content.as_bytes()).map_err(|e| format!("Failed to write: {}", e))?;
          }

          let status = child.wait().map_err(|e| format!("Failed to wait: {}", e))?;
          if !status.success() {
            return Err("sudo tee failed - check permissions".to_string());
          }

          Ok(content)
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.is_saving = false;
        match result {
          Ok(content) => {
            this.daemon_json_saved = true;
            this.daemon_json_content = content;
            if restart {
              this.restart_docker(cx);
            }
          }
          Err(e) => {
            this.daemon_json_error = Some(e);
          }
        }
        cx.notify();
      });
    })
    .detach();
  }

  fn restart_docker(&mut self, cx: &mut Context<'_, Self>) {
    self.is_restarting = true;
    cx.notify();

    cx.spawn(async move |this, cx| {
      let _ = cx
        .background_executor()
        .spawn(async {
          std::process::Command::new("sudo")
            .args(["systemctl", "restart", "docker"])
            .output()
        })
        .await;

      // Wait a moment for Docker to restart
      cx.background_executor().timer(std::time::Duration::from_secs(2)).await;

      let _ = this.update(cx, |this, cx| {
        this.is_restarting = false;
        cx.notify();
      });
    })
    .detach();
  }

  // Helper to render form rows (same pattern as MachineDialog)
  fn render_form_row(
    label: &'static str,
    description: &'static str,
    content: impl IntoElement,
    colors: &DialogColors,
  ) -> gpui::Div {
    h_flex()
      .w_full()
      .py(px(12.))
      .px(px(16.))
      .justify_between()
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(
        v_flex()
          .gap(px(2.))
          .child(Label::new(label).text_color(colors.foreground))
          .child(div().text_xs().text_color(colors.muted_foreground).child(description)),
      )
      .child(content)
  }

  fn render_section_header(title: &'static str, colors: &DialogColors) -> gpui::Div {
    div()
      .w_full()
      .py(px(8.))
      .px(px(16.))
      .bg(colors.sidebar)
      .child(div().text_xs().text_color(colors.muted_foreground).child(title))
  }

  fn render_docker_engine_tab(&self, colors: &DialogColors) -> impl IntoElement {
    let editor_view = if let Some(ref editor) = self.daemon_json_editor {
      div()
        .h(px(350.))
        .child(Input::new(editor).w_full().h_full())
        .into_any_element()
    } else {
      div()
        .h(px(350.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(colors.muted_foreground)
        .child("Loading daemon.json...")
        .into_any_element()
    };

    v_flex()
      .w_full()
      .gap(px(8.))
      .p(px(16.))
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Configure the Docker daemon by editing the JSON below."),
      )
      .child(editor_view
      )
  }

  fn render_registries_tab(&self, colors: &DialogColors) -> impl IntoElement {
    v_flex()
      .w_full()
      .child(Self::render_section_header("Insecure Registries", colors))
      .child(
        v_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("Allow Docker to pull from registries without TLS verification. Comma-separated list."),
          )
          .when_some(self.insecure_registries_input.clone(), |el, input| {
            el.child(Input::new(&input).w_full())
          }),
      )
      .child(Self::render_section_header("Registry Mirrors", colors))
      .child(
        v_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("Mirror registries to use when pulling images. Comma-separated list of URLs."),
          )
          .when_some(self.registry_mirrors_input.clone(), |el, input| {
            el.child(Input::new(&input).w_full())
          }),
      )
  }

  fn render_network_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    v_flex()
      .w_full()
      .child(Self::render_section_header("DNS", colors))
      .child(
        v_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("Custom DNS servers for containers. Comma-separated list."),
          )
          .when_some(self.dns_input.clone(), |el, input| {
            el.child(Input::new(&input).w_full())
          }),
      )
      .child(Self::render_section_header("Proxy Configuration", colors))
      .child(
        v_flex()
          .w_full()
          .gap(px(8.))
          .p(px(16.))
          .child(Self::render_form_row(
            "HTTP Proxy",
            "Proxy for HTTP connections",
            div().w(px(250.)).when_some(self.http_proxy_input.clone(), |el, input| {
              el.child(Input::new(&input).small().w_full())
            }),
            colors,
          ))
          .child(Self::render_form_row(
            "HTTPS Proxy",
            "Proxy for HTTPS connections",
            div().w(px(250.)).when_some(self.https_proxy_input.clone(), |el, input| {
              el.child(Input::new(&input).small().w_full())
            }),
            colors,
          ))
          .child(Self::render_form_row(
            "No Proxy",
            "Hosts to bypass proxy",
            div().w(px(250.)).when_some(self.no_proxy_input.clone(), |el, input| {
              el.child(Input::new(&input).small().w_full())
            }),
            colors,
          )),
      )
      .child(Self::render_section_header("Network Options", colors))
      .child(Self::render_form_row(
        "IP Forwarding",
        "Enable IP forwarding for containers",
        Switch::new("ip-forward")
          .checked(self.ip_forward)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.ip_forward = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row(
        "iptables",
        "Allow Docker to add iptables rules",
        Switch::new("iptables")
          .checked(self.iptables)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.iptables = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row(
        "Userland Proxy",
        "Use userland proxy for loopback traffic",
        Switch::new("userland-proxy")
          .checked(self.userland_proxy)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.userland_proxy = *checked;
            cx.notify();
          })),
        colors,
      ))
  }

  fn render_advanced_tab(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    v_flex()
      .w_full()
      .child(Self::render_section_header("Features", colors))
      .child(Self::render_form_row(
        "Experimental Features",
        "Enable experimental Docker features",
        Switch::new("experimental")
          .checked(self.experimental)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.experimental = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row(
        "Live Restore",
        "Keep containers running during daemon downtime",
        Switch::new("live-restore")
          .checked(self.live_restore)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.live_restore = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(Self::render_form_row(
        "Debug Mode",
        "Enable debug-level logging",
        Switch::new("debug")
          .checked(self.debug)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.debug = *checked;
            cx.notify();
          })),
        colors,
      ))
      .child(
        div()
          .w_full()
          .p(px(16.))
          .child(
            div()
              .w_full()
              .p(px(12.))
              .rounded(px(6.))
              .bg(colors.sidebar)
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("Changes require Docker daemon restart to take effect."),
          ),
      )
  }

  fn render_footer(&self, colors: &DialogColors, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let has_error = self.daemon_json_error.is_some();
    let is_saved = self.daemon_json_saved;
    let is_saving = self.is_saving;
    let is_restarting = self.is_restarting;

    h_flex()
      .w_full()
      .flex_shrink_0()
      .px(px(16.))
      .py(px(12.))
      .border_t_1()
      .border_color(colors.border)
      .justify_between()
      .items_center()
      // Status messages
      .child(
        div()
          .flex_1()
          .when(has_error, |el| {
            let error = self.daemon_json_error.clone().unwrap_or_default();
            el.child(div().text_sm().text_color(colors.danger).child(format!("Error: {}", error)))
          })
          .when(is_saved && !has_error, |el| {
            el.child(div().text_sm().text_color(colors.success).child("Configuration saved"))
          })
          .when(is_restarting, |el| {
            el.child(div().text_sm().text_color(colors.warning).child("Restarting Docker daemon..."))
          }),
      )
      // Buttons
      .child(
        h_flex()
          .gap(px(8.))
          .child(
            Button::new("apply")
              .label(if is_saving { "Saving..." } else { "Apply" })
              .primary()
              .disabled(is_saving || is_restarting)
              .on_click(cx.listener(|this, _, _, cx| {
                this.save_config(false, cx);
              })),
          )
          .child(
            Button::new("apply-restart")
              .label("Apply & Restart")
              .ghost()
              .disabled(is_saving || is_restarting)
              .on_click(cx.listener(|this, _, _, cx| {
                this.save_config(true, cx);
              })),
          ),
      )
  }
}

impl Focusable for HostDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for HostDialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Initialize other inputs once loading is complete
    self.ensure_initialized(window, cx);

    let theme_colors = cx.theme().colors;
    let colors = DialogColors {
      border: theme_colors.border,
      foreground: theme_colors.foreground,
      muted_foreground: theme_colors.muted_foreground,
      background: theme_colors.background,
      sidebar: theme_colors.sidebar,
      danger: theme_colors.danger,
      success: theme_colors.success,
      warning: theme_colors.warning,
    };

    let current_tab = self.active_tab;

    v_flex()
      .track_focus(&self.focus_handle)
      .w_full()
      .h(px(480.))
      .overflow_hidden()
      // Tab bar
      .child(
        div()
          .w_full()
          .flex_shrink_0()
          .border_b_1()
          .border_color(colors.border)
          .child(
            TabBar::new("host-dialog-tabs").children(HostDialogTab::all().into_iter().map(|tab| {
              Tab::new()
                .label(tab.label())
                .selected(current_tab == tab)
                .on_click(cx.listener(move |this, _, _, cx| {
                  this.active_tab = tab;
                  cx.notify();
                }))
            })),
          ),
      )
      // Tab content
      .child(
        div()
          .id("host-dialog-content")
          .w_full()
          .flex_1()
          .min_h_0()
          .overflow_y_scrollbar()
          .when(current_tab == HostDialogTab::DockerEngine, |el| {
            el.child(self.render_docker_engine_tab(&colors))
          })
          .when(current_tab == HostDialogTab::Registries, |el| {
            el.child(self.render_registries_tab(&colors))
          })
          .when(current_tab == HostDialogTab::Network, |el| {
            el.child(self.render_network_tab(&colors, cx))
          })
          .when(current_tab == HostDialogTab::Advanced, |el| {
            el.child(self.render_advanced_tab(&colors, cx))
          }),
      )
      // Footer
      .child(self.render_footer(&colors, cx))
  }
}
