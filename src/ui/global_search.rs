//! Global Search Component
//!
//! A search overlay that lets users quickly find any resource across all views.
//! Triggered by Cmd+F or /.
//!
//! Features:
//! - Automatically refreshes data when opened to ensure up-to-date results
//! - Subscribes to state changes for live updates while open
//! - Shows loading indicator while fetching data

use gpui::{
  App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, MouseButton, Render, SharedString, Styled,
  Window, anchored, div, point, prelude::*, px,
};
use gpui_component::{
  Icon, IconName,
  input::{Input, InputState},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;

use crate::state::{CurrentView, DockerState, Selection, StateChanged, docker_state};
use crate::ui::components::spinning_loader_circle;

// Actions for keyboard navigation within the search
gpui::actions!(global_search, [Cancel, SelectUp, SelectDown, Confirm]);

const CONTEXT: &str = "GlobalSearch";

pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    KeyBinding::new("up", SelectUp, Some(CONTEXT)),
    KeyBinding::new("down", SelectDown, Some(CONTEXT)),
    KeyBinding::new("enter", Confirm, Some(CONTEXT)),
  ]);
}

/// Type of search result
#[derive(Clone, Debug, PartialEq)]
pub enum SearchResultType {
  Container,
  Image,
  Volume,
  Network,
  Pod,
  Deployment,
  Service,
  Machine,
}

impl SearchResultType {
  fn icon(&self) -> AppIcon {
    match self {
      SearchResultType::Container => AppIcon::Container,
      SearchResultType::Image => AppIcon::Image,
      SearchResultType::Volume => AppIcon::Volume,
      SearchResultType::Network => AppIcon::Network,
      SearchResultType::Pod => AppIcon::Pod,
      SearchResultType::Deployment => AppIcon::Deployment,
      SearchResultType::Service => AppIcon::Service,
      SearchResultType::Machine => AppIcon::Machine,
    }
  }

  fn label(&self) -> &'static str {
    match self {
      SearchResultType::Container => "Container",
      SearchResultType::Image => "Image",
      SearchResultType::Volume => "Volume",
      SearchResultType::Network => "Network",
      SearchResultType::Pod => "Pod",
      SearchResultType::Deployment => "Deployment",
      SearchResultType::Service => "Service",
      SearchResultType::Machine => "Machine",
    }
  }

  fn view(&self) -> CurrentView {
    match self {
      SearchResultType::Container => CurrentView::Containers,
      SearchResultType::Image => CurrentView::Images,
      SearchResultType::Volume => CurrentView::Volumes,
      SearchResultType::Network => CurrentView::Networks,
      SearchResultType::Pod => CurrentView::Pods,
      SearchResultType::Deployment => CurrentView::Deployments,
      SearchResultType::Service => CurrentView::Services,
      SearchResultType::Machine => CurrentView::Machines,
    }
  }
}

/// A search result item
#[derive(Clone)]
pub struct SearchResult {
  pub result_type: SearchResultType,
  pub name: String,
  pub subtitle: String,
  pub selection: Selection,
}

/// Event emitted when a search result is selected
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum GlobalSearchEvent {
  Close,
  Navigate { view: CurrentView, selection: Selection },
}

/// The global search view
pub struct GlobalSearch {
  docker_state: Entity<DockerState>,
  query: String,
  input_state: Entity<InputState>,
  focus_handle: FocusHandle,
  selected_index: usize,
  results: Vec<SearchResult>,
  is_loading: bool,
}

impl GlobalSearch {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();
    let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search containers, images, pods..."));
    let docker_state = docker_state(cx);

    // Focus the input immediately
    input_state.update(cx, |input, cx| {
      input.focus(window, cx);
    });

