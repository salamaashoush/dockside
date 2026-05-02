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
    div().px(px(16.)).pt(px(20.)).pb(px(8.)).child(
      Label::new(title.to_string())
        .text_sm()
        .text_color(colors.muted_foreground),
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
    let sub_text = sub.unwrap_or_default();
    let id = format!("tile-{label}");
    div()
      .id(SharedString::from(id))
      .min_w(px(180.))
      .flex_1()
      .p(px(14.))
      .rounded(px(8.))
      .border_1()
      .border_color(colors.border)
      .bg(colors.background)
      .hover(|s| s.bg(colors.sidebar))
      .cursor_pointer()
      .on_click(cx.listener(move |_this, _ev, _w, cx| {
        services::set_view(target, cx);
      }))
      .child(
        h_flex()
          .gap(px(10.))
          .items_center()
          .child(
            div()
              .size(px(32.))
              .rounded(px(6.))
              .bg(colors.sidebar)
              .flex()
              .items_center()
              .justify_center()
              .child(icon.size(px(16.)).text_color(colors.muted_foreground)),
          )
          .child(
            v_flex()
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(label.to_string()),
              )
              .child(
                div()
                  .text_xl()
                  .font_weight(gpui::FontWeight::SEMIBOLD)
                  .text_color(colors.foreground)
                  .child(value),
              )
              .when(!sub_text.is_empty(), |el| {
                el.child(div().text_xs().text_color(colors.muted_foreground).child(sub_text))
              }),
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

    let mut row1 = h_flex().gap(px(12.)).w_full();
    row1 = row1.child(Self::count_tile(
      "Containers",
      containers_total.to_string(),
      Some(format!("{containers_running} running, {containers_stopped} stopped")),
      Icon::new(AppIcon::Container),
      CurrentView::Containers,
      cx,
    ));
    row1 = row1.child(Self::count_tile(
      "Images",
      images_count.to_string(),
      Some(format_bytes(images_size)),
      Icon::new(AppIcon::Image),
      CurrentView::Images,
      cx,
    ));
    row1 = row1.child(Self::count_tile(
      "Volumes",
      volumes_count.to_string(),
      None,
      Icon::new(AppIcon::Volume),
      CurrentView::Volumes,
      cx,
    ));
    row1 = row1.child(Self::count_tile(
      "Networks",
      networks_count.to_string(),
      None,
      Icon::new(AppIcon::Network),
      CurrentView::Networks,
      cx,
    ));

    let mut row2 = h_flex().gap(px(12.)).w_full();
    if k8s_available {
      row2 = row2.child(Self::count_tile(
        "Pods",
        pods_count.to_string(),
        None,
        Icon::new(AppIcon::Pod),
        CurrentView::Pods,
        cx,
      ));
      row2 = row2.child(Self::count_tile(
        "Deployments",
        deployments_count.to_string(),
        None,
        Icon::new(AppIcon::Deployment),
        CurrentView::Deployments,
        cx,
      ));
      row2 = row2.child(Self::count_tile(
        "Services",
        services_count.to_string(),
        None,
        Icon::new(AppIcon::Service),
        CurrentView::Services,
        cx,
      ));
    }

    let mut col = v_flex().px(px(16.)).gap(px(12.)).w_full().child(row1);
    if k8s_available {
      col = col.child(row2);
    }
    col
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

    let docker_line = host.map_or_else(
      || "Disconnected".to_string(),
      |h| format!("{} • {} {}", h.docker_version, h.os, h.arch),
    );
    let machine_line = if let Some(id) = active_machine {
      format!("{machines_total} total • active: {}", id.name())
    } else {
      format!("{machines_total} total")
    };
    let k8s_line = if k8s_available {
      format!("{nodes_count} node(s)")
    } else {
      "Unavailable".to_string()
    };

    h_flex()
      .px(px(16.))
      .gap(px(20.))
      .items_center()
      .w_full()
      .flex_wrap()
      .child(stat_tile(colors.success, "Docker", docker_line, cx))
      .child(stat_tile(
        if k8s_available {
          colors.success
        } else {
          colors.muted_foreground
        },
        "Kubernetes",
        k8s_line,
        cx,
      ))
      .child(stat_tile(colors.success, "Machines", machine_line, cx))
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
        FavoriteRef::Volume { .. } => Icon::new(AppIcon::Volume),
        FavoriteRef::Network { .. } => Icon::new(AppIcon::Network),
        FavoriteRef::Pod { .. } => Icon::new(AppIcon::Pod),
        FavoriteRef::Deployment { .. } | FavoriteRef::StatefulSet { .. } => Icon::new(AppIcon::Deployment),
        FavoriteRef::Service { .. } => Icon::new(AppIcon::Service),
        FavoriteRef::Machine { .. } => Icon::new(AppIcon::Machine),
      };
      let label = fav.label().to_string();
      let kind = fav.kind_label().to_string();
      let id_suffix = format!("{kind}-{label}");
      let fav_for_open = fav.clone();
      let fav_for_unpin = fav.clone();

      grid = grid.child(
        h_flex()
          .id(SharedString::from(format!("fav-{id_suffix}")))
          .min_w(px(220.))
          .gap(px(8.))
          .p(px(10.))
          .rounded(px(8.))
          .border_1()
          .border_color(colors.border)
          .bg(colors.background)
          .hover(|s| s.bg(colors.sidebar))
          .cursor_pointer()
          .on_click(cx.listener(move |_this, _ev, _w, cx| {
            services::open_favorite(&fav_for_open, cx);
          }))
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
            v_flex()
              .flex_1()
              .min_w_0()
              .child(
                div()
                  .text_sm()
                  .text_color(colors.foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(label),
              )
              .child(div().text_xs().text_color(colors.muted_foreground).child(kind)),
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
      );
    }
    grid
  }

  fn render_activity(&self, cx: &Context<'_, Self>) -> gpui::Div {
    let colors = cx.theme().colors;
    let state = self.docker_state.read(cx);

    // Rough recent feed: take last 5 events (Warning first), last 5
    // containers by created, last 5 pods by namespace+name. Cheap, all
    // pulled from existing state Vecs.
    let mut col = v_flex().px(px(16.)).gap(px(8.));

    if state.k8s_available && !state.events.is_empty() {
      col = col.child(
        Label::new("Recent Events")
          .text_xs()
          .text_color(colors.muted_foreground),
      );
      for e in state.events.iter().take(8) {
        let type_color = if e.event_type == "Warning" {
          colors.warning
        } else {
          colors.muted_foreground
        };
        col = col.child(
          h_flex()
            .gap(px(8.))
            .py(px(4.))
            .border_b_1()
            .border_color(colors.border)
            .child(
              div()
                .w(px(60.))
                .text_xs()
                .text_color(type_color)
                .child(e.event_type.clone()),
            )
            .child(
              div()
                .w(px(120.))
                .text_xs()
                .text_color(colors.foreground)
                .child(e.reason.clone()),
            )
            .child(
              div()
                .flex_1()
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_ellipsis()
                .overflow_hidden()
                .child(e.message.clone()),
            )
            .child(
              div()
                .w(px(60.))
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(e.age.clone()),
            ),
        );
      }
    }

    if !state.containers.is_empty() {
      col = col.child(
        div()
          .pt(px(8.))
          .child(Label::new("Containers").text_xs().text_color(colors.muted_foreground)),
      );
      for c in state.containers.iter().take(5) {
        let status_color = if c.state.is_running() {
          colors.success
        } else {
          colors.muted_foreground
        };
        col = col.child(
          h_flex()
            .gap(px(8.))
            .py(px(4.))
            .border_b_1()
            .border_color(colors.border)
            .child(div().size(px(8.)).rounded_full().bg(status_color))
            .child(
              div()
                .w(px(220.))
                .text_xs()
                .text_color(colors.foreground)
                .text_ellipsis()
                .overflow_hidden()
                .child(c.name.clone()),
            )
            .child(
              div()
                .flex_1()
                .text_xs()
                .text_color(colors.muted_foreground)
                .text_ellipsis()
                .overflow_hidden()
                .child(c.image.clone()),
            )
            .child(
              div()
                .w(px(80.))
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(c.state.to_string()),
            ),
        );
      }
    }

    col
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

fn stat_tile(dot_color: gpui::Hsla, label: &str, value: String, cx: &Context<'_, DashboardView>) -> gpui::Div {
  let colors = cx.theme().colors;
  h_flex()
    .gap(px(10.))
    .items_center()
    .py(px(12.))
    .child(div().size(px(8.)).rounded_full().bg(dot_color))
    .child(
      v_flex()
        .child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(label.to_string()),
        )
        .child(div().text_sm().text_color(colors.foreground).child(value)),
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
