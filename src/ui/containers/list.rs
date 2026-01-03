use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, IndexPath, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  list::{List, ListDelegate, ListEvent, ListItem, ListState},
  menu::{DropdownMenu, PopupMenuItem},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::docker::ContainerInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};

/// Container list events emitted to parent
pub enum ContainerListEvent {
  Selected(Box<ContainerInfo>),
  NewContainer,
}

/// Delegate for the container list
pub struct ContainerListDelegate {
  docker_state: Entity<DockerState>,
  selected_index: Option<IndexPath>,
  search_query: String,
}

impl ContainerListDelegate {
  fn containers<'a>(&self, cx: &'a App) -> &'a Vec<ContainerInfo> {
    &self.docker_state.read(cx).containers
  }

  fn filtered_containers(&self, cx: &App) -> Vec<ContainerInfo> {
    let containers = self.containers(cx);
    if self.search_query.is_empty() {
      return containers.clone();
    }

    let query = self.search_query.to_lowercase();
    containers
      .iter()
      .filter(|c| {
        c.name.to_lowercase().contains(&query)
          || c.image.to_lowercase().contains(&query)
          || c.state.to_string().to_lowercase().contains(&query)
          || c.id.to_lowercase().contains(&query)
      })
      .cloned()
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
    self.selected_index = None;
  }
}

impl ListDelegate for ContainerListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered_containers(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let containers = self.filtered_containers(cx);
    let container = containers.get(ix.row)?;
    let colors = &cx.theme().colors;

    let is_selected = self.selected_index == Some(ix);
    let is_running = container.state.is_running();
    let container_id = container.id.clone();

    let icon_bg = if is_running {
      colors.primary
    } else {
      colors.muted_foreground
    };

    let subtitle = container.image.clone();
    let status_color = if is_running {
      colors.success
    } else {
      colors.muted_foreground
    };

    // Three-dot menu button
    let id = container_id.clone();
    let container_name = container.name.clone();
    let running = is_running;
    let paused = container.state.is_paused();
    let row = ix.row;

    let menu_button = Button::new(("menu", row))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _window, _cx| {
        let mut menu = menu;
        let id = id.clone();
        let name = container_name.clone();

        if running {
          // Running container actions
          menu = menu
            .item(PopupMenuItem::new("Stop").icon(Icon::new(AppIcon::Stop)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::stop_container(id.clone(), cx);
              }
            }))
            .item(
              PopupMenuItem::new("Restart")
                .icon(Icon::new(AppIcon::Restart))
                .on_click({
                  let id = id.clone();
                  move |_, _, cx| {
                    services::restart_container(id.clone(), cx);
                  }
                }),
            )
            .item(PopupMenuItem::new("Pause").icon(IconName::Minus).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::pause_container(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Kill").icon(IconName::Close).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::kill_container(id.clone(), cx);
              }
            }))
            .separator()
            .item(
              PopupMenuItem::new("Terminal")
                .icon(Icon::new(AppIcon::Terminal))
                .on_click({
                  let id = id.clone();
                  move |_, _, cx| {
                    services::open_container_terminal(id.clone(), cx);
                  }
                }),
            )
            .item(PopupMenuItem::new("Logs").icon(Icon::new(AppIcon::Logs)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_logs(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Inspect").icon(IconName::Info).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_inspect(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Files").icon(IconName::Folder).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_files(id.clone(), cx);
              }
            }));
        } else if paused {
          // Paused container actions
          menu = menu
            .item(PopupMenuItem::new("Resume").icon(Icon::new(AppIcon::Play)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::unpause_container(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Stop").icon(Icon::new(AppIcon::Stop)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::stop_container(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Kill").icon(IconName::Close).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::kill_container(id.clone(), cx);
              }
            }))
            .separator()
            .item(PopupMenuItem::new("Logs").icon(Icon::new(AppIcon::Logs)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_logs(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Inspect").icon(IconName::Info).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_inspect(id.clone(), cx);
              }
            }));
        } else {
          // Stopped container actions
          menu = menu
            .item(PopupMenuItem::new("Start").icon(Icon::new(AppIcon::Play)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::start_container(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Logs").icon(Icon::new(AppIcon::Logs)).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_logs(id.clone(), cx);
              }
            }))
            .item(PopupMenuItem::new("Inspect").icon(IconName::Info).on_click({
              let id = id.clone();
              move |_, _, cx| {
                services::open_container_inspect(id.clone(), cx);
              }
            }));
        }

        // Common actions for all states
        menu = menu
          .separator()
          .item(PopupMenuItem::new("Rename").icon(IconName::Redo).on_click({
            let id = id.clone();
            let name = name.clone();
            move |_, _, cx| {
              services::request_rename_container(id.clone(), name.clone(), cx);
            }
          }))
          .item(PopupMenuItem::new("Commit to Image").icon(IconName::Copy).on_click({
            let id = id.clone();
            let name = name.clone();
            move |_, _, cx| {
              services::request_commit_container(id.clone(), name.clone(), cx);
            }
          }))
          .item(PopupMenuItem::new("Export").icon(IconName::ExternalLink).on_click({
            let id = id.clone();
            let name = name.clone();
            move |_, _, cx| {
              services::request_export_container(id.clone(), name.clone(), cx);
            }
          }))
          .separator()
          .item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click({
            let id = id.clone();
            move |_, _, cx| {
              services::delete_container(id.clone(), cx);
            }
          }));

        menu
      });

    // Build item content with menu button INSIDE (not as suffix which gets hidden)
    let item_content = h_flex()
      .w_full()
      .items_center()
      .justify_between()
      .gap(px(8.))
      .child(
        h_flex()
          .flex_1()
          .min_w_0()
          .items_center()
          .gap(px(8.))
          .child(
            div()
              .size(px(24.))
              .rounded(px(4.))
              .bg(icon_bg)
              .flex()
              .items_center()
              .justify_center()
              .flex_shrink_0()
              .child(
                Icon::new(AppIcon::Container)
                  .size(px(14.))
                  .text_color(colors.background),
              ),
          )
          .child(
            v_flex()
              .flex_1()
              .min_w_0()
              .gap(px(2.))
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(container.name.clone()),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(status_color)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(subtitle),
              ),
          ),
      )
      .child(div().flex_shrink_0().child(menu_button));

    let item = ListItem::new(("container", ix.row))
      .py(px(4.))
      .rounded(px(6.))
      .overflow_hidden()
      .selected(is_selected)
      .on_click(cx.listener(move |this, _ev, _window, cx| {
        this.delegate_mut().selected_index = Some(ix);
        cx.notify();
      }))
      .child(item_content);

    Some(item)
  }

  fn set_selected_index(&mut self, ix: Option<IndexPath>, _window: &mut Window, cx: &mut Context<'_, ListState<Self>>) {
    self.selected_index = ix;
    cx.notify();
  }
}

