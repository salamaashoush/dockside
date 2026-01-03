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
use crate::kubernetes::ServiceInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};

/// Service list events emitted to parent
pub enum ServiceListEvent {
  Selected(ServiceInfo),
  NewService,
}

/// Delegate for the service list
pub struct ServiceListDelegate {
  docker_state: Entity<DockerState>,
  selected_index: Option<IndexPath>,
  search_query: String,
}

impl ServiceListDelegate {
  fn services(&self, cx: &App) -> Vec<ServiceInfo> {
    let state = self.docker_state.read(cx);
    if state.selected_namespace == "all" {
      state.services.clone()
    } else {
      state
        .services
        .iter()
        .filter(|s| s.namespace == state.selected_namespace)
        .cloned()
        .collect()
    }
  }

  fn filtered_services(&self, cx: &App) -> Vec<ServiceInfo> {
    let services = self.services(cx);
    if self.search_query.is_empty() {
      return services;
    }

    let query = self.search_query.to_lowercase();
    services
      .iter()
      .filter(|s| {
        s.name.to_lowercase().contains(&query)
          || s.namespace.to_lowercase().contains(&query)
          || s.service_type.to_lowercase().contains(&query)
      })
      .cloned()
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
    self.selected_index = None;
  }
}

impl ListDelegate for ServiceListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered_services(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let services = self.filtered_services(cx);
    let service = services.get(ix.row)?;
    let colors = &cx.theme().colors;

    let is_selected = self.selected_index == Some(ix);
    let service_name = service.name.clone();
    let service_namespace = service.namespace.clone();

    let icon_bg = colors.primary;

    let subtitle = format!("{} - {}", service.namespace, service.service_type);
    let ports_display = service.ports_display();

    // Check if this is a system service (don't allow delete)
    let is_system_service = matches!(
      service.namespace.as_str(),
      "kube-system" | "kube-public" | "kube-node-lease"
    ) || (service.namespace == "default" && service.name == "kubernetes");

    // Three-dot menu button
    let row = ix.row;
    let menu_button = Button::new(("menu", row))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _window, _cx| {
        let name = service_name.clone();
        let ns = service_namespace.clone();

        let mut menu = menu.item(PopupMenuItem::new("View YAML").icon(IconName::File).on_click({
          let name = name.clone();
          let ns = ns.clone();
          move |_, _, cx| {
            services::open_service_yaml(name.clone(), ns.clone(), cx);
          }
        }));

        // Only show delete for non-system services
        if !is_system_service {
          menu = menu
            .separator()
            .item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click({
              let name = name.clone();
              let ns = ns.clone();
              move |_, _, cx| {
                services::delete_service(name.clone(), ns.clone(), cx);
              }
            }));
        }

        menu
      });

    // Build item content with menu button INSIDE
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
              .child(Icon::new(IconName::Globe).text_color(colors.background)),
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
                  .child(service.name.clone()),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(subtitle),
              )
              .when(!ports_display.is_empty(), |el| {
                el.child(
                  div()
                    .text_xs()
                    .text_color(colors.success)
                    .text_ellipsis()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(ports_display),
                )
              }),
          ),
      )
      .child(div().flex_shrink_0().child(menu_button));

    let item = ListItem::new(ix)
      .py(px(6.))
      .rounded(px(6.))
      .overflow_hidden()
      .selected(is_selected)
      .child(item_content)
      .on_click(cx.listener(move |this, _ev, _window, cx| {
        this.delegate_mut().selected_index = Some(ix);
        cx.notify();
      }));

    Some(item)
  }

  fn set_selected_index(&mut self, ix: Option<IndexPath>, _window: &mut Window, cx: &mut Context<'_, ListState<Self>>) {
    self.selected_index = ix;
    cx.notify();
  }
}

/// Self-contained service list component
pub struct ServiceList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<ServiceListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl ServiceList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = ServiceListDelegate {
      docker_state: docker_state.clone(),
      selected_index: None,
      search_query: String::new(),
    };

    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    // Subscribe to list events
    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered_services(cx);
        if let Some(service) = filtered.get(ix.row) {
          cx.emit(ServiceListEvent::Selected(service.clone()));
        }
      }
      _ => {}
    })
    .detach();

    // Subscribe to docker state changes to refresh list
    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::ServicesUpdated | StateChanged::NamespacesUpdated) {
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search services..."));
      self.search_input = Some(input_state);
    }
  }

  fn sync_search_query(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(input) = &self.search_input {
      let current_text = input.read(cx).text().to_string();
      if current_text != self.search_query {
        self.search_query = current_text.clone();
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
    let state = self.docker_state.read(cx);

    let (title, subtitle) = if !state.k8s_available {
      ("Kubernetes Unavailable", "Start a Colima VM with Kubernetes enabled")
    } else {
      ("No Services", "Deploy an application to see services here")
    };

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
          .child(Icon::new(IconName::Globe).text_color(colors.muted_foreground)),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child(title),
      )
      .child(div().text_sm().text_color(colors.muted_foreground).child(subtitle))
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
          .child(format!("No services match \"{}\"", self.search_query)),
      )
  }

  fn render_namespace_selector(&self, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let selected = state.selected_namespace.clone();
    let namespaces = state.namespaces.clone();

    let display = if selected == "all" {
      "All".to_string()
    } else {
      selected.clone()
    };

    Button::new("namespace-selector")
      .label(display)
      .ghost()
      .compact()
      .dropdown_menu(move |menu, _window, _cx| {
        let mut menu = menu.item(PopupMenuItem::new("All Namespaces").on_click({
          move |_, _, cx| {
            services::set_namespace("all".to_string(), cx);
          }
        }));

        if !namespaces.is_empty() {
          menu = menu.separator();
          for ns in namespaces.iter() {
            let ns_clone = ns.clone();
            menu = menu.item(PopupMenuItem::new(ns.clone()).on_click({
              let ns = ns_clone.clone();
              move |_, _, cx| {
                services::set_namespace(ns.clone(), cx);
              }
            }));
          }
        }

        menu
      })
  }
}

impl gpui::EventEmitter<ServiceListEvent> for ServiceList {}

impl Render for ServiceList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.services.len();

    let filtered_count = self.list_state.read(cx).delegate().filtered_services(cx).len();
    let is_filtering = !self.search_query.is_empty();
    let services_empty = filtered_count == 0;

    let subtitle = if is_filtering {
      format!("{} of {}", filtered_count, total_count)
    } else {
      format!("{} total", total_count)
    };

    let colors = cx.theme().colors;
    let search_visible = self.search_visible;

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
          .child(Label::new("Services"))
          .child(div().text_xs().text_color(colors.muted_foreground).child(subtitle)),
      )
      .child(
        h_flex()
          .items_center()
          .gap(px(8.))
          .child(self.render_namespace_selector(cx))
          .child(
            Button::new("search")
              .icon(Icon::new(AppIcon::Search))
              .when(search_visible, Button::primary)
              .when(!search_visible, |b| b.ghost())
              .compact()
              .on_click(cx.listener(|this, _ev, window, cx| {
                this.toggle_search(window, cx);
              })),
          )
          .child(
            Button::new("refresh")
              .icon(Icon::new(AppIcon::Restart))
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, _window, cx| {
                services::refresh_services(cx);
              })),
          )
          .child(
            Button::new("add")
              .icon(Icon::new(AppIcon::Plus))
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, _window, cx| {
                cx.emit(ServiceListEvent::NewService);
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

    let content: gpui::Div = if services_empty && !is_filtering {
      self.render_empty(cx)
    } else if services_empty && is_filtering {
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
          .id("service-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
