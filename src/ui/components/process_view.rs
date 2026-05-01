//! Reusable process view component with real-time updates
//!
//! Displays a table of running processes with:
//! - Real-time polling updates
//! - Sortable columns (PID, CPU, MEM, Command)
//! - Search/filter functionality
//! - Color-coded resource usage
//! - Works with both Colima VMs and Docker containers

use gpui::{
  App, Context, Entity, InteractiveElement, MouseButton, ParentElement, Render, SharedString, Styled, Window, div,
  prelude::*, px,
};
use gpui_component::{
  Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  menu::{DropdownMenu, PopupMenuItem},
  theme::ActiveTheme,
  v_flex,
};
use std::time::Duration;
use tracing::error;

use crate::colima::ColimaClient;
use crate::services;

/// A single process entry with parsed fields from ps aux
#[derive(Debug, Clone, Default)]
pub struct Process {
  pub user: String,
  pub pid: u32,
  pub cpu_percent: f64,
  pub mem_percent: f64,
  pub command: String,
}

impl Process {
  /// Parse a line from `ps aux` output or Docker top output
  /// Standard ps aux Format: USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND
  /// Also handles variations like: UID PID PPID C STIME TTY TIME CMD (ps -ef format)
  pub fn from_ps_aux_line(line: &str) -> Option<Self> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    // Need at least 2 parts (user/uid and pid)
    if parts.len() < 2 {
      return None;
    }

    // Try to detect the format based on column count and content
    // Docker top with aux: USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND (11+ columns)
    // ps -ef format: UID PID PPID C STIME TTY TIME CMD (8+ columns)
    // Minimal format: PID CMD (2+ columns)

    let (user, pid, cpu, mem, command) = if parts.len() >= 11 {
      // Full ps aux format
      let cmd = parts[10..].join(" ");
      (
        parts[0].to_string(),
        parts[1].parse().unwrap_or(0),
        parts[2].parse().unwrap_or(0.0),
        parts[3].parse().unwrap_or(0.0),
        cmd,
      )
    } else if parts.len() >= 8 {
      // ps -ef format or similar - less columns, no CPU/MEM
      let cmd = parts[7..].join(" ");
      (
        parts[0].to_string(),
        parts[1].parse().unwrap_or(0),
        0.0, // No CPU info in this format
        0.0, // No MEM info in this format
        cmd,
      )
    } else if parts.len() >= 4 {
      // Minimal format with at least user, pid, and some command
      let cmd = parts[3..].join(" ");
      (parts[0].to_string(), parts[1].parse().unwrap_or(0), 0.0, 0.0, cmd)
    } else {
      // Very minimal - just PID and command
      let pid = parts[0].parse().ok().or_else(|| parts[1].parse().ok())?;
      let cmd = if parts[0].parse::<u32>().is_ok() {
        parts[1..].join(" ")
      } else {
        parts[2..].join(" ")
      };
      ("?".to_string(), pid, 0.0, 0.0, cmd)
    };

    // Skip if pid is 0 (parsing failed)
    if pid == 0 {
      return None;
    }

    Some(Self {
      user,
      pid,
      cpu_percent: cpu,
      mem_percent: mem,
      command,
    })
  }
}

/// Parse raw ps aux output into a list of processes
pub fn parse_ps_aux(output: &str) -> Vec<Process> {
  output
    .lines()
    .skip(1) // Skip header
    .filter_map(Process::from_ps_aux_line)
    .collect()
}

/// Source of process data
#[derive(Debug, Clone)]
pub enum ProcessSource {
  /// Colima VM processes via SSH
  ColimaVm { profile: Option<String> },
  /// Docker container processes via docker exec ps aux
  DockerContainer { container_id: String },
  /// Host system processes (native Docker on Linux)
  Host,
}

/// Column to sort by
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
  User,
  Pid,
  #[default]
  Cpu,
  Mem,
  Command,
}

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
  Ascending,
  #[default]
  Descending,
}