/// Self-contained container list component
pub struct ContainerList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<ContainerListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl ContainerList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = ContainerListDelegate {
      docker_state: docker_state.clone(),
      selected_index: None,
      search_query: String::new(),
    };

    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    // Subscribe to list events
    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered_containers(cx);
        if let Some(container) = filtered.get(ix.row) {
          cx.emit(ContainerListEvent::Selected(Box::new(container.clone())));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    // Subscribe to docker state changes to refresh list
    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::ContainersUpdated) {
        this.list_state.update(cx, |_state, cx| {
          cx.notify();
        });
        cx.notify();
      }
    })
    .detach();

    Self {
      docker_state,
      list_state,
      search_input: None,
      search_visible: false,
      search_query: String::new(),
    }
  }

  fn ensure_search_input(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.search_input.is_none() {
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search containers..."));
      self.search_input = Some(input_state);
    }
  }

  fn sync_search_query(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(input) = &self.search_input {
      let current_text = input.read(cx).text().to_string();
      if current_text != self.search_query {
        current_text.clone_into(&mut self.search_query);
        self.list_state.update(cx, |state, cx| {
          state.delegate_mut().set_search_query(current_text);
          cx.notify();
        });
      }
    }
  }

  fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    self.search_visible = !self.search_visible;
    if self.search_visible {
      self.ensure_search_input(window, cx);
    } else {
      // Clear search when hiding
      self.search_query.clear();
      self.search_input = None;
      self.list_state.update(cx, |state, cx| {
        state.delegate_mut().set_search_query(String::new());
        cx.notify();
      });
    }
    cx.notify();
  }

  fn render_empty(cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .flex_1()
      .flex()
      .items_center()
      .justify_center()
      .gap(px(16.))
      .py(px(48.))
      .child(
        div()
          .size(px(64.))
          .rounded(px(12.))
          .bg(colors.sidebar)
          .flex()
          .items_center()
          .justify_center()
          .child(Icon::new(AppIcon::Container).text_color(colors.muted_foreground)),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child("No Containers"),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Run a container to get started"),
      )
  }

  fn render_no_results(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .flex_1()
      .flex()
      .items_center()
      .justify_center()
      .gap(px(16.))
      .py(px(48.))
      .child(
        div()
          .size(px(64.))
          .rounded(px(12.))
          .bg(colors.sidebar)
          .flex()
          .items_center()
          .justify_center()
          .child(Icon::new(IconName::Search).text_color(colors.muted_foreground)),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child("No Results"),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child(format!("No containers match \"{}\"", self.search_query)),
      )
  }
}