    // Subscribe to state changes to update results when data refreshes
    cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
      // Update results when any resource type updates
      match event {
        StateChanged::ContainersUpdated
        | StateChanged::ImagesUpdated
        | StateChanged::VolumesUpdated
        | StateChanged::NetworksUpdated
        | StateChanged::PodsUpdated
        | StateChanged::DeploymentsUpdated
        | StateChanged::ServicesUpdated
        | StateChanged::MachinesUpdated => {
          this.is_loading = false;
          this.results = this.search_resources(&this.query, cx);
          cx.notify();
        }
        StateChanged::Loading => {
          this.is_loading = state.read(cx).is_loading;
          cx.notify();
        }
        _ => {}
      }
    })
    .detach();

    // Check if we need to refresh data
    let needs_refresh = {
      let state = docker_state.read(cx);
      state.containers.is_empty() && state.images.is_empty() && state.volumes.is_empty() && state.networks.is_empty()
    };

    let is_loading = if needs_refresh {
      // Trigger refresh of all resources
      Self::refresh_all_resources(cx);
      true
    } else {
      docker_state.read(cx).is_loading
    };

    let mut search = Self {
      docker_state,
      query: String::new(),
      input_state,
      focus_handle,
      selected_index: 0,
      results: Vec::new(),
      is_loading,
    };

    // Initial results (may be empty if loading)
    search.results = search.search_resources("", cx);
    search
  }

  /// Refresh all resource types to ensure search has current data
  fn refresh_all_resources(cx: &mut Context<'_, Self>) {
    crate::services::refresh_containers(cx);
    crate::services::refresh_images(cx);
    crate::services::refresh_volumes(cx);
    crate::services::refresh_networks(cx);
    crate::services::refresh_machines(cx);
    // K8s resources - only if available
    crate::services::refresh_pods(cx);
    crate::services::refresh_deployments(cx);
    crate::services::refresh_services(cx);
  }

  fn search_resources(&self, query: &str, cx: &Context<'_, Self>) -> Vec<SearchResult> {
    let state = self.docker_state.read(cx);
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    // Search containers
    for container in &state.containers {
      let name = container.name.trim_start_matches('/').to_string();
      if query.is_empty()
        || name.to_lowercase().contains(&query_lower)
        || container.image.to_lowercase().contains(&query_lower)
      {
        results.push(SearchResult {
          result_type: SearchResultType::Container,
          name: name.clone(),
          subtitle: format!("{} - {}", container.image, container.status),
          selection: Selection::Container(container.clone()),
        });
      }
    }

    // Search images
    for image in &state.images {
      let name = image.repo_tags.first().map_or(image.id.as_str(), String::as_str);
      if query.is_empty() || name.to_lowercase().contains(&query_lower) {
        results.push(SearchResult {
          result_type: SearchResultType::Image,
          name: name.to_string(),
          subtitle: format_size(image.size),
          selection: Selection::Image(image.clone()),
        });
      }
    }

    // Search volumes
    for volume in &state.volumes {
      if query.is_empty() || volume.name.to_lowercase().contains(&query_lower) {
        results.push(SearchResult {
          result_type: SearchResultType::Volume,
          name: volume.name.clone(),
          subtitle: volume.driver.clone(),
          selection: Selection::Volume(volume.name.clone()),
        });
      }
    }

    // Search networks
    for network in &state.networks {
      if query.is_empty() || network.name.to_lowercase().contains(&query_lower) {
        results.push(SearchResult {
          result_type: SearchResultType::Network,
          name: network.name.clone(),
          subtitle: network.driver.clone(),
          selection: Selection::Network(network.id.clone()),
        });
      }
    }

    // Search pods
    for pod in &state.pods {
      if query.is_empty()
        || pod.name.to_lowercase().contains(&query_lower)
        || pod.namespace.to_lowercase().contains(&query_lower)
      {
        results.push(SearchResult {
          result_type: SearchResultType::Pod,
          name: pod.name.clone(),
          subtitle: format!("{} - {:?}", pod.namespace, pod.phase),
          selection: Selection::Pod {
            name: pod.name.clone(),
            namespace: pod.namespace.clone(),
          },
        });
      }
    }

    // Search deployments
    for deployment in &state.deployments {
      if query.is_empty()
        || deployment.name.to_lowercase().contains(&query_lower)
        || deployment.namespace.to_lowercase().contains(&query_lower)
      {
        results.push(SearchResult {
          result_type: SearchResultType::Deployment,
          name: deployment.name.clone(),
          subtitle: format!(
            "{} - {}/{} ready",
            deployment.namespace, deployment.ready_replicas, deployment.replicas
          ),
          selection: Selection::Deployment {
            name: deployment.name.clone(),
            namespace: deployment.namespace.clone(),
          },
        });
      }
    }

    // Search services
    for service in &state.services {
      if query.is_empty()
        || service.name.to_lowercase().contains(&query_lower)
        || service.namespace.to_lowercase().contains(&query_lower)
      {
        results.push(SearchResult {
          result_type: SearchResultType::Service,
          name: service.name.clone(),
          subtitle: format!("{} - {}", service.namespace, service.service_type),
          selection: Selection::Service {
            name: service.name.clone(),
            namespace: service.namespace.clone(),
          },
        });
      }
    }

    // Search machines
    for machine in &state.colima_vms {
      if query.is_empty() || machine.name.to_lowercase().contains(&query_lower) {
        #[allow(clippy::cast_possible_wrap)]
        let memory_size = format_size(machine.memory as i64);
        results.push(SearchResult {
          result_type: SearchResultType::Machine,
          name: machine.name.clone(),
          subtitle: format!("{:?} - {} CPU, {memory_size} memory", machine.status, machine.cpus),
          selection: Selection::Machine(machine.name.clone()),
        });
      }
    }

    // Sort by relevance if there's a query
    if !query.is_empty() {
      results.sort_by(|a, b| {
        let a_starts = a.name.to_lowercase().starts_with(&query_lower);
        let b_starts = b.name.to_lowercase().starts_with(&query_lower);

        if a_starts && !b_starts {
          std::cmp::Ordering::Less
        } else if !a_starts && b_starts {
          std::cmp::Ordering::Greater
        } else {
          a.name.cmp(&b.name)
        }
      });
    }

    // Limit results for performance
    results.truncate(50);
    results
  }

  fn on_query_changed(&mut self, cx: &mut Context<'_, Self>) {
    self.query = self.input_state.read(cx).text().to_string();
    self.results = self.search_resources(&self.query, cx);
    self.selected_index = 0;
    cx.notify();
  }

  fn select_up(&mut self, _: &SelectUp, _window: &mut Window, cx: &mut Context<'_, Self>) {
    cx.stop_propagation();
    if !self.results.is_empty() {
      self.selected_index = if self.selected_index == 0 {
        self.results.len() - 1
      } else {
        self.selected_index - 1
      };
      cx.notify();
    }
  }

  fn select_down(&mut self, _: &SelectDown, _window: &mut Window, cx: &mut Context<'_, Self>) {
    cx.stop_propagation();
    if !self.results.is_empty() {
      self.selected_index = (self.selected_index + 1) % self.results.len();
      cx.notify();
    }
  }

  fn confirm(&mut self, _: &Confirm, _window: &mut Window, cx: &mut Context<'_, Self>) {
    cx.stop_propagation();
    self.execute_selected(cx);
  }

  fn cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<'_, Self>) {
    let _ = &self; // Required by action handler signature
    cx.stop_propagation();
    cx.emit(GlobalSearchEvent::Close);
  }

  fn execute_selected(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(result) = self.results.get(self.selected_index) {
      cx.emit(GlobalSearchEvent::Navigate {
        view: result.result_type.view(),
        selection: result.selection.clone(),
      });
      cx.emit(GlobalSearchEvent::Close);
    }
  }

  fn on_item_click(&mut self, ix: usize, cx: &mut Context<'_, Self>) {
    self.selected_index = ix;
    self.execute_selected(cx);
  }
}

