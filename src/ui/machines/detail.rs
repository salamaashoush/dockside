use gpui::{App, Entity, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::colima::ColimaVm;
use crate::state::{MachineLogType, MachineTabState};
use crate::terminal::TerminalView;
use crate::ui::components::{FileExplorer, FileExplorerConfig, FileExplorerState};

type MachineActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type MachineEditCallback = Rc<dyn Fn(&ColimaVm, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;
type FileNavigateCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type RefreshCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type LogTypeCallback = Rc<dyn Fn(&MachineLogType, &mut Window, &mut App) + 'static>;
type FileSelectCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type SymlinkClickCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

pub struct MachineDetail {
  machine: Option<ColimaVm>,
  active_tab: usize,
  machine_state: Option<MachineTabState>,
  terminal_view: Option<Entity<TerminalView>>,
  logs_editor: Option<Entity<InputState>>,
  file_content_editor: Option<Entity<InputState>>,
  on_start: Option<MachineActionCallback>,
  on_stop: Option<MachineActionCallback>,
  on_restart: Option<MachineActionCallback>,
  on_delete: Option<MachineActionCallback>,
  on_edit: Option<MachineEditCallback>,
  on_tab_change: Option<TabChangeCallback>,
  on_navigate_path: Option<FileNavigateCallback>,
  on_refresh_logs: Option<RefreshCallback>,
  on_log_type_change: Option<LogTypeCallback>,
  on_file_select: Option<FileSelectCallback>,
  on_close_file_viewer: Option<RefreshCallback>,
  on_symlink_click: Option<SymlinkClickCallback>,
}

impl MachineDetail {
  pub fn new() -> Self {
    Self {
      machine: None,
      active_tab: 0,
      machine_state: None,
      terminal_view: None,
      logs_editor: None,
      file_content_editor: None,
      on_start: None,
      on_stop: None,
      on_restart: None,
      on_delete: None,
      on_edit: None,
      on_tab_change: None,
      on_navigate_path: None,
      on_refresh_logs: None,
      on_log_type_change: None,
      on_file_select: None,
      on_close_file_viewer: None,
      on_symlink_click: None,
    }
  }

  pub fn machine(mut self, machine: Option<ColimaVm>) -> Self {
    self.machine = machine;
    self
  }

  pub fn active_tab(mut self, tab: usize) -> Self {
    self.active_tab = tab;
    self
  }

  pub fn machine_state(mut self, state: MachineTabState) -> Self {
    self.machine_state = Some(state);
    self
  }

  pub fn terminal_view(mut self, view: Option<Entity<TerminalView>>) -> Self {
    self.terminal_view = view;
    self
  }

  pub fn logs_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.logs_editor = editor;
    self
  }

  pub fn file_content_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.file_content_editor = editor;
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

  pub fn on_edit<F>(mut self, callback: F) -> Self
  where
    F: Fn(&ColimaVm, &mut Window, &mut App) + 'static,
  {
    self.on_edit = Some(Rc::new(callback));
    self
  }

  pub fn on_tab_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&usize, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  pub fn on_navigate_path<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_navigate_path = Some(Rc::new(callback));
    self
  }

  pub fn on_refresh_logs<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_refresh_logs = Some(Rc::new(callback));
    self
  }

  pub fn on_log_type_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&MachineLogType, &mut Window, &mut App) + 'static,
  {
    self.on_log_type_change = Some(Rc::new(callback));
    self
  }

  pub fn on_file_select<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_file_select = Some(Rc::new(callback));
    self
  }

  pub fn on_close_file_viewer<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_close_file_viewer = Some(Rc::new(callback));
    self
  }

  pub fn on_symlink_click<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_symlink_click = Some(Rc::new(callback));
    self
  }

  fn render_empty(cx: &App) -> gpui::Div {
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
            Icon::new(AppIcon::Machine)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select a machine to view details"),
          ),
      )
  }

  fn render_info_tab(&self, machine: &ColimaVm, cx: &App) -> gpui::Div {
    let status_text = machine.status.to_string();
    let domain = format!("{}.local", machine.name);

    // Get colima version
    let colima_version = self
      .machine_state
      .as_ref()
      .map_or_else(|| "Loading...".to_string(), |s| s.colima_version.clone());

    // Basic info rows
    let mut basic_info = vec![
      ("Name", machine.name.clone()),
      ("Status", status_text),
      ("Domain", domain),
      ("Colima Version", colima_version),
    ];

    if let Some(addr) = &machine.address {
      basic_info.push(("IP Address", addr.clone()));
    }

    // Get real OS info from state if available
    let os_info = self.machine_state.as_ref().and_then(|s| s.os_info.as_ref());

    // Image section - use real OS info
    let image_info = if let Some(os) = os_info {
      vec![
        ("Distro", os.pretty_name.clone()),
        ("Kernel", os.kernel.clone()),
        ("Architecture", os.arch.clone()),
      ]
    } else {
      vec![
        ("Distro", "Loading...".to_string()),
        ("Kernel", "-".to_string()),
        ("Architecture", machine.arch.display_name().to_string()),
      ]
    };

    // Settings section
    let mut settings_info = vec![
      ("CPUs", machine.cpus.to_string()),
      ("Memory (Configured)", format!("{:.0} GB", machine.memory_gb())),
      ("Disk (Configured)", format!("{:.0} GB", machine.disk_gb())),
      ("Driver", machine.display_driver()),
      ("Mount Type", machine.display_mount_type()),
      ("Runtime", machine.runtime.to_string()),
    ];

    if machine.kubernetes {
      settings_info.push(("Kubernetes", "Enabled".to_string()));
    }

    if let Some(socket) = &machine.docker_socket {
      settings_info.push(("Docker Socket", socket.clone()));
    }

    v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .gap(px(12.))
      .child(Self::render_section(None, basic_info, cx))
      .child(Self::render_section(Some("Image"), image_info, cx))
      .child(Self::render_section(Some("Settings"), settings_info, cx))
  }

  fn render_processes_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let is_loading = self.machine_state.as_ref().is_none_or(|s| s.stats_loading);

    let processes = self
      .machine_state
      .as_ref()
      .map(|s| s.processes.clone())
      .filter(|s| !s.is_empty());

    if is_loading {
      return v_flex().flex_1().w_full().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading processes..."),
      );
    }

    let Some(procs) = processes else {
      return v_flex().flex_1().w_full().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No process data available"),
      );
    };

    // Parse process lines
    let lines: Vec<&str> = procs.lines().collect();
    let _header = lines.first().copied().unwrap_or("");
    let data_lines = lines.iter().skip(1);

    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      // Header row
      .child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .border_b_1()
          .border_color(colors.border)
          .bg(colors.sidebar)
          .child(
            div()
              .w(px(80.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .child("USER"),
          )
          .child(
            div()
              .w(px(70.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("PID"),
          )
          .child(
            div()
              .w(px(70.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("CPU %"),
          )
          .child(
            div()
              .w(px(70.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("MEM %"),
          )
          .child(
            div()
              .flex_1()
              .pl(px(16.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .child("COMMAND"),
          ),
      )
      // Process rows
      .child(
        div()
          .id("processes-scroll")
          .flex_1()
          .overflow_y_scrollbar()
          .children(data_lines.filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 11 {
              let user = parts[0];
              let pid = parts[1];
              let cpu = parts[2];
              let mem = parts[3];
              let command = parts[10..].join(" ");

              // Parse CPU/MEM for coloring
              let cpu_val: f64 = cpu.parse().unwrap_or(0.0);
              let mem_val: f64 = mem.parse().unwrap_or(0.0);

              let cpu_color = if cpu_val > 50.0 {
                colors.danger
              } else if cpu_val > 20.0 {
                colors.warning
              } else {
                colors.secondary_foreground
              };

              let mem_color = if mem_val > 50.0 {
                colors.danger
              } else if mem_val > 20.0 {
                colors.warning
              } else {
                colors.secondary_foreground
              };

              Some(
                h_flex()
                  .w_full()
                  .px(px(16.))
                  .py(px(6.))
                  .hover(|s| s.bg(colors.list_hover))
                  .child(
                    div()
                      .w(px(80.))
                      .text_xs()
                      .text_color(colors.foreground)
                      .overflow_hidden()
                      .text_ellipsis()
                      .child(user.to_string()),
                  )
                  .child(
                    div()
                      .w(px(70.))
                      .text_xs()
                      .text_color(colors.secondary_foreground)
                      .text_right()
                      .child(pid.to_string()),
                  )
                  .child(
                    div()
                      .w(px(70.))
                      .text_xs()
                      .text_color(cpu_color)
                      .text_right()
                      .child(cpu.to_string()),
                  )
                  .child(
                    div()
                      .w(px(70.))
                      .text_xs()
                      .text_color(mem_color)
                      .text_right()
                      .child(mem.to_string()),
                  )
                  .child(
                    div()
                      .flex_1()
                      .pl(px(16.))
                      .text_xs()
                      .text_color(colors.secondary_foreground)
                      .overflow_hidden()
                      .text_ellipsis()
                      .whitespace_nowrap()
                      .child(command),
                  ),
              )
            } else {
              None
            }
          })),
      )
  }

  fn render_stats_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let is_loading = self.machine_state.as_ref().is_none_or(|s| s.stats_loading);

    let disk_usage = self
      .machine_state
      .as_ref()
      .map(|s| s.disk_usage.clone())
      .filter(|s| !s.is_empty());

    let memory_info = self
      .machine_state
      .as_ref()
      .map(|s| s.memory_info.clone())
      .filter(|s| !s.is_empty());

    if is_loading {
      return v_flex().flex_1().w_full().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading stats..."),
      );
    }

    v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .gap(px(24.))
      // Memory Section
      .child(Self::render_memory_card(memory_info.as_ref(), cx))
      // Disk Section
      .child(Self::render_disk_card(disk_usage.as_ref(), cx))
  }

  #[allow(clippy::cast_possible_truncation)]
  fn render_memory_card(memory_info: Option<&String>, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    // Parse memory info from "free -h" output
    // Format: total, used, free, shared, buff/cache, available
    let (used, total, percent) = if let Some(info) = memory_info {
      parse_memory_info(info)
    } else {
      ("--".to_string(), "--".to_string(), 0.0)
    };

    let bar_color = if percent > 80.0 {
      colors.danger
    } else if percent > 60.0 {
      colors.warning
    } else {
      colors.primary
    };

    v_flex()
      .gap(px(12.))
      .child(
        h_flex()
          .items_center()
          .justify_between()
          .child(
            h_flex()
              .items_center()
              .gap(px(8.))
              .child(Icon::new(IconName::ChartPie).size(px(16.)).text_color(colors.primary))
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::SEMIBOLD)
                  .text_color(colors.foreground)
                  .child("Memory"),
              ),
          )
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child(format!("{used} / {total}")),
          ),
      )
      // Progress bar
      .child(
        div()
          .w_full()
          .h(px(8.))
          .bg(colors.background)
          .rounded(px(4.))
          .child(
            div()
              .h_full()
              .rounded(px(4.))
              .bg(bar_color)
              .w(gpui::relative(percent as f32 / 100.0)),
          ),
      )
      // Percentage
      .child(
        h_flex()
          .items_center()
          .justify_between()
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("Used"),
          )
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(bar_color)
              .child(format!("{percent:.1}%")),
          ),
      )
  }

  #[allow(clippy::cast_possible_truncation)]
  fn render_disk_card(disk_usage: Option<&String>, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    // Parse disk info from "df -h /" output
    // Format: Filesystem  Size  Used  Avail  Use%  Mounted on
    let (used, total, percent) = if let Some(info) = disk_usage {
      parse_disk_info(info)
    } else {
      ("--".to_string(), "--".to_string(), 0.0)
    };

    let bar_color = if percent > 80.0 {
      colors.danger
    } else if percent > 60.0 {
      colors.warning
    } else {
      colors.success
    };

    v_flex()
      .gap(px(12.))
      .child(
        h_flex()
          .items_center()
          .justify_between()
          .child(
            h_flex()
              .items_center()
              .gap(px(8.))
              .child(Icon::new(IconName::Folder).size(px(16.)).text_color(colors.success))
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::SEMIBOLD)
                  .text_color(colors.foreground)
                  .child("Disk"),
              ),
          )
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child(format!("{used} / {total}")),
          ),
      )
      // Progress bar
      .child(
        div()
          .w_full()
          .h(px(8.))
          .bg(colors.background)
          .rounded(px(4.))
          .child(
            div()
              .h_full()
              .rounded(px(4.))
              .bg(bar_color)
              .w(gpui::relative(percent as f32 / 100.0)),
          ),
      )
      // Percentage
      .child(
        h_flex()
          .items_center()
          .justify_between()
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("Used"),
          )
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(bar_color)
              .child(format!("{percent:.1}%")),
          ),
      )
  }

  fn render_section(header: Option<&str>, rows: Vec<(&str, String)>, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut section = v_flex().gap(px(1.));

    if let Some(title) = header {
      section = section.child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child(title.to_string()),
      );
    }

    let rows_container = v_flex()
      .bg(colors.background)
      .rounded(px(8.))
      .overflow_hidden()
      .children(
        rows
          .into_iter()
          .enumerate()
          .map(|(i, (label, value))| Self::render_section_row(label, value, i == 0, cx)),
      );

    section.child(rows_container)
  }

  fn render_section_row(label: &str, value: String, is_first: bool, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut row = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .items_center()
      .justify_between()
      .child(
        div()
          .text_sm()
          .text_color(colors.secondary_foreground)
          .child(label.to_string()),
      )
      .child(div().text_sm().text_color(colors.foreground).child(value));

    if !is_first {
      row = row.border_t_1().border_color(colors.border);
    }

    row
  }

  fn render_logs_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let is_loading = self.machine_state.as_ref().is_some_and(|s| s.logs_loading);
    let current_log_type = self.machine_state.as_ref().map(|s| s.log_type).unwrap_or_default();
    let on_refresh = self.on_refresh_logs.clone();
    let on_log_type_change = self.on_log_type_change.clone();

    // Log type selector buttons
    let log_type_selector = {
      let on_system = on_log_type_change.clone();
      let on_docker = on_log_type_change.clone();
      let on_containerd = on_log_type_change.clone();

      h_flex()
        .gap(px(4.))
        .child(
          Button::new("log-system")
            .label("System")
            .compact()
            .when(current_log_type == MachineLogType::System, Button::primary)
            .when(current_log_type != MachineLogType::System, ButtonVariants::ghost)
            .when_some(on_system, |btn, cb| {
              btn.on_click(move |_ev, window, cx| {
                cb(&MachineLogType::System, window, cx);
              })
            }),
        )
        .child(
          Button::new("log-docker")
            .label("Docker")
            .compact()
            .when(current_log_type == MachineLogType::Docker, Button::primary)
            .when(current_log_type != MachineLogType::Docker, ButtonVariants::ghost)
            .when_some(on_docker, |btn, cb| {
              btn.on_click(move |_ev, window, cx| {
                cb(&MachineLogType::Docker, window, cx);
              })
            }),
        )
        .child(
          Button::new("log-containerd")
            .label("Containerd")
            .compact()
            .when(current_log_type == MachineLogType::Containerd, Button::primary)
            .when(current_log_type != MachineLogType::Containerd, ButtonVariants::ghost)
            .when_some(on_containerd, |btn, cb| {
              btn.on_click(move |_ev, window, cx| {
                cb(&MachineLogType::Containerd, window, cx);
              })
            }),
        )
    };

    if is_loading {
      return div()
        .size_full()
        .flex()
        .flex_col()
        .child(
          h_flex()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .child(log_type_selector)
            .child(
              Button::new("refresh-logs")
                .icon(IconName::Redo)
                .ghost()
                .compact()
                .opacity(0.5),
            ),
        )
        .child(
          v_flex().flex_1().items_center().justify_center().child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("Loading logs..."),
          ),
        );
    }

    if let Some(ref editor) = self.logs_editor {
      return div()
        .size_full()
        .flex()
        .flex_col()
        .child(
          h_flex()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .child(log_type_selector)
            .child(
              Button::new("refresh-logs")
                .icon(IconName::Redo)
                .ghost()
                .compact()
                .when_some(on_refresh, |btn, cb| {
                  btn.on_click(move |_ev, window, cx| {
                    cb(&(), window, cx);
                  })
                }),
            ),
        )
        .child(
          div()
            .flex_1()
            .min_h_0()
            .child(Input::new(editor).size_full().appearance(false)),
        );
    }

    // Fallback to plain text
    let logs_content = self.machine_state.as_ref().map(|s| s.logs.clone()).unwrap_or_default();

    div()
      .size_full()
      .flex()
      .flex_col()
      .child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .items_center()
          .justify_between()
          .flex_shrink_0()
          .child(log_type_selector)
          .child(
            Button::new("refresh-logs")
              .icon(IconName::Redo)
              .ghost()
              .compact()
              .when_some(on_refresh, |btn, cb| {
                btn.on_click(move |_ev, window, cx| {
                  cb(&(), window, cx);
                })
              }),
          ),
      )
      .child(
        div().flex_1().min_h_0().p(px(16.)).child(
          div()
            .size_full()
            .overflow_y_scrollbar()
            .bg(colors.sidebar)
            .p(px(12.))
            .font_family("monospace")
            .text_xs()
            .text_color(colors.foreground)
            .child(logs_content),
        ),
      )
  }

  fn render_terminal_tab(&self, cx: &App) -> gpui::Div {
    // If we have a terminal view, render it full size
    if let Some(terminal) = &self.terminal_view {
      return div().size_full().flex_1().min_h_0().p(px(8.)).child(terminal.clone());
    }

    let colors = &cx.theme().colors;

    // Fallback: show message when terminal not yet connected
    v_flex()
      .flex_1()
      .w_full()
      .items_center()
      .justify_center()
      .gap(px(16.))
      .child(
        Icon::new(IconName::SquareTerminal)
          .size(px(48.))
          .text_color(colors.muted_foreground),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Connecting to terminal..."),
      )
  }

  fn render_files_tab(&self, window: &mut Window, cx: &App) -> gpui::AnyElement {
    let state = self.machine_state.as_ref();

    let explorer_state = FileExplorerState {
      current_path: state.map_or_else(|| "/".to_string(), |s| s.current_path.clone()),
      is_loading: state.is_some_and(|s| s.files_loading),
      selected_file: state.and_then(|s| s.selected_file.clone()),
      file_content: state.map(|s| s.file_content.clone()).unwrap_or_default(),
      file_content_loading: state.is_some_and(|s| s.file_content_loading),
    };

    let files = state.map(|s| s.files.clone()).unwrap_or_default();

    let mut explorer = FileExplorer::new()
      .files(files)
      .state(explorer_state)
      .config(
        FileExplorerConfig::default()
          .empty_message("Directory is empty")
          .show_owner(true),
      )
      .file_content_editor(self.file_content_editor.clone());

    if let Some(ref cb) = self.on_navigate_path {
      let cb = cb.clone();
      explorer = explorer.on_navigate(move |path, window, cx| {
        cb(path, window, cx);
      });
    }

    if let Some(ref cb) = self.on_file_select {
      let cb = cb.clone();
      explorer = explorer.on_file_select(move |path, window, cx| {
        cb(path, window, cx);
      });
    }

    if let Some(ref cb) = self.on_close_file_viewer {
      let cb = cb.clone();
      explorer = explorer.on_close_viewer(move |(), window, cx| {
        cb(&(), window, cx);
      });
    }

    if let Some(ref cb) = self.on_symlink_click {
      let cb = cb.clone();
      explorer = explorer.on_symlink_click(move |path, window, cx| {
        cb(path, window, cx);
      });
    }

    explorer.render(window, cx)
  }
}

