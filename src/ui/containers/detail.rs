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

// Re-export from state module for backwards compatibility
pub use crate::state::ContainerDetailTab;

/// One-line tooltip summary for a sparkline series. Shared between
/// the Activity Monitor and the per-container Stats tab.
fn sparkline_summary(label: &str, series: &[f64], unit: &str) -> String {
  if series.is_empty() {
    return format!("{label} no data");
  }
  let last = *series.last().unwrap();
  let min = series.iter().copied().fold(f64::INFINITY, f64::min);
  let max = series.iter().copied().fold(f64::NEG_INFINITY, f64::max);
  #[allow(clippy::cast_precision_loss)]
  let avg = series.iter().sum::<f64>() / series.len() as f64;
  format!(
    "{label} last {last:.1}{unit} (min {min:.1}{unit} / max {max:.1}{unit} / avg {avg:.1}{unit})"
  )
}

fn format_bytes(bytes: u64) -> String {
  bytesize::ByteSize(bytes).to_string()
}

use crate::assets::AppIcon;
use crate::docker::{ContainerFileEntry, ContainerInfo};
use crate::terminal::TerminalView;
use crate::ui::components::{FileExplorer, FileExplorerConfig, FileExplorerState, ProcessView};

type ContainerActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&ContainerDetailTab, &mut Window, &mut App) + 'static>;
type RefreshCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type FileNavigateCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type FileSelectCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type CloseViewerCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type SymlinkClickCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type OpenInEditorCallback = Rc<dyn Fn(&(String, bool), &mut Window, &mut App) + 'static>;

/// State for container detail tabs
#[derive(Debug, Clone)]
pub struct ContainerTabState {
  pub logs: String,
  pub logs_loading: bool,
  /// Live-tail (follow) mode toggle. When true the view drives a streaming
  /// task that appends chunks; when false a one-shot snapshot is fetched.
  pub logs_follow: bool,
  /// Include RFC3339 timestamps in log lines.
  pub logs_timestamps: bool,
  pub inspect: String,
  pub inspect_loading: bool,
  pub current_path: String,
  pub files: Vec<ContainerFileEntry>,
  pub files_loading: bool,
  /// Error when listing files failed
  pub files_error: Option<String>,
  /// Selected file path for viewing
  pub selected_file: Option<String>,
  /// Content of selected file
  pub file_content: String,
  /// Whether file content is loading
  pub file_content_loading: bool,
  /// Error when loading file content failed
  pub file_content_error: Option<String>,
  /// Structured extras from container inspect (health, `restart_count`, etc).
  pub container_extras: Option<crate::docker::ContainerExtras>,
  /// Latest container stats sample (None if not yet loaded / unavailable).
  pub stats_latest: Option<crate::docker::ContainerStats>,
  /// Rolling history (last 60 samples) for sparkline charts.
  pub stats_cpu: Vec<f64>,
  pub stats_mem_pct: Vec<f64>,
  pub stats_net: Vec<f64>,
  pub stats_disk: Vec<f64>,
}

impl Default for ContainerTabState {
  fn default() -> Self {
    Self {
      logs: String::new(),
      logs_loading: false,
      logs_follow: true,
      logs_timestamps: false,
      inspect: String::new(),
      inspect_loading: false,
      current_path: String::new(),
      files: Vec::new(),
      files_loading: false,
      files_error: None,
      selected_file: None,
      file_content: String::new(),
      file_content_loading: false,
      file_content_error: None,
      container_extras: None,
      stats_latest: None,
      stats_cpu: Vec::with_capacity(60),
      stats_mem_pct: Vec::with_capacity(60),
      stats_net: Vec::with_capacity(60),
      stats_disk: Vec::with_capacity(60),
    }
  }
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
  active_tab: ContainerDetailTab,
  container_state: Option<ContainerTabState>,
  terminal_view: Option<Entity<TerminalView>>,
  process_view: Option<Entity<ProcessView>>,
  inspect_editor: Option<Entity<InputState>>,
  file_content_editor: Option<Entity<InputState>>,
  on_start: Option<ContainerActionCallback>,
  on_stop: Option<ContainerActionCallback>,
  on_restart: Option<ContainerActionCallback>,
  on_delete: Option<ContainerActionCallback>,
  on_tab_change: Option<TabChangeCallback>,
  on_refresh_logs: Option<RefreshCallback>,
  on_toggle_logs_follow: Option<RefreshCallback>,
  on_toggle_logs_timestamps: Option<RefreshCallback>,
  on_clear_logs: Option<RefreshCallback>,
  logs_terminal: Option<Entity<TerminalView>>,
  on_navigate_path: Option<FileNavigateCallback>,
  on_file_select: Option<FileSelectCallback>,
  on_close_file_viewer: Option<CloseViewerCallback>,
  on_symlink_click: Option<SymlinkClickCallback>,
  on_open_in_editor: Option<OpenInEditorCallback>,
}