/// Kill a process asynchronously (called from dropdown menu)
fn kill_process_async(source: ProcessSource, pid: u32, signal: &str, cx: &mut App) {
  tracing::info!("Kill process requested: pid={pid}, signal={signal}, source={source:?}");

  // Convert signal name to number for better compatibility
  let signal_num = match signal {
    "TERM" => "15",
    "KILL" => "9",
    s => s,
  };
  let signal_num = signal_num.to_string();

  cx.spawn(async move |cx| {
    let result = match &source {
      ProcessSource::ColimaVm { profile } => {
        let profile = profile.clone();
        // Use sudo for Colima VMs since processes may be owned by root
        let cmd = format!("sudo kill -{signal_num} {pid}");
        tracing::debug!("Killing process in VM: {cmd}");
        cx.background_executor()
          .spawn(async move { ColimaClient::run_command(profile.as_deref(), &cmd) })
          .await
      }
      ProcessSource::DockerContainer { container_id } => {
        let tokio_handle = services::Tokio::runtime_handle();
        let client = services::docker_client();
        let container_id = container_id.clone();
        let kill_cmd = format!("kill -{signal_num} {pid}");

        tracing::debug!("Killing process {pid} in container {container_id}: {kill_cmd}");
        cx.background_executor()
          .spawn(async move {
            tokio_handle.block_on(async {
              let guard = client.read().await;
              match guard.as_ref() {
                Some(c) => c.exec_command(&container_id, vec!["/bin/sh", "-c", &kill_cmd]).await,
                None => Err(anyhow::anyhow!("Docker client not connected")),
              }
            })
          })
          .await
      }
      ProcessSource::Host => {
        // Kill process on local system (requires sudo for processes owned by other users)
        tracing::debug!("Killing local process {pid} with signal {signal_num}");
        cx.background_executor()
          .spawn(async move {
            std::process::Command::new("kill")
              .args([&format!("-{signal_num}"), &pid.to_string()])
              .output()
              .map_err(|e| anyhow::anyhow!("Failed to run kill: {e}"))
              .and_then(|output| {
                if output.status.success() {
                  Ok(String::new())
                } else {
                  Err(anyhow::anyhow!(
                    "kill command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                  ))
                }
              })
          })
          .await
      }
    };

    match &result {
      Ok(output) => {
        tracing::info!("Kill process {pid} result: {output}");
      }
      Err(e) => {
        error!("Failed to kill process {pid}: {e}");
      }
    }
  })
  .detach();
}

/// Process view component with real-time updates
pub struct ProcessView {
  source: ProcessSource,
  processes: Vec<Process>,
  filtered_processes: Vec<Process>,
  is_loading: bool,
  error: Option<String>,
  search_input: Option<Entity<InputState>>,
  search_query: String,
  sort_column: SortColumn,
  sort_direction: SortDirection,
  poll_interval: Duration,
}

impl ProcessView {
  pub fn new(source: ProcessSource, _window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let mut view = Self {
      source,
      processes: Vec::new(),
      filtered_processes: Vec::new(),
      is_loading: true,
      error: None,
      search_input: None,
      search_query: String::new(),
      sort_column: SortColumn::Cpu,
      sort_direction: SortDirection::Descending,
      poll_interval: Duration::from_secs(2),
    };

    view.start_polling(cx);
    view
  }

  pub fn for_colima(profile: Option<String>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    Self::new(ProcessSource::ColimaVm { profile }, window, cx)
  }

  pub fn for_container(container_id: String, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    Self::new(ProcessSource::DockerContainer { container_id }, window, cx)
  }

  pub fn for_host(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    Self::new(ProcessSource::Host, window, cx)
  }

  /// Pick the right `ProcessSource` for a `Machine` and return a configured view.
  pub fn for_machine(machine: &crate::colima::Machine, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    if machine.supports_terminal() {
      // Colima — needs SSH access via profile
      Self::for_colima(machine.profile(), window, cx)
    } else {
      // Host — `ps aux` locally
      Self::for_host(window, cx)
    }
  }

  fn ensure_search_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.search_input.is_none() {
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Filter processes..."));
      self.search_input = Some(input_state);
    }
  }

  fn sync_search_query(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(input) = &self.search_input {
      let current_text = input.read(cx).text().to_string();
      if current_text != self.search_query {
        current_text.clone_into(&mut self.search_query);
        self.apply_filter_and_sort();
        cx.notify();
      }
    }
  }

  fn start_polling(&mut self, cx: &mut Context<'_, Self>) {
    // Initial load
    self.refresh(cx);

    // Start polling loop
    let poll_interval = self.poll_interval;
    cx.spawn(async move |this, cx| {
      loop {
        gpui::Timer::after(poll_interval).await;

        let should_continue = this
          .update(cx, |this, cx| {
            this.refresh(cx);
            true
          })
          .unwrap_or(false);

        if !should_continue {
          break;
        }
      }
    })
    .detach();
  }

  fn refresh(&mut self, cx: &mut Context<'_, Self>) {
    self.is_loading = true;
    self.error = None;
    cx.notify();

    let source = self.source.clone();

    cx.spawn(async move |this, cx| {
      let result = match &source {
        ProcessSource::ColimaVm { profile } => {
          // Run in background executor for sync colima command
          let profile = profile.clone();
          cx.background_executor()
            .spawn(async move { ColimaClient::get_processes(profile.as_deref()) })
            .await
            .map(|output| parse_ps_aux(&output))
        }
        ProcessSource::DockerContainer { container_id } => {
          // Use global docker client and tokio runtime
          let tokio_handle = services::Tokio::runtime_handle();
          let client = services::docker_client();
          let container_id = container_id.clone();

          cx.background_executor()
            .spawn(async move {
              tokio_handle.block_on(async {
                let guard = client.read().await;
                match guard.as_ref() {
                  Some(c) => c.get_container_processes(&container_id).await,
                  None => Err(anyhow::anyhow!("Docker client not connected")),
                }
              })
            })
            .await
            .map(|output| parse_ps_aux(&output))
        }
        ProcessSource::Host => {
          // Run ps aux on local system
          cx.background_executor()
            .spawn(async move {
              std::process::Command::new("ps")
                .args(["aux"])
                .output()
                .map_err(|e| anyhow::anyhow!("Failed to run ps: {e}"))
                .and_then(|output| {
                  if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                  } else {
                    Err(anyhow::anyhow!(
                      "ps command failed: {}",
                      String::from_utf8_lossy(&output.stderr)
                    ))
                  }
                })
            })
            .await
            .map(|output| parse_ps_aux(&output))
        }
      };

      let _ = this.update(cx, |this, cx| {
        this.is_loading = false;

        match result {
          Ok(processes) => {
            this.processes = processes;
            this.error = None;
            this.apply_filter_and_sort();
          }
          Err(e) => {
            this.error = Some(e.to_string());
          }
        }
        cx.notify();
      });
    })
    .detach();
  }

  fn apply_filter_and_sort(&mut self) {
    let query = self.search_query.to_lowercase();

    // Filter
    let mut filtered: Vec<Process> = if query.is_empty() {
      self.processes.clone()
    } else {
      self
        .processes
        .iter()
        .filter(|p| {
          p.user.to_lowercase().contains(&query)
            || p.command.to_lowercase().contains(&query)
            || p.pid.to_string().contains(&query)
        })
        .cloned()
        .collect()
    };

    // Sort
    let sort_col = self.sort_column;
    let sort_dir = self.sort_direction;

    filtered.sort_by(|a, b| {
      let cmp = match sort_col {
        SortColumn::User => a.user.cmp(&b.user),
        SortColumn::Pid => a.pid.cmp(&b.pid),
        SortColumn::Cpu => a
          .cpu_percent
          .partial_cmp(&b.cpu_percent)
          .unwrap_or(std::cmp::Ordering::Equal),
        SortColumn::Mem => a
          .mem_percent
          .partial_cmp(&b.mem_percent)
          .unwrap_or(std::cmp::Ordering::Equal),
        SortColumn::Command => a.command.cmp(&b.command),
      };

      match sort_dir {
        SortDirection::Ascending => cmp,
        SortDirection::Descending => cmp.reverse(),
      }
    });

    self.filtered_processes = filtered;
  }

  fn set_sort(&mut self, column: SortColumn, cx: &mut Context<'_, Self>) {
    if self.sort_column == column {
      // Toggle direction
      self.sort_direction = match self.sort_direction {
        SortDirection::Ascending => SortDirection::Descending,
        SortDirection::Descending => SortDirection::Ascending,
      };
    } else {
      self.sort_column = column;
      self.sort_direction = SortDirection::Descending;
    }
    self.apply_filter_and_sort();
    cx.notify();
  }

  fn render_header_column(
    &self,
    label: &str,
    column: SortColumn,
    width: Option<f32>,
    align_right: bool,
    cx: &App,
  ) -> gpui::Div {
    let colors = cx.theme().colors;
    let is_active = self.sort_column == column;

    let sort_icon = if is_active {
      Some(
        Icon::new(if self.sort_direction == SortDirection::Ascending {
          IconName::ChevronUp
        } else {
          IconName::ChevronDown
        })
        .size(px(12.))
        .text_color(colors.link),
      )
    } else {
      None
    };

    let mut col = h_flex().gap(px(4.)).items_center().cursor_pointer();

    if let Some(w) = width {
      col = col.w(px(w));
    } else {
      col = col.flex_1().pl(px(16.));
    }

    if align_right {
      col = col.justify_end();
    }

    col
      .child(
        div()
          .text_xs()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(if is_active {
            colors.link
          } else {
            colors.muted_foreground
          })
          .child(label.to_string()),
      )
      .children(sort_icon)
  }

  fn render_process_row(&self, process: &Process, row_index: usize, cx: &App) -> impl IntoElement {
    let colors = cx.theme().colors;
    let pid = process.pid;
    let command = process.command.clone();
    let source = self.source.clone();

    let cpu_color = if process.cpu_percent > 50.0 {
      colors.danger
    } else if process.cpu_percent > 20.0 {
      colors.warning
    } else {
      colors.secondary_foreground
    };

    let mem_color = if process.mem_percent > 50.0 {
      colors.danger
    } else if process.mem_percent > 20.0 {
      colors.warning
    } else {
      colors.secondary_foreground
    };

    // Clone source for each menu item
    let source_term = source.clone();
    let source_kill = source.clone();

    // Context menu button for process actions
    let menu_button = Button::new(SharedString::from(format!("process-menu-{row_index}")))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _window, _cx| {
        let cmd_display = if command.len() > 30 {
          format!("{}...", &command[..30])
        } else {
          command.clone()
        };

        let source_term = source_term.clone();
        let source_kill = source_kill.clone();

        menu
          .item(
            PopupMenuItem::new(format!("Kill {pid} (TERM)"))
              .icon(Icon::new(IconName::Close))
              .on_click({
                let source = source_term.clone();
                move |_, _, cx| {
                  kill_process_async(source.clone(), pid, "TERM", cx);
                }
              }),
          )
          .item(
            PopupMenuItem::new(format!("Kill {pid} (KILL)"))
              .icon(Icon::new(IconName::CircleX))
              .on_click({
                let source = source_kill.clone();
                move |_, _, cx| {
                  kill_process_async(source.clone(), pid, "KILL", cx);
                }
              }),
          )
          .separator()
          .item(
            PopupMenuItem::new("Copy PID")
              .icon(Icon::new(IconName::Copy))
              .on_click({
                move |_, _, cx| {
                  cx.write_to_clipboard(gpui::ClipboardItem::new_string(pid.to_string()));
                }
              }),
          )
          .item(
            PopupMenuItem::new("Copy Command")
              .icon(Icon::new(IconName::Copy))
              .on_click({
                let cmd = cmd_display.clone();
                move |_, _, cx| {
                  cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd.clone()));
                }
              }),
          )
      });

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
          .child(process.user.clone()),
      )
      .child(
        div()
          .w(px(70.))
          .text_xs()
          .text_color(colors.secondary_foreground)
          .text_right()
          .child(process.pid.to_string()),
      )
      .child(
        div()
          .w(px(70.))
          .text_xs()
          .text_color(cpu_color)
          .text_right()
          .child(format!("{:.1}", process.cpu_percent)),
      )
      .child(
        div()
          .w(px(70.))
          .text_xs()
          .text_color(mem_color)
          .text_right()
          .child(format!("{:.1}", process.mem_percent)),
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
          .child(process.command.clone()),
      )
      .child(
        div()
          .w(px(32.))
          .flex_shrink_0()
          .items_center()
          .justify_center()
          .child(menu_button),
      )
  }
}

