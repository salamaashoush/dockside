// Allow precision loss for display formatting of resource statistics
#![allow(clippy::cast_precision_loss)]

use gpui::{Context, Entity, Hsla, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{Icon, h_flex, label::Label, scroll::ScrollableElement, theme::ActiveTheme, v_flex};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::assets::AppIcon;
use crate::docker::{AggregateStats, ContainerStats};
use crate::kubernetes::PodPhase;
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
  k8s_expanded: bool,
  machines_expanded: bool,
}

impl ActivityMonitorView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    // Get refresh interval from settings
    let refresh_interval = settings_state(cx).read(cx).settings.stats_refresh_interval;
    let docker_state_entity = docker_state(cx);

    // Re-render whenever the runtime signals it changed: container
    // ticks, Kubernetes pod/node/metrics refreshes, machine refreshes,
    // and context switches (services clear + refetch on switch — we
    // just react and re-render the new context's numbers).
    cx.subscribe(
      &docker_state_entity,
      |_this, _state, event: &crate::state::StateChanged, cx| {
        use crate::state::StateChanged::{
          ContainersUpdated, KubeContextSwitched, MachinesUpdated, NodeMetricsUpdated, NodesUpdated, PodMetricsUpdated,
          PodsUpdated,
        };
        if matches!(
          event,
          ContainersUpdated
            | PodsUpdated
            | NodesUpdated
            | NodeMetricsUpdated
            | PodMetricsUpdated
            | MachinesUpdated
            | KubeContextSwitched
        ) {
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

        let _ = this.update(cx, |_this, cx| {
          Self::refresh_stats(cx);
          Self::refresh_runtime(cx);
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
      k8s_expanded: true,
      machines_expanded: true,
    };

    Self::refresh_stats(cx);
    Self::refresh_runtime(cx);
    view
  }

  /// Trigger the generation-guarded refresh services for the non-Docker
  /// runtime data shown here. Each service drops stale responses on a
  /// context switch itself — we never touch the guard.
  fn refresh_runtime(cx: &mut Context<'_, Self>) {
    let settings = settings_state(cx).read(cx).settings.clone();
    if settings.colima_enabled {
      services::refresh_machines(cx);
    }
    if settings.kubernetes_enabled {
      services::refresh_pods(cx);
      services::refresh_nodes(cx);
      services::refresh_node_metrics(cx);
      services::refresh_pod_metrics(cx);
    }
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

    // Running / paused / exited counts, inlined on the group header
    // (the same place the Kubernetes group carries its summary).
    let (mut running, mut paused, mut exited) = (0_usize, 0_usize, 0_usize);
    for c in &self.docker_state.read(cx).containers {
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
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .pl(px(24.))
                            .gap(px(16.))
                            .items_center()
                            .child(Self::count_seg("running", running, colors.success, cx))
                            .child(Self::count_seg("paused", paused, colors.warning, cx))
                            .child(Self::count_seg("exited", exited, colors.muted_foreground, cx)),
                    ),
            )
            // Container rows (when expanded)
            .when(expanded, |el| {
                el.child(
                    h_flex()
                        .w_full()
                        .h(px(28.))
                        .px(px(16.))
                        .pl(px(56.))
                        .items_center()
                        .child(
                            h_flex()
                                .flex_1()
                                .items_center()
                                .child(
                                    div()
                                        .w(px(268.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(colors.muted_foreground)
                                        .child("Name"),
                                )
                                .child(
                                    div()
                                        .w(px(60.))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(colors.muted_foreground)
                                        .text_right()
                                        .child("CPU %"),
                                )
                                .child(div().w(px(40.)).ml(px(4.)))
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
                        ),
                )
                .children(
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
      .flex_shrink_0()
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
      .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tooltip_text.clone()).build(window, cx))
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

  /// Inline `<count> <label>` segment: colored number, muted label.
  /// Used to carry status counts on a group header.
  fn count_seg(label: &'static str, count: usize, color: Hsla, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    h_flex()
      .gap(px(6.))
      .items_center()
      .child(
        div()
          .min_w(px(20.))
          .text_sm()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(if count > 0 { color } else { colors.muted_foreground })
          .text_right()
          .child(count.to_string()),
      )
      .child(div().text_sm().text_color(colors.muted_foreground).child(label))
  }

  /// Collapsible group header styled exactly like the Containers
  /// group: chevron, icon, title, then a muted summary on the right.
  fn group_header(
    id: &'static str,
    icon: AppIcon,
    title: &'static str,
    summary: String,
    expanded: bool,
    cx: &Context<'_, Self>,
    toggle: impl Fn(&mut Self, &mut Context<'_, Self>) + 'static,
  ) -> impl IntoElement {
    let colors = &cx.theme().colors;
    h_flex()
      .id(id)
      .w_full()
      .h(px(36.))
      .px(px(16.))
      .items_center()
      .cursor_pointer()
      .hover(|el| el.bg(colors.list_hover))
      .on_click(cx.listener(move |this, _ev, _window, cx| {
        toggle(this, cx);
        cx.notify();
      }))
      .child(
        h_flex()
          .flex_1()
          .items_center()
          .gap(px(8.))
          .child(
            Icon::new(if expanded {
              AppIcon::ChevronDown
            } else {
              AppIcon::ChevronRight
            })
            .size(px(14.))
            .text_color(colors.muted_foreground),
          )
          .child(Icon::new(icon).size(px(16.)).text_color(colors.foreground))
          .child(
            div()
              .flex_1()
              .text_sm()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(colors.foreground)
              .child(title),
          )
          .child(div().text_sm().text_color(colors.muted_foreground).child(summary)),
      )
  }

  /// Kubernetes signal: pod count by phase + node Ready/NotReady (+
  /// metrics-server CPU/mem when present). Reads the global state the
  /// guarded refresh services populate; multi-context aware because
  /// those services clear + refetch on a context switch.
  fn render_k8s_group(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let expanded = self.k8s_expanded;
    let state = self.docker_state.read(cx);

    let running = state.pods.iter().filter(|p| p.phase == PodPhase::Running).count();
    let summary = format!(
      "{} running / {} pods · {} nodes",
      running,
      state.pods.len(),
      state.nodes.len()
    );

    let phase_color = |p: PodPhase| match p {
      PodPhase::Running => colors.success,
      PodPhase::Pending => colors.warning,
      PodPhase::Failed => colors.danger,
      PodPhase::Succeeded | PodPhase::Unknown => colors.muted_foreground,
    };

    // Pods, busiest first, as a CPU/memory table like the containers.
    let mut pods: Vec<_> = state.pods.iter().collect();
    pods.sort_by(|a, b| {
      let ca = state.pod_usage(&a.namespace, &a.name).map_or(0.0, |(c, _)| c);
      let cb = state.pod_usage(&b.namespace, &b.name).map_or(0.0, |(c, _)| c);
      cb.total_cmp(&ca)
    });
    let pod_rows: Vec<gpui::AnyElement> = pods
      .iter()
      .map(|p| {
        let usage = state.pod_usage(&p.namespace, &p.name);
        let cpu = usage.map_or_else(
          || "—".to_string(),
          |(m, _)| {
            if m >= 1000.0 {
              format!("{:.2} cores", m / 1000.0)
            } else {
              format!("{m:.0}m")
            }
          },
        );
        let mem = usage.map_or_else(|| "—".to_string(), |(_, b)| bytesize::ByteSize(b).to_string());
        h_flex()
          .id(gpui::SharedString::from(format!("pod-{}-{}", p.namespace, p.name)))
          .w_full()
          .h(px(32.))
          .px(px(16.))
          .pl(px(56.))
          .items_center()
          .gap(px(8.))
          .hover(|el| el.bg(colors.list_hover))
          .child(
            div()
              .size(px(6.))
              .rounded_full()
              .flex_shrink_0()
              .bg(phase_color(p.phase)),
          )
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_sm()
              .text_color(colors.foreground)
              .text_ellipsis()
              .overflow_hidden()
              .whitespace_nowrap()
              .child(format!("{}/{}", p.namespace, p.name)),
          )
          .child(
            div()
              .flex_shrink_0()
              .w(px(96.))
              .text_sm()
              .text_color(phase_color(p.phase))
              .text_right()
              .child(p.phase.to_string()),
          )
          .child(
            div()
              .flex_shrink_0()
              .w(px(96.))
              .text_sm()
              .text_color(colors.secondary_foreground)
              .text_right()
              .child(cpu),
          )
          .child(
            div()
              .flex_shrink_0()
              .w(px(110.))
              .text_sm()
              .text_color(colors.secondary_foreground)
              .text_right()
              .child(mem),
          )
          .into_any_element()
      })
      .collect();

    // Nodes: Ready/NotReady + node-level usage from metrics-server.
    let node_rows: Vec<gpui::AnyElement> =
      state
        .nodes
        .iter()
        .map(|n| {
          let ok = n.status == "Ready";
          let (cpu, mem) = state.node_usage(&n.name).cloned().unwrap_or_default();
          h_flex()
            .w_full()
            .h(px(32.))
            .px(px(16.))
            .pl(px(56.))
            .items_center()
            .gap(px(8.))
            .child(div().size(px(6.)).rounded_full().flex_shrink_0().bg(if ok {
              colors.success
            } else {
              colors.danger
            }))
            .child(
              div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(colors.foreground)
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(n.name.clone()),
            )
            .child(
              div()
                .flex_shrink_0()
                .w(px(96.))
                .text_sm()
                .text_color(if ok { colors.success } else { colors.danger })
                .text_right()
                .child(n.status.clone()),
            )
            .child(
              div()
                .flex_shrink_0()
                .w(px(96.))
                .text_sm()
                .text_color(colors.secondary_foreground)
                .text_right()
                .child(fmt_cpu_raw(&cpu)),
            )
            .child(
              div()
                .flex_shrink_0()
                .w(px(110.))
                .text_sm()
                .text_color(colors.secondary_foreground)
                .text_right()
                .child(fmt_mem_raw(&mem)),
            )
            .into_any_element()
        })
        .collect();

    // Column header aligned to the pod/node rows (same paddings and
    // fixed column widths). Doubles as the section label.
    let col = |w: f32, label: &'static str| {
      div()
        .flex_shrink_0()
        .w(px(w))
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(colors.muted_foreground)
        .text_right()
        .child(label)
    };
    let cols_header = |title: &'static str| {
      h_flex()
        .w_full()
        .px(px(16.))
        .pl(px(56.))
        .py(px(6.))
        .gap(px(8.))
        .items_center()
        .child(div().size(px(6.)).flex_shrink_0())
        .child(
          div()
            .flex_1()
            .min_w_0()
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.muted_foreground)
            .child(title),
        )
        .child(col(96., "Status"))
        .child(col(96., "CPU"))
        .child(col(110., "Memory"))
    };

    v_flex()
      .w_full()
      .child(Self::group_header(
        "k8s-group",
        AppIcon::Pod,
        "Kubernetes",
        summary,
        expanded,
        cx,
        |this, _cx| this.k8s_expanded = !this.k8s_expanded,
      ))
      .when(expanded, |el| {
        el.child(
          v_flex()
            .w_full()
            .child(cols_header("Pods"))
            .children(pod_rows)
            .child(cols_header("Nodes"))
            .children(node_rows),
        )
      })
  }

  /// Colima / machine signal: VM status + sizing for every machine.
  fn render_machines_group(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let expanded = self.machines_expanded;
    let state = self.docker_state.read(cx);
    let machines = state.machines.clone();
    let running = machines.iter().filter(|m| m.is_running()).count();
    let summary = format!("{} running · {} total", running, machines.len());

    let rows: Vec<gpui::AnyElement> = machines
      .iter()
      .map(|m| {
        let up = m.is_running();
        h_flex()
          .w_full()
          .h(px(30.))
          .px(px(16.))
          .pl(px(56.))
          .items_center()
          .gap(px(8.))
          .child(div().size(px(6.)).rounded_full().flex_shrink_0().bg(if up {
            colors.success
          } else {
            colors.muted_foreground
          }))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_sm()
              .text_color(colors.foreground)
              .text_ellipsis()
              .overflow_hidden()
              .whitespace_nowrap()
              .child(m.name().to_string()),
          )
          .child(
            div()
              .flex_shrink_0()
              .text_xs()
              .text_color(if up { colors.success } else { colors.muted_foreground })
              .child(m.status_display()),
          )
          .child(
            div()
              .flex_shrink_0()
              .w(px(160.))
              .text_xs()
              .text_color(colors.muted_foreground)
              .text_right()
              .child(format!("{} CPU · {}", m.cpus(), m.display_memory())),
          )
          .into_any_element()
      })
      .collect();

    v_flex()
      .w_full()
      .child(Self::group_header(
        "machines-group",
        AppIcon::Machine,
        "Machines",
        summary,
        expanded,
        cx,
        |this, _cx| this.machines_expanded = !this.machines_expanded,
      ))
      .when(expanded, |el| el.child(v_flex().w_full().children(rows)))
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
  format!("{label} last {last:.1}{unit} (min {min:.1}{unit} / max {max:.1}{unit} / avg {avg:.1}{unit})")
}

/// Format a raw Kubernetes CPU quantity (`150m`, `2297905584n`, `1`)
/// into a compact human string.
fn fmt_cpu_raw(s: &str) -> String {
  let s = s.trim();
  if s.is_empty() {
    return "—".to_string();
  }
  let millicores = if let Some(v) = s.strip_suffix('n') {
    v.parse::<f64>().unwrap_or(0.0) / 1_000_000.0
  } else if let Some(v) = s.strip_suffix('u') {
    v.parse::<f64>().unwrap_or(0.0) / 1_000.0
  } else if let Some(v) = s.strip_suffix('m') {
    v.parse::<f64>().unwrap_or(0.0)
  } else {
    s.parse::<f64>().unwrap_or(0.0) * 1_000.0
  };
  if millicores >= 1000.0 {
    format!("{:.2} cores", millicores / 1000.0)
  } else {
    format!("{millicores:.0}m")
  }
}

/// Format a raw Kubernetes memory quantity (`7124364Ki`, `512Mi`) using
/// `format_bytes`.
fn fmt_mem_raw(s: &str) -> String {
  let s = s.trim();
  if s.is_empty() {
    return "—".to_string();
  }
  let pairs: [(&str, u64); 10] = [
    ("Ki", 1 << 10),
    ("Mi", 1 << 20),
    ("Gi", 1 << 30),
    ("Ti", 1 << 40),
    ("Pi", 1 << 50),
    ("k", 1_000),
    ("M", 1_000_000),
    ("G", 1_000_000_000),
    ("T", 1_000_000_000_000),
    ("P", 1_000_000_000_000_000),
  ];
  for (suffix, mult) in pairs {
    if let Some(v) = s.strip_suffix(suffix) {
      return format_bytes(v.trim().parse::<u64>().unwrap_or(0).saturating_mul(mult));
    }
  }
  format_bytes(s.parse::<u64>().unwrap_or(0))
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
    let settings = settings_state(cx).read(cx).settings.clone();
    let show_k8s = settings.kubernetes_enabled;
    let show_machines = settings.colima_enabled;

    div()
            .size_full()
            .bg(colors.background)
            .flex()
            .flex_col()
            // Title bar
            .child(
                h_flex()
                    .w_full()
                    .h(px(56.))
                    .flex_shrink_0()
                    .px(px(20.))
                    .gap(px(16.))
                    .items_center()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        Label::new("Activity Monitor")
                            .text_color(colors.foreground)
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    ),
            )
            // Content area
            .child(
                div()
                    .id("activity-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .when(has_containers, |el| {
                        el.child(self.render_container_group(cx))
                    })
                    .when(!has_containers, |el| {
                        el.child(Self::render_empty(cx))
                    })
                    .when(show_k8s, |el| el.child(self.render_k8s_group(cx)))
                    .when(show_machines, |el| el.child(self.render_machines_group(cx))),
            )
            // Summary section at bottom
            .child(self.render_summary_section(cx))
  }
}
