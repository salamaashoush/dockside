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
use crate::kubernetes::NodeInfo;
use crate::services;
use crate::state::{DockerState, FavoriteRef, LoadState, Selection, StateChanged, docker_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub enum NodeListEvent {
  Selected(NodeInfo),
}

pub struct NodeListDelegate {
  docker_state: Entity<DockerState>,
  search_query: String,
}

impl NodeListDelegate {
  fn filtered_nodes(&self, cx: &App) -> Vec<NodeInfo> {
    let nodes = self.docker_state.read(cx).nodes.clone();
    if self.search_query.is_empty() {
      return nodes;
    }
    let q = self.search_query.to_lowercase();
    nodes
      .into_iter()
      .filter(|n| n.name.to_lowercase().contains(&q) || n.roles.iter().any(|r| r.to_lowercase().contains(&q)))
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
  }
}

impl ListDelegate for NodeListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered_nodes(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let nodes = self.filtered_nodes(cx);
    let node = nodes.get(ix.row)?;
    let colors = &cx.theme().colors;

    let global_selection = &self.docker_state.read(cx).selection;
    let is_selected = matches!(global_selection, Selection::Node(name) if *name == node.name);

    let is_ready = node.status == "Ready";
    let icon_bg = if !is_ready {
      colors.danger
    } else if node.unschedulable {
      colors.warning
    } else {
      colors.success
    };

    let roles = if node.roles.is_empty() {
      "worker".to_string()
    } else {
      node.roles.join(",")
    };
    let status_text = if node.unschedulable && is_ready {
      "Ready,SchedulingDisabled".to_string()
    } else {
      node.status.clone()
    };
    let subtitle = format!("{roles} · {} · {}", node.version, node.age);

    let node_name = node.name.clone();
    let unschedulable = node.unschedulable;
    let pin = FavoriteRef::Node {
      name: node.name.clone(),
    };
    let pinned = services::is_favorite(&pin, cx);

    let row = ix.row;
    let menu_button = Button::new(("menu", row))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _window, _cx| {
        let name = node_name.clone();
        let mut menu = menu;
        if unschedulable {
          menu = menu.item(PopupMenuItem::new("Uncordon").icon(IconName::Check).on_click({
            let name = name.clone();
            move |_, _, cx| services::uncordon_node(name.clone(), cx)
          }));
        } else {
          menu = menu.item(PopupMenuItem::new("Cordon").icon(IconName::CircleX).on_click({
            let name = name.clone();
            move |_, _, cx| services::cordon_node(name.clone(), cx)
          }));
        }
        menu = menu
          .item(PopupMenuItem::new("Drain").icon(Icon::new(AppIcon::Trash)).on_click({
            let name = name.clone();
            move |_, _, cx| services::drain_node(name.clone(), cx)
          }))
          .separator()
          .item(PopupMenuItem::new("View YAML").icon(IconName::File).on_click({
            let name = name.clone();
            move |_, _, cx| services::open_node_yaml(name.clone(), cx)
          }))
          .separator()
          .item(
            PopupMenuItem::new(if pinned {
              "Unpin from Dashboard"
            } else {
              "Pin to Dashboard"
            })
            .icon(IconName::Star)
            .on_click({
              let pin = pin.clone();
              move |_, _, cx| services::toggle_favorite(pin.clone(), cx)
            }),
          );
        menu
      });

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
          .gap(px(10.))
          .child(
            div()
              .size(px(36.))
              .flex_shrink_0()
              .rounded(px(8.))
              .bg(icon_bg)
              .flex()
              .items_center()
              .justify_center()
              .child(Icon::new(AppIcon::Machine).text_color(colors.background)),
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
                  .child(node.name.clone()),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(subtitle),
              ),
          )
          .child(
            div()
              .flex_shrink_0()
              .px(px(8.))
              .py(px(2.))
              .rounded(px(4.))
              .bg(icon_bg.opacity(0.2))
              .text_xs()
              .font_weight(gpui::FontWeight::MEDIUM)
              .text_color(icon_bg)
              .child(status_text),
          ),
      )
      .child(div().flex_shrink_0().child(menu_button));

    Some(
      ListItem::new(ix)
        .py(px(6.))
        .rounded(px(6.))
        .overflow_hidden()
        .selected(is_selected)
        .child(item_content),
    )
  }

  fn set_selected_index(
    &mut self,
    _ix: Option<IndexPath>,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) {
    cx.notify();
  }
}

pub struct NodeList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<NodeListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl NodeList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = NodeListDelegate {
      docker_state: docker_state.clone(),
      search_query: String::new(),
    };
    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered_nodes(cx);
        if let Some(node) = filtered.get(ix.row) {
          cx.emit(NodeListEvent::Selected(node.clone()));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::NodesUpdated | StateChanged::SelectionChanged | StateChanged::KubeContextSwitched
      ) {
        this.list_state.update(cx, |_state, cx| cx.notify());
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search nodes..."));
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
      self.search_query.clear();
      self.search_input = None;
      self.list_state.update(cx, |state, cx| {
        state.delegate_mut().set_search_query(String::new());
        cx.notify();
      });
    }
    cx.notify();
  }

  fn render_empty(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let k8s_available = self.docker_state.read(cx).k8s_available;
    let title = if k8s_available {
      "No Nodes"
    } else {
      "Kubernetes Unavailable"
    };
    v_flex()
      .flex_1()
      .w_full()
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
          .child(
            Icon::new(AppIcon::Machine)
              .size(px(32.))
              .text_color(colors.muted_foreground),
          ),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child(title),
      )
  }
}

impl gpui::EventEmitter<NodeListEvent> for NodeList {}

impl Render for NodeList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total = state.nodes.len();
    let nodes_state = state.nodes_state.clone();

    let filtered = self.list_state.read(cx).delegate().filtered_nodes(cx).len();
    let is_filtering = !self.search_query.is_empty();

    let subtitle = match &nodes_state {
      LoadState::NotLoaded | LoadState::Loading => "Loading...".to_string(),
      LoadState::Error(_) => "Error loading".to_string(),
      LoadState::Loaded => {
        if is_filtering {
          format!("{filtered} of {total}")
        } else {
          format!("{total} total")
        }
      }
    };

    let colors = cx.theme().colors;
    let search_visible = self.search_visible;

    if search_visible {
      self.ensure_search_input(window, cx);
      self.sync_search_query(cx);
    }

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
          .child(Label::new("Nodes"))
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
            Button::new("nodes-add")
              .icon(IconName::Plus)
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, window, cx| {
                super::join_dialog::open_add_node_dialog(window, cx);
              })),
          )
          .child(
            Button::new("nodes-refresh")
              .icon(Icon::new(AppIcon::Refresh))
              .ghost()
              .compact()
              .on_click(|_, _, cx| services::refresh_nodes(cx)),
          ),
      );

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
            Icon::new(AppIcon::Search)
              .size(px(16.))
              .text_color(colors.muted_foreground),
          )
          .child(div().flex_1().when_some(self.search_input.clone(), |el, input| {
            el.child(Input::new(&input).small().w_full())
          })),
      )
    } else {
      None
    };

    let content: gpui::Div = match &nodes_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("nodes", cx),
      LoadState::Error(e) => {
        let msg = e.clone();
        render_k8s_error("nodes", &msg, |_ev, _window, cx| services::refresh_nodes(cx), cx)
      }
      LoadState::Loaded => {
        if filtered == 0 {
          self.render_empty(cx)
        } else {
          div().size_full().p(px(8.)).child(List::new(&self.list_state))
        }
      }
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
          .id("node-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