impl gpui::EventEmitter<ContainerListEvent> for ContainerList {}

impl Render for ContainerList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.containers.len();
    let running_count = state.containers.iter().filter(|c| c.state.is_running()).count();

    // Get filtered count
    let filtered_count = self.list_state.read(cx).delegate().filtered_containers(cx).len();
    let is_filtering = !self.search_query.is_empty();
    let containers_empty = filtered_count == 0;

    let subtitle = if is_filtering {
      format!("{filtered_count} of {total_count} ({running_count}  running)")
    } else if running_count > 0 {
      format!("{running_count} running")
    } else {
      "None running".to_string()
    };

    let colors = cx.theme().colors;
    let search_visible = self.search_visible;

    // Ensure search input exists if visible and sync query
    if search_visible {
      self.ensure_search_input(window, cx);
      self.sync_search_query(cx);
    }

    // Toolbar
    let toolbar = h_flex()
      .h(px(52.))
      .w_full()
      .px(px(16.))
      .border_b_1()
      .border_color(colors.border)
      .items_center()
      .justify_between()
      .flex_shrink_0()
      .child(
        v_flex()
          .child(Label::new("Containers"))
          .child(div().text_xs().text_color(colors.muted_foreground).child(subtitle)),
      )
      .child(
        h_flex()
          .items_center()
          .gap(px(8.))
          .child(
            Button::new("search")
              .icon(Icon::new(AppIcon::Search))
              .when(search_visible, Button::primary)
              .when(!search_visible, ButtonVariants::ghost)
              .compact()
              .on_click(cx.listener(|this, _ev, window, cx| {
                this.toggle_search(window, cx);
              })),
          )
          .child(
            Button::new("add")
              .icon(Icon::new(AppIcon::Plus))
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, _window, cx| {
                cx.emit(ContainerListEvent::NewContainer);
              })),
          ),
      );

    // Search bar
    let search_bar = if search_visible {
      Some(
        h_flex()
          .w_full()
          .h(px(40.))
          .px(px(12.))
          .gap(px(8.))
          .items_center()
          .bg(colors.sidebar)
          .border_b_1()
          .border_color(colors.border)
          .child(
            Icon::new(IconName::Search)
              .size(px(16.))
              .text_color(colors.muted_foreground),
          )
          .child(div().flex_1().when_some(self.search_input.clone(), |el, input| {
            el.child(Input::new(&input).small().w_full())
          }))
          .when(!self.search_query.is_empty(), |el| {
            el.child(
              Button::new("clear-search")
                .icon(IconName::Close)
                .ghost()
                .xsmall()
                .on_click(cx.listener(|this, _ev, window, cx| {
                  this.toggle_search(window, cx);
                })),
            )
          }),
      )
    } else {
      None
    };

    let content: gpui::Div = if containers_empty && !is_filtering {
      Self::render_empty(cx)
    } else if containers_empty && is_filtering {
      self.render_no_results(cx)
    } else {
      div().size_full().p(px(8.)).child(List::new(&self.list_state))
    };

    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      .child(toolbar)
      .children(search_bar)
      .child(
        div()
          .id("container-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