impl MachineDetail {
  pub fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let colors = &cx.theme().colors;

    let Some(machine) = &self.machine else {
      return Self::render_empty(cx).into_any_element();
    };

    let is_running = machine.status.is_running();
    let machine_name = machine.name.clone();
    let machine_name_for_stop = machine_name.clone();
    let machine_name_for_restart = machine_name.clone();
    let machine_name_for_delete = machine_name.clone();

    let on_start = self.on_start.clone();
    let on_stop = self.on_stop.clone();
    let on_restart = self.on_restart.clone();
    let on_delete = self.on_delete.clone();
    let on_edit = self.on_edit.clone();
    let on_tab_change = self.on_tab_change.clone();
    let machine_for_edit = machine.clone();

    let tabs = ["Info", "Processes", "Stats", "Logs", "Terminal", "Files"];

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(8.))
      .gap(px(12.))
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .flex_shrink_0()
      .child(
        TabBar::new("machine-tabs")
          .flex_1()
          .py(px(0.))
          .children(tabs.iter().enumerate().map(|(i, label)| {
            let on_tab_change = on_tab_change.clone();
            Tab::new()
              .label((*label).to_string())
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
            let name = machine_name.clone();
            el.child(
              Button::new("start")
                .icon(Icon::new(AppIcon::Play))
                .ghost()
                .small()
                .on_click(move |_ev, window, cx| {
                  if let Some(ref cb) = on_start {
                    cb(&name, window, cx);
                  }
                }),
            )
          })
          .when(is_running, |el| {
            let on_stop = on_stop.clone();
            let name = machine_name_for_stop.clone();
            el.child(
              Button::new("stop")
                .icon(Icon::new(AppIcon::Stop))
                .ghost()
                .small()
                .on_click(move |_ev, window, cx| {
                  if let Some(ref cb) = on_stop {
                    cb(&name, window, cx);
                  }
                }),
            )
          })
          .child({
            let on_restart = on_restart.clone();
            let name = machine_name_for_restart.clone();
            Button::new("restart")
              .icon(Icon::new(AppIcon::Restart))
              .ghost()
              .small()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_restart {
                  cb(&name, window, cx);
                }
              })
          })
          .child({
            let on_edit = on_edit.clone();
            let machine = machine_for_edit.clone();
            Button::new("edit")
              .icon(Icon::new(IconName::Settings))
              .ghost()
              .small()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_edit {
                  cb(&machine, window, cx);
                }
              })
          })
          .child({
            let on_delete = on_delete.clone();
            let name = machine_name_for_delete.clone();
            Button::new("delete")
              .icon(Icon::new(AppIcon::Trash))
              .ghost()
              .small()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_delete {
                  cb(&name, window, cx);
                }
              })
          }),
      );

    // Terminal, Logs, and Files tabs need full height without scroll (they handle their own scrolling)
    let is_full_height_tab = self.active_tab == 3 || self.active_tab == 4 || self.active_tab == 5;

    let mut result = div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .overflow_hidden()
      .child(toolbar);

    if is_full_height_tab {
      let content = match self.active_tab {
        3 => self.render_logs_tab(cx).into_any_element(),
        4 => self.render_terminal_tab(cx).into_any_element(),
        5 => self.render_files_tab(window, cx),
        _ => self.render_info_tab(machine, cx).into_any_element(),
      };
      result = result.child(div().flex_1().min_h_0().w_full().overflow_hidden().child(content));
    } else {
      let content = match self.active_tab {
        1 => self.render_processes_tab(cx),
        2 => self.render_stats_tab(cx),
        _ => self.render_info_tab(machine, cx),
      };
      result = result.child(
        div()
          .id("machine-detail-scroll")
          .flex_1()
          .min_h_0()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      );
    }

    result.into_any_element()
  }
}

