//! Top-level Dashboard view: resource counts, system stats, recent
//! activity, and pinned favorites.

use gpui::{Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  ActiveTheme, Icon, IconName, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  scroll::ScrollableElement,
  v_flex,
};

use crate::assets::AppIcon;
use crate::docker::ContainerState;
use crate::services;
use crate::state::{
  CurrentView, DockerState, FavoriteRef, SettingsChanged, SettingsState, StateChanged, docker_state, settings_state,
};

pub struct DashboardView {
  docker_state: Entity<DockerState>,
  settings_state: Entity<SettingsState>,
}

impl DashboardView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);
    let settings_state = settings_state(cx);

    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::ContainersUpdated
          | StateChanged::ImagesUpdated
          | StateChanged::VolumesUpdated
          | StateChanged::NetworksUpdated
          | StateChanged::PodsUpdated
          | StateChanged::DeploymentsUpdated
          | StateChanged::ServicesUpdated
          | StateChanged::MachinesUpdated
          | StateChanged::EventsUpdated
      ) {
        cx.notify();
      }
    })
    .detach();

    cx.subscribe(&settings_state, |_this, _state, _event: &SettingsChanged, cx| {
      cx.notify();
    })
    .detach();

    Self {
      docker_state,
      settings_state,
    }
  }

  fn section_header(title: &str, cx: &Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    div().px(px(16.)).pt(px(24.)).pb(px(10.)).child(
      div()
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.foreground)
        .child(title.to_string()),
    )
  }

  fn count_tile(
    label: &str,
    value: String,
    sub: Option<String>,
    icon: Icon,
    target: CurrentView,
    cx: &mut Context<'_, Self>,
  ) -> gpui::Stateful<gpui::Div> {
    let colors = cx.theme().colors;
    let sub_text = sub.unwrap_or_else(|| "—".to_string());
    let id = format!("tile-{label}");
    div()
      .id(SharedString::from(id))
      .w(px(220.))
      .h(px(108.))
      .p(px(16.))
      .rounded(px(10.))
      .border_1()
      .border_color(colors.border)
      .bg(colors.background)
      .hover(|s| s.border_color(colors.primary).bg(colors.sidebar))
      .cursor_pointer()
      .on_click(cx.listener(move |_this, _ev, _w, cx| {
        services::set_view(target, cx);
      }))
      .child(
        v_flex()
          .size_full()
          .justify_between()
          .child(
            h_flex()
              .w_full()
              .items_center()
              .justify_between()
              .child(
                div()
                  .text_xs()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_color(colors.muted_foreground)
                  .child(label.to_string()),
              )
              .child(
                div()
                  .size(px(28.))
                  .rounded(px(6.))
                  .bg(colors.sidebar)
                  .flex()
                  .items_center()
                  .justify_center()
                  .child(icon.size(px(14.)).text_color(colors.muted_foreground)),
              ),
          )
          .child(
            v_flex()
              .gap(px(2.))
              .child(
                div()
                  .text_2xl()
                  .font_weight(gpui::FontWeight::SEMIBOLD)
                  .text_color(colors.foreground)
                  .line_height(px(28.))
                  .child(value),
              )
              .child(
                div()
                  .h(px(16.))
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(sub_text),
              ),
          ),
      )
  }

  fn render_counts(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let state = self.docker_state.read(cx);
    let containers_total = state.containers.len();
    let containers_running = state.containers.iter().filter(|c| c.state.is_running()).count();
    let containers_stopped = state
      .containers
      .iter()
      .filter(|c| matches!(c.state, ContainerState::Exited | ContainerState::Dead))
      .count();
    let images_count = state.images.len();
    let images_size: i64 = state.images.iter().map(|i| i.size).sum();
    let volumes_count = state.volumes.len();
    let networks_count = state.networks.len();

    let pods_count = state.pods.len();
    let deployments_count = state.deployments.len();
    let services_count = state.services.len();
    let k8s_available = state.k8s_available;

    let mut grid = h_flex().px(px(16.)).gap(px(12.)).flex_wrap();
    grid = grid.child(Self::count_tile(
      "Containers",
      containers_total.to_string(),
      Some(format!("{containers_running} running · {containers_stopped} stopped")),
      Icon::new(AppIcon::Container),
      CurrentView::Containers,
      cx,
    ));
    grid = grid.child(Self::count_tile(
      "Images",
      images_count.to_string(),
      Some(format_bytes(images_size)),
      Icon::new(AppIcon::Image),
      CurrentView::Images,
      cx,
    ));
    grid = grid.child(Self::count_tile(
      "Volumes",
      volumes_count.to_string(),
      None,
      Icon::new(AppIcon::Volume),
      CurrentView::Volumes,
      cx,
    ));
    grid = grid.child(Self::count_tile(
      "Networks",
      networks_count.to_string(),
      None,
      Icon::new(AppIcon::Network),
      CurrentView::Networks,
      cx,
    ));

    if k8s_available {
      grid = grid.child(Self::count_tile(
        "Pods",
        pods_count.to_string(),
        None,
        Icon::new(AppIcon::Pod),
        CurrentView::Pods,
        cx,
      ));
      grid = grid.child(Self::count_tile(
        "Deployments",
        deployments_count.to_string(),
        None,
        Icon::new(AppIcon::Deployment),
        CurrentView::Deployments,
        cx,
      ));
      grid = grid.child(Self::count_tile(
        "Services",
        services_count.to_string(),
        None,
        Icon::new(AppIcon::Service),
        CurrentView::Services,
        cx,
      ));
    }

    div().w_full().child(grid)
  }

  fn render_system(&self, cx: &Context<'_, Self>) -> gpui::Div {
    let theme = cx.theme();
    let colors = theme.colors;
    let state = self.docker_state.read(cx);
    let host = state.host().cloned();
    let machines_total = state.machines.len();
    let active_machine = state.active_machine.clone();
    let k8s_available = state.k8s_available;
    let nodes_count = state.nodes.len();

    let docker_connected = host.is_some();
    let (docker_value, docker_sub) = host.map_or_else(
      || ("Disconnected".to_string(), String::new()),
      |h| (h.docker_version.clone(), format!("{} {}", h.os, h.arch)),
    );
    let (machine_value, machine_sub) = if let Some(id) = active_machine {
      (id.name().to_string(), format!("{machines_total} total"))
    } else {
      (format!("{machines_total}"), "machines".to_string())
    };
    let (k8s_value, k8s_sub) = if k8s_available {
      (format!("{nodes_count}"), "node(s) ready".to_string())
    } else {
      ("Off".to_string(), "Kubernetes disabled".to_string())
    };

    h_flex()
      .px(px(16.))
      .gap(px(12.))
      .w_full()
      .flex_wrap()
      .child(stat_tile(
        if docker_connected {
          colors.success
        } else {
          colors.muted_foreground
        },
        "Docker",
        docker_value,
        docker_sub,
        Icon::new(AppIcon::Container),
        cx,
      ))
      .child(stat_tile(
        if k8s_available {
          colors.success
        } else {
          colors.muted_foreground
        },
        "Kubernetes",
        k8s_value,
        k8s_sub,
        Icon::new(AppIcon::Pod),
        cx,
      ))
      .child(stat_tile(
        colors.success,
        "Machines",
        machine_value,
        machine_sub,
        Icon::new(AppIcon::Machine),
        cx,
      ))
  }

  fn render_favorites(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let favorites = self.settings_state.read(cx).settings.favorites.clone();

    if favorites.is_empty() {
      return div().px(px(16.)).child(
        div()
          .p(px(16.))
          .rounded(px(8.))
          .border_1()
          .border_color(colors.border)
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No favorites yet. Pin items from their detail or row menus."),
      );
    }

    let mut grid = h_flex().px(px(16.)).gap(px(12.)).flex_wrap();
    for fav in favorites {
      let icon = match &fav {
        FavoriteRef::Container { .. } => Icon::new(AppIcon::Container),
        FavoriteRef::Image { .. } => Icon::new(AppIcon::Image),
        FavoriteRef::Volume { .. } | FavoriteRef::Pvc { .. } => Icon::new(AppIcon::Volume),
        FavoriteRef::Network { .. } => Icon::new(AppIcon::Network),
        FavoriteRef::Pod { .. } | FavoriteRef::Job { .. } | FavoriteRef::CronJob { .. } => Icon::new(AppIcon::Pod),
        FavoriteRef::Deployment { .. } | FavoriteRef::StatefulSet { .. } | FavoriteRef::DaemonSet { .. } => {
          Icon::new(AppIcon::Deployment)
        }
        FavoriteRef::Service { .. } | FavoriteRef::Ingress { .. } => Icon::new(AppIcon::Service),
        FavoriteRef::Secret { .. } | FavoriteRef::ConfigMap { .. } => Icon::new(AppIcon::Settings),
        FavoriteRef::Machine { .. } => Icon::new(AppIcon::Machine),
      };
      let label = fav.label().to_string();
      let kind = fav.kind_label().to_string();
      let id_suffix = format!("{kind}-{label}");
      let fav_for_open = fav.clone();
      let fav_for_unpin = fav.clone();

      grid = grid.child(
        div()
          .id(SharedString::from(format!("fav-{id_suffix}")))
          .w(px(220.))
          .h(px(108.))
          .p(px(16.))
          .rounded(px(10.))
          .border_1()
          .border_color(colors.border)
          .bg(colors.background)
          .hover(|s| s.border_color(colors.primary).bg(colors.sidebar))
          .cursor_pointer()
          .on_click(cx.listener(move |_this, _ev, _w, cx| {
            services::open_favorite(&fav_for_open, cx);
          }))
          .child(
            v_flex()
              .size_full()
              .justify_between()
              .child(
                h_flex()
                  .w_full()
                  .items_center()
                  .justify_between()
                  .child(
                    div()
                      .text_xs()
                      .font_weight(gpui::FontWeight::MEDIUM)
                      .text_color(colors.muted_foreground)
                      .child(kind),
                  )
                  .child(
                    h_flex()
                      .gap(px(4.))
                      .items_center()
                      .child(
                        div()
                          .size(px(28.))
                          .rounded(px(6.))
                          .bg(colors.sidebar)
                          .flex()
                          .items_center()
                          .justify_center()
                          .child(icon.size(px(14.)).text_color(colors.muted_foreground)),
                      )
                      .child(
                        Button::new(SharedString::from(format!("unpin-{id_suffix}")))
                          .icon(IconName::Close)
                          .ghost()
                          .xsmall()
                          .on_click(cx.listener(move |_this, _ev, _w, cx| {
                            services::toggle_favorite(fav_for_unpin.clone(), cx);
                          })),
                      ),
                  ),
              )
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_color(colors.foreground)
                  .line_height(px(20.))
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(label),
              ),
          ),
      );
    }
    grid
  }

  fn render_activity(&self, cx: &Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);

    let panel = |title: &str, rows: Vec<gpui::Div>, cx: &Context<'_, Self>| -> gpui::Div {
      let colors = cx.theme().colors;
      v_flex()
        .flex_1()
        .min_w(px(360.))
        .rounded(px(10.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.background)
        .child(
          h_flex()
            .w_full()
            .px(px(14.))
            .py(px(10.))
            .border_b_1()
            .border_color(colors.border)
            .child(
              div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.foreground)
                .child(title.to_string()),
            ),
        )
        .child(v_flex().w_full().px(px(14.)).py(px(8.)).gap(px(4.)).children(rows))
    };

    let events_rows: Vec<gpui::Div> = if state.k8s_available && !state.events.is_empty() {
      state
        .events
        .iter()
        .take(6)
        .map(|e| {
          let type_color = if e.event_type == "Warning" {
            colors.warning
          } else {
            colors.muted_foreground
          };
          h_flex()
            .gap(px(8.))
            .py(px(4.))
            .items_center()
            .child(
              div()
                .w(px(60.))
                .text_xs()
                .text_color(type_color)
                .child(e.event_type.clone()),
            )
            .child(
              div()
                .w(px(110.))
                .text_xs()
                .text_color(colors.foreground)
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(e.reason.clone()),
            )
            .child(
              div()
                .flex_1()
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(e.message.clone()),
            )
            .child(
              div()
                .w(px(40.))
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_right()
                .child(e.age.clone()),
            )
        })
        .collect()
    } else {
      vec![
        div()
          .py(px(4.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("No recent events."),
      ]
    };

    let container_rows: Vec<gpui::Div> = if state.containers.is_empty() {
      vec![
        div()
          .py(px(4.))
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("No containers."),
      ]
    } else {
      state
        .containers
        .iter()
        .take(6)
        .map(|c| {
          let status_color = if c.state.is_running() {
            colors.success
          } else {
            colors.muted_foreground
          };
          h_flex()
            .gap(px(8.))
            .py(px(4.))
            .items_center()
            .child(div().size(px(8.)).rounded_full().bg(status_color))
            .child(
              div()
                .w(px(160.))
                .text_xs()
                .text_color(colors.foreground)
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(c.name.clone()),
            )
            .child(
              div()
                .flex_1()
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(c.image.clone()),
            )
            .child(
              div()
                .w(px(70.))
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_right()
                .child(c.state.to_string()),
            )
        })
        .collect()
    };

    h_flex()
      .px(px(16.))
      .gap(px(12.))
      .w_full()
      .flex_wrap()
      .items_start()
      .child(panel("Containers", container_rows, cx))
      .when(state.k8s_available, |el| {
        el.child(panel("Recent Events", events_rows, cx))
      })
  }
}