impl Render for ProcessView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;

    // Ensure search input exists and sync query
    self.ensure_search_input(window, cx);
    self.sync_search_query(cx);

    // Loading state (only show if no data yet)
    if self.is_loading && self.processes.is_empty() {
      return v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Loading processes..."),
        )
        .into_any_element();
    }

    // Error state
    if let Some(ref err) = self.error {
      return v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(16.))
        .child(Icon::new(IconName::CircleX).size(px(48.)).text_color(colors.danger))
        .child(
          div()
            .text_sm()
            .text_color(colors.danger)
            .max_w(px(400.))
            .text_center()
            .child(err.clone()),
        )
        .child(
          Button::new("retry")
            .label("Retry")
            .primary()
            .on_click(cx.listener(|this, _ev, _window, cx| {
              this.refresh(cx);
            })),
        )
        .into_any_element();
    }

    // Empty state
    if self.filtered_processes.is_empty() {
      return v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child(if self.search_query.is_empty() {
              "No processes found"
            } else {
              "No processes match your search"
            }),
        )
        .into_any_element();
    }

    // Main view
    let process_count = self.filtered_processes.len();
    let total_count = self.processes.len();
    let is_filtered = !self.search_query.is_empty();

    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      // Toolbar
      .child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .gap(px(12.))
          .items_center()
          .border_b_1()
          .border_color(colors.border)
          // Search input
          .child(div().w(px(200.)).when_some(self.search_input.clone(), |el, input| {
            el.child(Input::new(&input).small().w_full())
          }))
          // Clear search button
          .when(!self.search_query.is_empty(), |el| {
            el.child(
              Button::new("clear-search")
                .icon(IconName::Close)
                .ghost()
                .xsmall()
                .on_click(cx.listener(|this, _ev, window, cx| {
                  if let Some(input) = &this.search_input {
                    input.update(cx, |state, cx| {
                      state.set_value("", window, cx);
                    });
                  }
                  this.search_query.clear();
                  this.apply_filter_and_sort();
                  cx.notify();
                })),
            )
          })
          // Process count
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child(if is_filtered {
                format!("{process_count} of {total_count} processes")
              } else {
                format!("{total_count} processes")
              }),
          )
          // Spacer
          .child(div().flex_1())
          // Refresh button
          .child(
            Button::new("refresh")
              .icon(IconName::Redo)
              .ghost()
              .xsmall()
              .loading(self.is_loading)
              .on_click(cx.listener(|this, _ev, _window, cx| {
                this.refresh(cx);
              })),
          )
          // Status indicator
          .child(
            h_flex()
              .gap(px(4.))
              .items_center()
              .child(
                div()
                  .w(px(6.))
                  .h(px(6.))
                  .rounded_full()
                  .bg(if self.is_loading { colors.warning } else { colors.success }),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(if self.is_loading { "Updating..." } else { "Live" }),
              ),
          ),
      )
      // Header with sortable columns
      .child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .border_b_1()
          .border_color(colors.border)
          .bg(colors.sidebar)
          .child(
            self
              .render_header_column("USER", SortColumn::User, Some(80.), false, cx)
              .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| this.set_sort(SortColumn::User, cx))),
          )
          .child(
            self
              .render_header_column("PID", SortColumn::Pid, Some(70.), true, cx)
              .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| this.set_sort(SortColumn::Pid, cx))),
          )
          .child(
            self
              .render_header_column("CPU %", SortColumn::Cpu, Some(70.), true, cx)
              .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| this.set_sort(SortColumn::Cpu, cx))),
          )
          .child(
            self
              .render_header_column("MEM %", SortColumn::Mem, Some(70.), true, cx)
              .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, _window, cx| this.set_sort(SortColumn::Mem, cx))),
          )
          .child(
            self.render_header_column("COMMAND", SortColumn::Command, None, false, cx).on_mouse_down(
              MouseButton::Left,
              cx.listener(|this, _ev, _window, cx| this.set_sort(SortColumn::Command, cx)),
            ),
          ),
      )
      // Process rows
      .child(
        div()
          .id("process-list")
          .flex_1()
          .overflow_y_scroll()
          .children(self.filtered_processes.iter().enumerate().map(|(i, p)| self.render_process_row(p, i, cx))),
      )
      .into_any_element()
  }
}