impl ContainerDetail {
  pub fn new() -> Self {
    Self {
      container: None,
      active_tab: ContainerDetailTab::Info,
      container_state: None,
      terminal_view: None,
      process_view: None,
      inspect_editor: None,
      file_content_editor: None,
      on_start: None,
      on_stop: None,
      on_restart: None,
      on_delete: None,
      on_tab_change: None,
      on_refresh_logs: None,
      on_toggle_logs_follow: None,
      on_toggle_logs_timestamps: None,
      on_clear_logs: None,
      logs_terminal: None,
      on_navigate_path: None,
      on_file_select: None,
      on_close_file_viewer: None,
      on_symlink_click: None,
      on_open_in_editor: None,
    }
  }

  pub fn container(mut self, container: Option<ContainerInfo>) -> Self {
    self.container = container;
    self
  }

  pub fn active_tab(mut self, tab: ContainerDetailTab) -> Self {
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

  pub fn process_view(mut self, view: Option<Entity<ProcessView>>) -> Self {
    self.process_view = view;
    self
  }

  pub fn inspect_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.inspect_editor = editor;
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

  pub fn on_tab_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&ContainerDetailTab, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  pub fn logs_terminal(mut self, view: Option<Entity<TerminalView>>) -> Self {
    self.logs_terminal = view;
    self
  }

  pub fn on_toggle_logs_follow<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_toggle_logs_follow = Some(Rc::new(callback));
    self
  }