/// Parse memory info from "free -h" output
/// Returns (used, total, `percent_used`)
fn parse_memory_info(info: &str) -> (String, String, f64) {
  // Format:
  //               total        used        free      shared  buff/cache   available
  // Mem:          7.7Gi       1.2Gi       5.8Gi       0.0Ki       760Mi       6.2Gi
  for line in info.lines() {
    let line = line.trim();
    if line.starts_with("Mem:") {
      let parts: Vec<&str> = line.split_whitespace().collect();
      if parts.len() >= 3 {
        let total = parts[1].to_string();
        let used = parts[2].to_string();

        // Parse values to calculate percentage
        let total_val = parse_memory_value(parts[1]);
        let used_val = parse_memory_value(parts[2]);

        let percent = if total_val > 0.0 {
          (used_val / total_val) * 100.0
        } else {
          0.0
        };

        return (used, total, percent);
      }
    }
  }

  ("--".to_string(), "--".to_string(), 0.0)
}

/// Parse memory value like "7.7Gi", "760Mi", "1.2Gi" to bytes
fn parse_memory_value(s: &str) -> f64 {
  let s = s.trim();

  // Find where the number ends and unit begins
  let num_end = s
    .chars()
    .position(|c| !c.is_ascii_digit() && c != '.')
    .unwrap_or(s.len());

  let (num_str, unit) = s.split_at(num_end);
  let value: f64 = num_str.parse().unwrap_or(0.0);

  match unit.to_lowercase().as_str() {
    "gi" | "g" => value * 1024.0 * 1024.0 * 1024.0,
    "mi" | "m" => value * 1024.0 * 1024.0,
    "ki" | "k" => value * 1024.0,
    _ => value,
  }
}

/// Parse disk info from "df -h /" output
/// Returns (used, total, `percent_used`)
fn parse_disk_info(info: &str) -> (String, String, f64) {
  // Format:
  // Filesystem      Size  Used Avail Use% Mounted on
  // /dev/vda1        59G   10G   46G  19% /
  for line in info.lines() {
    let line = line.trim();
    // Skip header
    if line.starts_with("Filesystem") {
      continue;
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 5 {
      let total = parts[1].to_string();
      let used = parts[2].to_string();
      let percent_str = parts[4].trim_end_matches('%');

      let percent: f64 = percent_str.parse().unwrap_or(0.0);

      return (used, total, percent);
    }
  }

  ("--".to_string(), "--".to_string(), 0.0)
}