impl Render for DashboardView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;

    let toolbar = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .child(Label::new("Dashboard"))
      .child(
        Button::new("dashboard-refresh")
          .icon(Icon::new(AppIcon::Refresh))
          .ghost()
          .compact()
          .on_click(cx.listener(|_this, _ev, _w, cx| {
            services::refresh_containers(cx);
            services::refresh_images(cx);
            services::refresh_volumes(cx);
            services::refresh_networks(cx);
            if docker_state(cx).read(cx).k8s_available {
              services::refresh_pods(cx);
              services::refresh_deployments(cx);
              services::refresh_services(cx);
              services::refresh_events(cx);
            }
          })),
      );

    let body = v_flex()
      .w_full()
      .pb(px(24.))
      .child(Self::section_header("System", cx))
      .child(self.render_system(cx))
      .child(Self::section_header("Resources", cx))
      .child(self.render_counts(cx))
      .child(Self::section_header("Favorites", cx))
      .child(self.render_favorites(cx))
      .child(Self::section_header("Recent Activity", cx))
      .child(self.render_activity(cx));

    v_flex().size_full().child(toolbar).child(
      div()
        .id("dashboard-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .child(body),
    )
  }
}

fn stat_tile(
  dot_color: gpui::Hsla,
  label: &str,
  value: String,
  sub: String,
  icon: Icon,
  cx: &Context<'_, DashboardView>,
) -> gpui::Div {
  let colors = cx.theme().colors;
  div()
    .w(px(220.))
    .h(px(108.))
    .p(px(16.))
    .rounded(px(10.))
    .border_1()
    .border_color(colors.border)
    .bg(colors.background)
    .child(
      v_flex()
        .size_full()
        .justify_between()
        .child(
          h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(
              h_flex()
                .gap(px(6.))
                .items_center()
                .child(div().size(px(8.)).rounded_full().bg(dot_color))
                .child(
                  div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.muted_foreground)
                    .child(label.to_string()),
                ),
            )
            .child(
              div()
                .size(px(28.))
                .rounded(px(6.))
                .bg(colors.sidebar)
                .flex()
                .items_center()
                .justify_center()
                .child(icon.size(px(14.)).text_color(colors.muted_foreground)),
            ),
        )
        .child(
          v_flex()
            .gap(px(2.))
            .child(
              div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.foreground)
                .line_height(px(22.))
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(value),
            )
            .child(
              div()
                .h(px(16.))
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(sub),
            ),
        ),
    )
}

fn format_bytes(b: i64) -> String {
  #[allow(clippy::cast_precision_loss)]
  let f = b as f64;
  if f < 1024.0 {
    format!("{b} B")
  } else if f < 1024.0 * 1024.0 {
    format!("{:.1} KiB", f / 1024.0)
  } else if f < 1024.0 * 1024.0 * 1024.0 {
    format!("{:.1} MiB", f / (1024.0 * 1024.0))
  } else {
    format!("{:.1} GiB", f / (1024.0 * 1024.0 * 1024.0))
  }
}