  pub fn on_toggle_logs_timestamps<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_toggle_logs_timestamps = Some(Rc::new(callback));
    self
  }

  pub fn on_clear_logs<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_clear_logs = Some(Rc::new(callback));
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

  pub fn on_open_in_editor<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(String, bool), &mut Window, &mut App) + 'static,
  {
    self.on_open_in_editor = Some(Rc::new(callback));
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
        .child(div().text_sm().text_color(colors.foreground).child(value))
    };

    let status_text = container.status.clone();
    let is_running = container.state.is_running();
    let status_color = if is_running { colors.success } else { colors.danger };

    let extras = self
      .container_state
      .as_ref()
      .and_then(|s| s.container_extras.clone());

    let mut col = v_flex()
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
          .child(div().text_sm().text_color(colors.muted_foreground).child("Status"))
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(status_color))
              .child(div().text_sm().text_color(colors.foreground).child(status_text)),
          ),
      )
      .child(info_row("Ports", container.display_ports()))
      .when(container.command.is_some(), |el| {
        el.child(info_row("Command", container.command.clone().unwrap_or_default()))
      })
      .when(container.created.is_some(), |el| {
        el.child(info_row(
          "Created",
          container
            .created
            .map(|c| c.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default(),
        ))
      });

    if let Some(ex) = extras {
      if let Some(rc) = ex.restart_count {
        col = col.child(info_row("Restart count", rc.to_string()));
      }
      if let Some(ec) = ex.exit_code
        && !is_running
      {
        col = col.child(info_row("Exit code", ec.to_string()));
      }
      if let Some(start) = ex.started_at.as_ref().filter(|s| !s.is_empty() && *s != "0001-01-01T00:00:00Z") {
        col = col.child(info_row("Started", start.clone()));
      }
      if let Some(end) = ex.finished_at.as_ref().filter(|s| !s.is_empty() && *s != "0001-01-01T00:00:00Z") {
        col = col.child(info_row("Finished", end.clone()));
      }

      // Health section.
      if let Some(h) = ex.health.as_ref() {
        let health_color = match h.status.as_str() {
          "healthy" => colors.success,
          "starting" => colors.warning,
          "unhealthy" => colors.danger,
          _ => colors.muted_foreground,
        };
        col = col.child(
          h_flex()
            .w_full()
            .py(px(12.))
            .justify_between()
            .border_b_1()
            .border_color(colors.border)
            .child(div().text_sm().text_color(colors.muted_foreground).child("Health"))
            .child(
              h_flex()
                .gap(px(8.))
                .items_center()
                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(health_color))
                .child(
                  div()
                    .text_sm()
                    .text_color(colors.foreground)
                    .child(if h.status.is_empty() { "n/a".to_string() } else { h.status.clone() }),
                )
                .when_some(h.failing_streak.filter(|n| *n > 0), |el, n| {
                  el.child(
                    div()
                      .text_xs()
                      .text_color(colors.muted_foreground)
                      .child(format!("(failing streak {n})")),
                  )
                }),
            ),
        );
        if !h.log.is_empty() {
          let mut entries = v_flex().w_full().gap(px(4.));
          for entry in h.log.iter().rev().take(5) {
            let exit = entry.exit_code.unwrap_or(0);
            let line_color = if exit == 0 { colors.success } else { colors.danger };
            let when = entry.end.clone().unwrap_or_default();
            let trimmed = entry.output.trim().to_string();
            entries = entries.child(
              h_flex()
                .gap(px(8.))
                .items_start()
                .child(div().w(px(60.)).text_xs().text_color(line_color).child(format!("exit {exit}")))
                .child(div().w(px(160.)).text_xs().text_color(colors.muted_foreground).child(when))
                .child(
                  div()
                    .flex_1()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(colors.foreground)
                    .child(trimmed),
                ),
            );
          }
          col = col.child(div().mt(px(4.)).child(entries));
        }
      }

      // Mounts section.
      if !ex.mounts.is_empty() {
        col = col.child(
          div()
            .mt(px(8.))
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Mounts"),
        );
        for m in &ex.mounts {
          let rw_label = if m.rw { "rw" } else { "ro" };
          let title = match m.kind.as_str() {
            "volume" => format!(
              "volume {} → {}",
              m.name.clone().unwrap_or_else(|| m.source.clone()),
              m.destination
            ),
            "bind" => format!("bind {} → {}", m.source, m.destination),
            other => format!("{other} {} → {}", m.source, m.destination),
          };
          col = col.child(
            h_flex()
              .w_full()
              .py(px(8.))
              .gap(px(8.))
              .border_b_1()
              .border_color(colors.border)
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .font_family("monospace")
                  .text_color(colors.foreground)
                  .child(title),
              )
              .child(div().w(px(40.)).text_xs().text_color(colors.muted_foreground).child(rw_label))
              .child(
                div()
                  .w(px(80.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(m.mode.clone()),
              ),
          );
        }
      }
    }

    col
  }

  fn render_stats_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.container_state.as_ref();
    let Some(state) = state else {
      return v_flex().w_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No stats."),
      );
    };

    let cpu = state
      .stats_latest
      .as_ref()
      .map_or(0.0, |s| s.cpu_percent);
    let mem_pct = state
      .stats_latest
      .as_ref()
      .map_or(0.0, |s| s.memory_percent);
    let mem_usage = state.stats_latest.as_ref().map_or(0, |s| s.memory_usage);
    let mem_limit = state.stats_latest.as_ref().map_or(0, |s| s.memory_limit);
    let net_rx = state.stats_latest.as_ref().map_or(0, |s| s.network_rx);
    let net_tx = state.stats_latest.as_ref().map_or(0, |s| s.network_tx);
    let blk_r = state.stats_latest.as_ref().map_or(0, |s| s.block_read);
    let blk_w = state.stats_latest.as_ref().map_or(0, |s| s.block_write);

    let card = |title: &'static str,
                chart_id: &'static str,
                unit: &'static str,
                main: String,
                sub: String,
                history: &[f64],
                color: gpui::Hsla|
     -> gpui::Div {
      let history_owned: Vec<f64> = history.to_vec();
      v_flex()
        .flex_1()
        .min_w(px(180.))
        .p(px(12.))
        .bg(colors.background)
        .rounded(px(8.))
        .border_1()
        .border_color(colors.border)
        .gap(px(4.))
        .child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(title.to_string()),
        )
        .child(
          div()
            .text_lg()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child(main),
        )
        .child(div().text_xs().text_color(colors.muted_foreground).child(sub))
        .child(
          div()
            .h(px(48.))
            .mt(px(6.))
            .child(Self::render_sparkline(chart_id, title, unit, &history_owned, color)),
        )
    };

    let cpu_color = colors.primary;
    let mem_color = colors.success;
    let net_color = colors.warning;
    let disk_color = colors.danger;

    let row1 = h_flex()
      .gap(px(12.))
      .child(card(
        "CPU",
        "stats-cpu",
        "%",
        format!("{cpu:.1}%"),
        String::new(),
        &state.stats_cpu,
        cpu_color,
      ))
      .child(card(
        "Memory",
        "stats-mem",
        "%",
        format!("{mem_pct:.1}%"),
        format!("{} / {}", format_bytes(mem_usage), format_bytes(mem_limit)),
        &state.stats_mem_pct,
        mem_color,
      ));

    let row2 = h_flex()
      .gap(px(12.))
      .mt(px(12.))
      .child(card(
        "Network",
        "stats-net",
        "B",
        format!("rx {} / tx {}", format_bytes(net_rx), format_bytes(net_tx)),
        String::new(),
        &state.stats_net,
        net_color,
      ))
      .child(card(
        "Disk I/O",
        "stats-disk",
        "B",
        format!("r {} / w {}", format_bytes(blk_r), format_bytes(blk_w)),
        String::new(),
        &state.stats_disk,
        disk_color,
      ));

    v_flex().w_full().p(px(16.)).gap(px(8.)).child(row1).child(row2)
  }

  fn render_sparkline(
    chart_id: &'static str,
    label: &'static str,
    unit: &'static str,
    history: &[f64],
    color: gpui::Hsla,
  ) -> impl IntoElement {
    let data: Vec<f64> = history.iter().rev().take(60).rev().copied().collect();
    let tooltip_text = sparkline_summary(label, &data, unit);
    div()
      .id(chart_id)
      .w_full()
      .h_full()
      .child(crate::ui::components::Sparkline::new(data, color))
      .tooltip(move |window, cx| {
        gpui_component::tooltip::Tooltip::new(tooltip_text.clone()).build(window, cx)
      })
  }

  fn render_logs_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.container_state.as_ref();
    let is_loading = state.is_some_and(|s| s.logs_loading);
    let follow_on = state.is_some_and(|s| s.logs_follow);
    let ts_on = state.is_some_and(|s| s.logs_timestamps);

    let toggle_follow = self.on_toggle_logs_follow.clone();
    let toggle_ts = self.on_toggle_logs_timestamps.clone();
    let refresh = self.on_refresh_logs.clone();
    let clear = self.on_clear_logs.clone();

    let toolbar = h_flex()
      .gap(px(8.))
      .px(px(8.))
      .py(px(6.))
      .border_b_1()
      .border_color(colors.border)
      .child(
        Button::new("logs-follow")
          .label(if follow_on { "Following" } else { "Paused" })
          .icon(if follow_on {
            Icon::new(AppIcon::Pause)
          } else {
            Icon::new(AppIcon::Play)
          })
          .small()
          .when_some(toggle_follow, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      )
      .child(
        Button::new("logs-timestamps")
          .label(if ts_on { "Timestamps: on" } else { "Timestamps: off" })
          .small()
          .ghost()
          .when_some(toggle_ts, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      )
      .child(
        Button::new("logs-refresh")
          .icon(Icon::new(AppIcon::Refresh))
          .small()
          .ghost()
          .when_some(refresh, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      )
      .child(
        Button::new("logs-clear")
          .icon(Icon::new(AppIcon::Trash))
          .small()
          .ghost()
          .when_some(clear, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      );

    let body: gpui::AnyElement = if is_loading && state.is_none_or(|s| s.logs.is_empty()) {
      v_flex()
        .size_full()
        .p(px(16.))
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Loading logs..."),
        )
        .into_any_element()
    } else if let Some(view) = self.logs_terminal.clone() {
      div()
        .id("container-logs-terminal")
        .size_full()
        .min_h_0()
        .child(view)
        .into_any_element()
    } else {
      div().size_full().into_any_element()
    };

    v_flex()
      .size_full()
      .child(toolbar)
      .child(div().flex_1().min_h_0().w_full().child(body))
  }

  fn render_processes_tab(&self, is_running: bool, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    // Container must be running to show processes
    if !is_running {
      return v_flex()
        .flex_1()
        .w_full()
        .p(px(16.))
        .items_center()
        .justify_center()
        .gap(px(16.))
        .child(
          Icon::new(IconName::Info)
            .size(px(48.))
            .text_color(colors.muted_foreground),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Container must be running to view processes"),
        )
        .into_any_element();
    }

    // If we have a process view, render it full size
    if let Some(process_view) = &self.process_view {
      return div()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(process_view.clone())
        .into_any_element();
    }

    // Fallback: show loading message
    v_flex()
      .flex_1()
      .w_full()
      .items_center()
      .justify_center()
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading processes..."),
      )
      .into_any_element()
  }

  fn render_terminal_tab(&self, is_running: bool, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    // Container must be running to connect terminal
    if !is_running {
      return v_flex()
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
            .child("Container must be running to connect terminal"),
        )
        .into_any_element();
    }

    // If we have a terminal view, render it full size
    if let Some(terminal) = &self.terminal_view {
      return div()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(terminal.clone())
        .into_any_element();
    }

    // Fallback: show message (shouldn't normally happen if tab change creates terminal)
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
          .child("Connecting to container..."),
      )
      .into_any_element()
  }

  fn render_inspect_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.container_state.as_ref();
    let is_loading = state.is_some_and(|s| s.inspect_loading);

    if is_loading {
      return v_flex()
        .size_full()
        .p(px(16.))
        .child(div().text_sm().text_color(colors.muted_foreground).child("Loading..."));
    }

    if let Some(ref editor) = self.inspect_editor {
      return div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true));
    }

    // Fallback to plain text
    let inspect_content = state.map_or_else(|| "{}".to_string(), |s| s.inspect.clone());
    div().size_full().child(
      div()
        .size_full()
        .overflow_y_scrollbar()
        .bg(colors.sidebar)
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(inspect_content),
    )
  }

  fn render_files_tab(&self, is_running: bool, window: &mut Window, cx: &App) -> gpui::AnyElement {
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
        )
        .into_any_element();
    }

    let state = self.container_state.as_ref();

    let explorer_state = FileExplorerState {
      current_path: state.map_or_else(|| "/".to_string(), |s| s.current_path.clone()),
      is_loading: state.is_some_and(|s| s.files_loading),
      error: state.and_then(|s| s.files_error.clone()),
      selected_file: state.and_then(|s| s.selected_file.clone()),
      file_content: state.map(|s| s.file_content.clone()).unwrap_or_default(),
      file_content_loading: state.is_some_and(|s| s.file_content_loading),
      file_content_error: state.and_then(|s| s.file_content_error.clone()),
    };

    let files = state.map(|s| s.files.clone()).unwrap_or_default();

    let mut explorer = FileExplorer::new()
      .files(files)
      .state(explorer_state)
      .config(FileExplorerConfig::default().empty_message("Directory is empty"))
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

    if let Some(ref cb) = self.on_open_in_editor {
      let cb = cb.clone();
      explorer = explorer.on_open_in_editor(move |data: &(String, bool), window, cx| {
        cb(data, window, cx);
      });
    }

    explorer.render(window, cx)
  }

  pub fn render(&self, window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(container) = &self.container else {
      return Self::render_empty(cx).into_any_element();
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

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(8.))
      .gap(px(12.))
      .items_center()
      .flex_shrink_0()
      .border_b_1()
      .border_color(colors.border)
      .child(
        TabBar::new("container-tabs")
          .flex_1()
          .children(ContainerDetailTab::ALL.iter().map(|tab| {
            let on_tab_change = on_tab_change.clone();
            let tab_variant = *tab;
            Tab::new()
              .label(tab.label().to_string())
              .selected(self.active_tab == *tab)
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_tab_change {
                  cb(&tab_variant, window, cx);
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
                .label("Start")
                .ghost()
                .compact()
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
                .label("Stop")
                .ghost()
                .compact()
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
              .label("Restart")
              .ghost()
              .compact()
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
              .label("Delete")
              .ghost()
              .compact()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_delete {
                  cb(&id, window, cx);
                }
              })
          }),
      );

    // Terminal, Logs, Processes, and Files tabs need full height without scroll
    let is_full_height_tab = matches!(
      self.active_tab,
      ContainerDetailTab::Logs
        | ContainerDetailTab::Processes
        | ContainerDetailTab::Terminal
        | ContainerDetailTab::Files
    );

    // Content based on active tab
    let mut result = div()
      .size_full()
      .overflow_hidden()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .child(toolbar);

    if is_full_height_tab {
      let content = match self.active_tab {
        ContainerDetailTab::Logs => self.render_logs_tab(cx).into_any_element(),
        ContainerDetailTab::Processes => self.render_processes_tab(is_running, cx),
        ContainerDetailTab::Terminal => self.render_terminal_tab(is_running, cx),
        ContainerDetailTab::Files => self.render_files_tab(is_running, window, cx),
        _ => self.render_info_tab(container, cx).into_any_element(),
      };
      result = result.child(
        div()
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .flex()
          .flex_col()
          .child(content),
      );
    } else {
      let content = match self.active_tab {
        ContainerDetailTab::Inspect => self.render_inspect_tab(cx),
        ContainerDetailTab::Stats => self.render_stats_tab(cx),
        _ => self.render_info_tab(container, cx),
      };
      result = result.child(
        div()
          .id("container-detail-scroll")
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      );
    }

    result.into_any_element()
  }
}
