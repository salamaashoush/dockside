// Allow precision loss for display formatting of resource statistics
#![allow(clippy::cast_precision_loss)]

use gpui::{Context, Entity, Hsla, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{Icon, h_flex, label::Label, scroll::ScrollableElement, theme::ActiveTheme, v_flex};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::assets::AppIcon;
use crate::docker::{AggregateStats, ContainerStats};
use crate::services;
use crate::state::{DockerState, docker_state, settings_state};

const PER_ROW_SAMPLES: usize = 60;

/// Activity monitor showing container resource usage
pub struct ActivityMonitorView {
  docker_state: Entity<DockerState>,
  stats: AggregateStats,
  expanded: bool,
  is_loading: bool,
  // History for mini charts (last 60 samples)
  cpu_history: Vec<f64>,
  memory_history: Vec<u64>,
  network_history: Vec<u64>,
  disk_history: Vec<u64>,
  /// Per-container CPU% ring buffer keyed by container id. Trimmed to
  /// `PER_ROW_SAMPLES` so the inline row sparkline matches the bottom
  /// summary cards.
  cpu_history_per: HashMap<String, VecDeque<f64>>,
}

impl ActivityMonitorView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    // Get refresh interval from settings
    let refresh_interval = settings_state(cx).read(cx).settings.stats_refresh_interval;
    let docker_state_entity = docker_state(cx);

    // Re-render the breakdown badges whenever the global container
    // list ticks so paused / exited counts stay live without us
    // having to poll docker for state ourselves.
    cx.subscribe(
      &docker_state_entity,
      |_this, _state, event: &crate::state::StateChanged, cx| {
        if matches!(event, crate::state::StateChanged::ContainersUpdated) {
          cx.notify();
        }
      },
    )
    .detach();

    // Start periodic refresh
    cx.spawn(async move |this, cx| {
      loop {
        // Wait for configured refresh interval
        Timer::after(Duration::from_secs(refresh_interval)).await;

        // Refresh stats
        let _ = this.update(cx, |_this, cx| {
          Self::refresh_stats(cx);
        });
      }
    })
    .detach();

    // Initial refresh
    let view = Self {
      docker_state: docker_state(cx),
      stats: AggregateStats::default(),
      expanded: true,
      is_loading: true,
      cpu_history: Vec::with_capacity(60),
      memory_history: Vec::with_capacity(60),
      network_history: Vec::with_capacity(60),
      disk_history: Vec::with_capacity(60),
      cpu_history_per: HashMap::new(),
    };

    Self::refresh_stats(cx);
    view
  }

  fn refresh_stats(cx: &mut Context<'_, Self>) {
    let tokio_handle = services::Tokio::runtime_handle();
    let client = services::docker_client();

    cx.spawn(async move |this, cx| {
      let stats = cx
        .background_executor()
        .spawn(async move {
          tokio_handle.block_on(async {
            let guard = client.read().await;
            match guard.as_ref() {
              Some(docker) => docker.get_all_container_stats().await.ok(),
              None => None,
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.is_loading = false;
        if let Some(stats) = stats {
          // Update history
          this.cpu_history.push(stats.total_cpu_percent);
          this.memory_history.push(stats.total_memory);
          this
            .network_history
            .push(stats.total_network_rx + stats.total_network_tx);
          this.disk_history.push(stats.total_block_read + stats.total_block_write);

          // Keep only last 60 samples
          if this.cpu_history.len() > 60 {
            this.cpu_history.remove(0);
          }
          if this.memory_history.len() > 60 {
            this.memory_history.remove(0);
          }
          if this.network_history.len() > 60 {
            this.network_history.remove(0);
          }
          if this.disk_history.len() > 60 {
            this.disk_history.remove(0);
          }

          // Per-container CPU history: append the latest sample for
          // every container reporting stats this tick, then drop any
          // entries no longer present so containers that disappear
          // don't keep stale ring buffers around forever.
          let mut alive: std::collections::HashSet<&str> = std::collections::HashSet::new();
          for s in &stats.container_stats {
            alive.insert(s.id.as_str());
            let entry = this
              .cpu_history_per
              .entry(s.id.clone())
              .or_insert_with(|| VecDeque::with_capacity(PER_ROW_SAMPLES));
            entry.push_back(s.cpu_percent);
            while entry.len() > PER_ROW_SAMPLES {
              entry.pop_front();
            }
          }
          this.cpu_history_per.retain(|id, _| alive.contains(id.as_str()));

          this.stats = stats;
        }
        cx.notify();
      });
    })
    .detach();
  }

  fn render_header(cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;

    h_flex()
      .w_full()
      .h(px(40.))
      .px(px(16.))
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .bg(colors.sidebar)
      .child(
        h_flex()
          .flex_1()
          .items_center()
          .child(
            div()
              .w(px(300.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .child("Name"),
          )
          .child(
            div()
              .w(px(100.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("CPU %"),
          )
          .child(
            div()
              .w(px(100.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("Memory"),
          )
          .child(
            div()
              .w(px(100.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("Network"),
          )
          .child(
            div()
              .w(px(100.))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.muted_foreground)
              .text_right()
              .child("Disk"),
          ),
      )
  }

  fn render_container_group(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let expanded = self.expanded;

    // Calculate totals for the group
    let total_cpu: f64 = self.stats.container_stats.iter().map(|s| s.cpu_percent).sum();
    let total_memory: u64 = self.stats.container_stats.iter().map(|s| s.memory_usage).sum();
    let total_network: u64 = self
      .stats
      .container_stats
      .iter()
      .map(|s| s.network_rx + s.network_tx)
      .sum();
    let total_disk: u64 = self
      .stats
      .container_stats
      .iter()
      .map(|s| s.block_read + s.block_write)
      .sum();

    v_flex()
            .w_full()
            // Group header
            .child(
                h_flex()
                    .id("containers-group")
                    .w_full()
                    .h(px(36.))
                    .px(px(16.))
                    .items_center()
                    .cursor_pointer()
                    .hover(|el| el.bg(colors.list_hover))
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.expanded = !this.expanded;
                        cx.notify();
                    }))
                    .child(
                        h_flex()
                            .flex_1()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Icon::new(if expanded { AppIcon::ChevronDown } else { AppIcon::ChevronRight })
                                    .size(px(14.))
                                    .text_color(colors.muted_foreground),
                            )
                            .child(
                                Icon::new(AppIcon::Container)
                                    .size(px(16.))
                                    .text_color(colors.foreground),
                            )
                            .child(
                                div()
                                    .w(px(260.))
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child("Containers"),
                            )
                            .child(
                                div()
                                    .w(px(100.))
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .text_right()
                                    .child(format!("{total_cpu:.1}")),
                            )
                            .child(
                                div()
                                    .w(px(100.))
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .text_right()
                                    .child(format_bytes(total_memory)),
                            )
                            .child(
                                div()
                                    .w(px(100.))
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .text_right()
                                    .child(format!("{}/s", format_bytes(total_network))),
                            )
                            .child(
                                div()
                                    .w(px(100.))
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .text_right()
                                    .child(format!("{}/s", format_bytes(total_disk))),
                            ),
                    ),
            )
            // Container rows (when expanded)
            .when(expanded, |el| {
                el.children(
                    self.stats
                        .container_stats
                        .iter()
                        .map(|stats| {
                            let series: Vec<f64> = self
                                .cpu_history_per
                                .get(&stats.id)
                                .map(|q| q.iter().copied().collect::<Vec<_>>())
                                .unwrap_or_default();
                            Self::render_container_row(stats, series, cx)
                        }),
                )
            })
  }

  fn render_container_row(stats: &ContainerStats, cpu_series: Vec<f64>, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let name = if stats.name.is_empty() {
      stats.id.chars().take(12).collect::<String>()
    } else {
      stats.name.clone()
    };

    h_flex()
            .id(gpui::SharedString::from(format!("container-row-{}", stats.id)))
            .w_full()
            .h(px(32.))
            .px(px(16.))
            .pl(px(56.)) // Indent for child rows
            .items_center()
            .hover(|el| el.bg(colors.list_hover))
            .child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .child(
                        div()
                            .w(px(268.))
                            .text_sm()
                            .text_color(colors.foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(name),
                    )
                    .child(
                        div()
                            .w(px(60.))
                            .text_sm()
                            .text_color(colors.secondary_foreground)
                            .text_right()
                            .child(format!("{:.1}", stats.cpu_percent)),
                    )
                    .child({
                        let tooltip_text = sparkline_summary("CPU", &cpu_series, "%");
                        div()
                            .id(gpui::SharedString::from(format!("cpu-spark-{}", stats.id)))
                            .w(px(40.))
                            .h(px(20.))
                            .ml(px(4.))
                            .child(crate::ui::components::Sparkline::new(cpu_series, colors.link).max(100.0))
                            .tooltip(move |window, cx| {
                                gpui_component::tooltip::Tooltip::new(tooltip_text.clone()).build(window, cx)
                            })
                    })
                    .child(
                        div()
                            .w(px(100.))
                            .text_sm()
                            .text_color(colors.secondary_foreground)
                            .text_right()
                            .child(stats.display_memory()),
                    )
                    .child(
                        div()
                            .w(px(100.))
                            .text_sm()
                            .text_color(colors.secondary_foreground)
                            .text_right()
                            .child(stats.display_network_rx()),
                    )
                    .child(
                        div()
                            .w(px(100.))
                            .text_sm()
                            .text_color(colors.secondary_foreground)
                            .text_right()
                            .child(stats.display_block_read()),
                    ),
            )
  }

  fn render_summary_section(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;

    h_flex()
      .w_full()
      .h(px(120.))
      .border_t_1()
      .border_color(colors.border)
      .bg(colors.sidebar)
      .child(
        // Total CPU
        Self::render_summary_card(
          "Total CPU:",
          format!("{:.1}%", self.stats.total_cpu_percent),
          "summary-cpu",
          &self.cpu_history,
          "%",
          colors.link,
          cx,
        ),
      )
      .child(
        // Memory
        Self::render_summary_card(
          "Memory:",
          self.stats.display_total_memory(),
          "summary-mem",
          &self.memory_history.iter().map(|&v| v as f64).collect::<Vec<_>>(),
          "B",
          colors.primary,
          cx,
        ),
      )
      .child(
        // Network
        Self::render_summary_card(
          "Network:",
          self.stats.display_total_network(),
          "summary-net",
          &self.network_history.iter().map(|&v| v as f64).collect::<Vec<_>>(),
          "B/s",
          colors.accent,
          cx,
        ),
      )
      .child(
        // Disk
        Self::render_summary_card(
          "Disk:",
          self.stats.display_total_disk(),
          "summary-disk",
          &self.disk_history.iter().map(|&v| v as f64).collect::<Vec<_>>(),
          "B/s",
          colors.success,
          cx,
        ),
      )
  }

  fn render_summary_card(
    label: &'static str,
    value: String,
    chart_id: &'static str,
    history: &[f64],
    unit: &'static str,
    color: Hsla,
    cx: &Context<'_, Self>,
  ) -> impl IntoElement {
    let colors = &cx.theme().colors;

    v_flex()
      .flex_1()
      .h_full()
      .p(px(12.))
      .border_r_1()
      .border_color(colors.border)
      .child(
        h_flex()
          .w_full()
          .justify_between()
          .child(
            Label::new(label)
              .text_color(colors.foreground)
              .font_weight(gpui::FontWeight::MEDIUM),
          )
          .child(Label::new(value).text_color(colors.muted_foreground)),
      )
      .child(Self::render_mini_chart(chart_id, label, history, unit, color))
  }

  fn render_mini_chart(
    chart_id: &'static str,
    label: &'static str,
    history: &[f64],
    unit: &'static str,
    color: Hsla,
  ) -> impl IntoElement {
    let data: Vec<f64> = history.iter().rev().take(60).rev().copied().collect();
    let tooltip_text = sparkline_summary(label, &data, unit);
    div()
      .id(chart_id)
      .flex_1()
      .mt(px(8.))
      .w_full()
      .h_full()
      .child(crate::ui::components::Sparkline::new(data, color))
      .tooltip(move |window, cx| {
        gpui_component::tooltip::Tooltip::new(tooltip_text.clone()).build(window, cx)
      })
  }

  /// Status breakdown badges: running / paused / exited container
  /// counts pulled from the global container list (the docker daemon
  /// is the source of truth for state — `AggregateStats` only knows
  /// about the running set).
  fn render_status_breakdown(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let containers = &self.docker_state.read(cx).containers;
    let mut running = 0_usize;
    let mut paused = 0_usize;
    let mut exited = 0_usize;
    for c in containers {
      if c.state.is_running() {
        running += 1;
      } else if c.state.is_paused() {
        paused += 1;
      } else if matches!(
        c.state,
        crate::docker::ContainerState::Exited
          | crate::docker::ContainerState::Dead
          | crate::docker::ContainerState::Removing
      ) {
        exited += 1;
      }
    }
    let badge = move |label: &'static str, count: usize, color: Hsla| -> gpui::Div {
      v_flex()
        .px(px(10.))
        .py(px(4.))
        .gap(px(0.))
        .rounded(px(6.))
        .bg(if count > 0 { color.opacity(0.15) } else { colors.muted })
        .child(
          div()
            .text_xs()
            .text_color(if count > 0 { color } else { colors.muted_foreground })
            .child(label),
        )
        .child(
          div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child(count.to_string()),
        )
    };
    h_flex()
      .gap(px(8.))
      .items_center()
      .child(badge("RUNNING", running, colors.success))
      .child(badge("PAUSED", paused, colors.warning))
      .child(badge("EXITED", exited, colors.muted_foreground))
  }

  fn render_empty(cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;

    div().flex_1().flex().items_center().justify_center().child(
      v_flex()
        .items_center()
        .gap(px(16.))
        .child(
          Icon::new(AppIcon::Container)
            .size(px(48.))
            .text_color(colors.muted_foreground),
        )
        .child(div().text_color(colors.muted_foreground).child("No running containers")),
    )
  }
}

/// One-line tooltip body for a sparkline: "CPU last 12.3% (min 0 / max
/// 80 / avg 14)". `unit` is appended after each numeric value (e.g.
/// "%"). `series` is allowed to be empty — returns "<label> no data".
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
  const KB: u64 = 1024;
  const MB: u64 = KB * 1024;
  const GB: u64 = MB * 1024;

  if bytes >= GB {
    format!("{:.1} GB", bytes as f64 / GB as f64)
  } else if bytes >= MB {
    format!("{:.1} MB", bytes as f64 / MB as f64)
  } else if bytes >= KB {
    format!("{:.0} KB", bytes as f64 / KB as f64)
  } else {
    format!("{bytes} B")
  }
}

impl Render for ActivityMonitorView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let has_containers = !self.stats.container_stats.is_empty();

    div()
            .size_full()
            .bg(colors.background)
            .flex()
            .flex_col()
            // Title bar
            .child(
                h_flex()
                    .w_full()
                    .h(px(52.))
                    .px(px(16.))
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        Label::new("Activity Monitor")
                            .text_color(colors.foreground)
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .child(self.render_status_breakdown(cx)),
            )
            // Table header
            .child(Self::render_header(cx))
            // Content area
            .child(
                div()
                    .id("activity-scroll")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .when(has_containers, |el| {
                        el.child(self.render_container_group(cx))
                    })
                    .when(!has_containers, |el| {
                        el.child(Self::render_empty(cx))
                    }),
            )
            // Summary section at bottom
            .child(self.render_summary_section(cx))
  }
}