impl EventEmitter<GlobalSearchEvent> for GlobalSearch {}

impl Focusable for GlobalSearch {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for GlobalSearch {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let selected_idx = self.selected_index;
    let results = self.results.clone();
    let view_size = window.viewport_size();
    let is_loading = self.is_loading;

    // Check for input changes
    let current_text = self.input_state.read(cx).text().to_string();
    if current_text != self.query {
      self.on_query_changed(cx);
    }

    // Use anchored to position at top-left, then manually center
    anchored().position(point(px(0.), px(0.))).child(
      div()
          .id("global-search-overlay")
          .key_context(CONTEXT)
          .track_focus(&self.focus_handle)
          .w(view_size.width)
          .h(view_size.height)
          .bg(gpui::hsla(0.0, 0.0, 0.0, 0.7))
          .flex()
          .justify_center()
          .pt(px(100.))
          .occlude()
          // Action handlers
          .on_action(cx.listener(Self::cancel))
          .on_action(cx.listener(Self::select_up))
          .on_action(cx.listener(Self::select_down))
          .on_action(cx.listener(Self::confirm))
          // Close on click outside
          .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _, cx| {
            cx.stop_propagation();
            cx.emit(GlobalSearchEvent::Close);
          }))
          .child(
            v_flex()
              .w(px(600.))
              .max_h(px(500.))
              .bg(colors.background)
              .border_1()
              .border_color(colors.border)
              .rounded(px(12.))
              .overflow_hidden()
              .occlude()
              // Stop click events from closing when clicking inside
              .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
              })
              // Search input
              .child(
                div()
                  .p(px(16.))
                  .border_b_1()
                  .border_color(colors.border)
                  .flex()
                  .items_center()
                  .gap(px(12.))
                  .child(
                    Icon::new(IconName::Search)
                      .size(px(20.))
                      .text_color(colors.muted_foreground),
                  )
                  .child(
                    div()
                      .flex_1()
                      .child(
                        Input::new(&self.input_state)
                          .cleanable(true)
                          .appearance(false),
                      ),
                  )
                  // Loading indicator (spinning)
                  .when(is_loading, |el| {
                    el.child(spinning_loader_circle(px(18.), colors.muted_foreground))
                  }),
              )
              // Results list
              .child(
                div()
                  .flex_1()
                  .overflow_hidden()
                  .py(px(8.))
                  // Loading state
                  .when(is_loading && results.is_empty(), |el| {
                    el.child(
                      div()
                        .px(px(16.))
                        .py(px(24.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .child(spinning_loader_circle(px(32.), colors.muted_foreground))
                        .child(
                          div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child("Loading resources..."),
                        ),
                    )
                  })
                  // Empty state (not loading)
                  .when(!is_loading && results.is_empty(), |el| {
                    el.child(
                      div()
                        .px(px(16.))
                        .py(px(24.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .child(
                          Icon::new(IconName::Search)
                            .size(px(32.))
                            .text_color(colors.muted_foreground),
                        )
                        .child(
                          div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child("No results found"),
                        )
                        .child(
                          div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child("Try a different search term"),
                        ),
                    )
                  })
                  .children(results.iter().enumerate().map(|(idx, result)| {
                    let is_selected = idx == selected_idx;
                    let icon = result.result_type.icon();
                    let type_label = result.result_type.label();
                    let name = SharedString::from(result.name.clone());
                    let subtitle = SharedString::from(result.subtitle.clone());
                    let item_id = SharedString::from(format!("search-result-{idx}"));
                    let group_name = format!("search-item-{idx}");

                    div()
                      .id(item_id)
                      .group(SharedString::from(group_name.clone()))
                      .px(px(12.))
                      .py(px(10.))
                      .mx(px(8.))
                      .rounded(px(8.))
                      .cursor_pointer()
                      .when(is_selected, |el| el.bg(colors.accent).text_color(colors.accent_foreground))
                      .when(!is_selected, |el| {
                        el.group_hover(SharedString::from(group_name), |el| {
                          el.bg(colors.accent).text_color(colors.accent_foreground)
                        })
                      })
                      .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered {
                          this.selected_index = idx;
                          cx.notify();
                        }
                      }))
                      .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                      })
                      .on_click(cx.listener(move |this, _, _, cx| {
                        this.on_item_click(idx, cx);
                      }))
                      .child(
                        div()
                          .flex()
                          .items_center()
                          .gap(px(12.))
                          .child(
                            div()
                              .w(px(36.))
                              .h(px(36.))
                              .flex()
                              .items_center()
                              .justify_center()
                              .rounded(px(8.))
                              .when(!is_selected, |el| el.bg(colors.secondary))
                              .when(is_selected, |el| el.bg(colors.accent_foreground.opacity(0.2)))
                              .child(
                                Icon::new(icon)
                                  .size(px(18.))
                                  .when(!is_selected, |el| el.text_color(colors.foreground)),
                              ),
                          )
                          .child(
                            div()
                              .flex_1()
                              .flex()
                              .flex_col()
                              .gap(px(2.))
                              .overflow_hidden()
                              .child(
                                div()
                                  .text_sm()
                                  .font_weight(gpui::FontWeight::MEDIUM)
                                  .truncate()
                                  .child(name),
                              )
                              .child(
                                div()
                                  .text_xs()
                                  .truncate()
                                  .when(!is_selected, |el| el.text_color(colors.muted_foreground))
                                  .child(subtitle),
                              ),
                          )
                          .child(
                            div()
                              .px(px(8.))
                              .py(px(4.))
                              .rounded(px(4.))
                              .when(!is_selected, |el| el.bg(colors.secondary))
                              .when(is_selected, |el| el.bg(colors.accent_foreground.opacity(0.2)))
                              .text_xs()
                              .when(!is_selected, |el| el.text_color(colors.muted_foreground))
                              .child(type_label),
                          ),
                      )
                  })),
              )
              // Footer hint
              .child(
                div()
                  .px(px(16.))
                  .py(px(10.))
                  .border_t_1()
                  .border_color(colors.border)
                  .flex()
                  .items_center()
                  .justify_between()
                  .child(
                    div()
                      .flex()
                      .items_center()
                      .gap(px(16.))
                      .child(
                        div()
                          .flex()
                          .items_center()
                          .gap(px(4.))
                          .text_xs()
                          .text_color(colors.muted_foreground)
                          .child(
                            div()
                              .px(px(4.))
                              .py(px(2.))
                              .bg(colors.secondary)
                              .rounded(px(4.))
                              .child("Up/Down"),
                          )
                          .child("navigate"),
                      )
                      .child(
                        div()
                          .flex()
                          .items_center()
                          .gap(px(4.))
                          .text_xs()
                          .text_color(colors.muted_foreground)
                          .child(
                            div()
                              .px(px(4.))
                              .py(px(2.))
                              .bg(colors.secondary)
                              .rounded(px(4.))
                              .child("Enter"),
                          )
                          .child("select"),
                      )
                      .child(
                        div()
                          .flex()
                          .items_center()
                          .gap(px(4.))
                          .text_xs()
                          .text_color(colors.muted_foreground)
                          .child(
                            div()
                              .px(px(4.))
                              .py(px(2.))
                              .bg(colors.secondary)
                              .rounded(px(4.))
                              .child("Esc"),
                          )
                          .child("close"),
                      ),
                  )
                  .child(
                    div()
                      .text_xs()
                      .text_color(colors.muted_foreground)
                      .child(format!("{} results", self.results.len())),
                  ),
              ),
          ),
    )
  }
}

/// Format bytes into human-readable size
#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: i64) -> String {
  const KB: i64 = 1024;
  const MB: i64 = KB * 1024;
  const GB: i64 = MB * 1024;

  if bytes >= GB {
    format!("{:.1} GB", bytes as f64 / GB as f64)
  } else if bytes >= MB {
    format!("{:.1} MB", bytes as f64 / MB as f64)
  } else if bytes >= KB {
    format!("{:.1} KB", bytes as f64 / KB as f64)
  } else {
    format!("{bytes} B")
  }
}
