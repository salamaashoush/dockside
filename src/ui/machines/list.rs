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
use crate::colima::ColimaVm;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};

/// Machine list events emitted to parent
pub enum MachineListEvent {
  Selected(ColimaVm),
  NewMachine,
}

/// Delegate for the machine list
pub struct MachineListDelegate {
  docker_state: Entity<DockerState>,
  selected_index: Option<IndexPath>,
  search_query: String,
}

impl MachineListDelegate {
  fn machines<'a>(&self, cx: &'a App) -> &'a Vec<ColimaVm> {
    &self.docker_state.read(cx).colima_vms
  }

  fn filtered_machines(&self, cx: &App) -> Vec<ColimaVm> {
    let machines = self.machines(cx);
    if self.search_query.is_empty() {
      return machines.clone();
    }

    let query = self.search_query.to_lowercase();
    machines
      .iter()
      .filter(|m| {
        m.name.to_lowercase().contains(&query)
          || m.arch.to_string().to_lowercase().contains(&query)
          || m.status.to_string().to_lowercase().contains(&query)
      })
      .cloned()
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
    self.selected_index = None;
  }
}

impl ListDelegate for MachineListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered_machines(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let machines = self.filtered_machines(cx);
    let machine = machines.get(ix.row)?;
    let colors = &cx.theme().colors;

    let is_selected = self.selected_index == Some(ix);
    let is_running = machine.status.is_running();
    let machine_name = machine.name.clone();

    let icon_bg = if is_running {
      colors.primary
    } else {
      colors.muted_foreground
    };

    let subtitle = format!("latest, {}", machine.arch);
    let status_color = if is_running {
      colors.success
    } else {
      colors.muted_foreground
    };

    let item_content = h_flex()
            .w_full()
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
                    .overflow_hidden()
                    .gap(px(2.))
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(Label::new(machine.name.clone())),
                    )
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(subtitle),
                    ),
            )
            // Status dot
            .child(
                div()
                    .flex_shrink_0()
                    .size(px(8.))
                    .rounded_full()
                    .bg(status_color),
            );

    // Three-dot menu button
    let name = machine_name.clone();
    let running = is_running;
    let has_k8s = machine.kubernetes;
    let row = ix.row;

    let item = ListItem::new(ix)
      .py(px(6.))
      .rounded(px(6.))
      .selected(is_selected)
      .child(item_content)
      .suffix(move |_, _| {
        let n = name.clone();
        div()
          .size(px(28.))
          .flex_shrink_0()
          .flex()
          .items_center()
          .justify_center()
          .child(
            Button::new(("menu", row))
              .icon(IconName::Ellipsis)
              .ghost()
              .xsmall()
              .dropdown_menu(move |menu, _window, _cx| {
                let mut menu = menu;
                let n = n.clone();

                if running {
                  menu = menu
                    .item(
                      PopupMenuItem::new("Use Docker Context")
                        .icon(IconName::Check)
                        .on_click({
                          let n = n.clone();
                          move |_, _, cx| {
                            services::set_docker_context(n.clone(), cx);
                          }
                        }),
                    )
                    .separator()
                    .item(PopupMenuItem::new("Stop").icon(Icon::new(AppIcon::Stop)).on_click({
                      let n = n.clone();
                      move |_, _, cx| {
                        services::stop_machine(n.clone(), cx);
                      }
                    }))
                    .item(
                      PopupMenuItem::new("Restart")
                        .icon(Icon::new(AppIcon::Restart))
                        .on_click({
                          let n = n.clone();
                          move |_, _, cx| {
                            services::restart_machine(n.clone(), cx);
                          }
                        }),
                    )
                    .separator()
                    .item(
                      PopupMenuItem::new("Terminal")
                        .icon(Icon::new(AppIcon::Terminal))
                        .on_click({
                          let n = n.clone();
                          move |_, _, cx| {
                            services::open_machine_terminal(n.clone(), cx);
                          }
                        }),
                    )
                    .item(PopupMenuItem::new("Files").icon(Icon::new(AppIcon::Files)).on_click({
                      let n = n.clone();
                      move |_, _, cx| {
                        services::open_machine_files(n.clone(), cx);
                      }
                    }));

                  // Kubernetes actions (only if machine has K8s enabled)
                  if has_k8s {
                    menu = menu
                      .separator()
                      .item(
                        PopupMenuItem::new("K8s Start")
                          .icon(Icon::new(AppIcon::Kubernetes))
                          .on_click({
                            let n = n.clone();
                            move |_, _, cx| {
                              services::kubernetes_start(n.clone(), cx);
                            }
                          }),
                      )
                      .item(
                        PopupMenuItem::new("K8s Stop")
                          .icon(Icon::new(AppIcon::Kubernetes))
                          .on_click({
                            let n = n.clone();
                            move |_, _, cx| {
                              services::kubernetes_stop(n.clone(), cx);
                            }
                          }),
                      )
                      .item(
                        PopupMenuItem::new("K8s Reset")
                          .icon(Icon::new(AppIcon::Kubernetes))
                          .on_click({
                            let n = n.clone();
                            move |_, _, cx| {
                              services::kubernetes_reset(n.clone(), cx);
                            }
                          }),
                      );
                  }
                } else {
                  menu = menu.item(PopupMenuItem::new("Start").icon(Icon::new(AppIcon::Play)).on_click({
                    let n = n.clone();
                    move |_, _, cx| {
                      services::start_machine(n.clone(), cx);
                    }
                  }));
                }

                menu = menu
                  .separator()
                  .item(PopupMenuItem::new("Edit").icon(IconName::Settings).on_click({
                    let n = n.clone();
                    move |_, _, cx| {
                      docker_state(cx).update(cx, |_, cx| {
                        cx.emit(StateChanged::EditMachineRequest {
                          machine_name: n.clone(),
                        });
                      });
                    }
                  }))
                  .item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click({
                    let n = n.clone();
                    move |_, _, cx| {
                      services::delete_machine(n.clone(), cx);
                    }
                  }));

                menu
              }),
          )
      })
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

/// Self-contained machine list component with working context menus
pub struct MachineList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<MachineListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl MachineList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = MachineListDelegate {
      docker_state: docker_state.clone(),
      selected_index: None,
      search_query: String::new(),
    };

    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    // Subscribe to list events
    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered_machines(cx);
        if let Some(machine) = filtered.get(ix.row) {
          cx.emit(MachineListEvent::Selected(machine.clone()));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    // Subscribe to docker state changes to refresh list
    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::MachinesUpdated) {
        // Notify list state to re-render
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search machines..."));
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
          .child(Icon::new(AppIcon::Machine).text_color(colors.muted_foreground)),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child("No Machines"),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Start Colima to create a Linux VM"),
      )
      .child(
        Button::new("new-machine")
          .label("New Machine")
          .primary()
          .on_click(cx.listener(|_this, _ev, _window, cx| {
            cx.emit(MachineListEvent::NewMachine);
          })),
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
          .child(format!("No machines match \"{}\"", self.search_query)),
      )
  }
}

impl gpui::EventEmitter<MachineListEvent> for MachineList {}

impl Render for MachineList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.colima_vms.len();
    let running_count = state.colima_vms.iter().filter(|m| m.status.is_running()).count();
    let colors = cx.theme().colors;

    // Get filtered count
    let filtered_count = self.list_state.read(cx).delegate().filtered_machines(cx).len();
    let is_filtering = !self.search_query.is_empty();
    let machines_empty = filtered_count == 0;

    let search_visible = self.search_visible;

    // Ensure search input exists if visible and sync query
    if search_visible {
      self.ensure_search_input(window, cx);
      self.sync_search_query(cx);
    }

    let subtitle = if is_filtering {
      format!("{filtered_count} of {total_count} ({running_count} running)")
    } else if running_count > 0 {
      format!("{running_count} running")
    } else {
      "None running".to_string()
    };

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
          .child(Label::new("Machines"))
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
                cx.emit(MachineListEvent::NewMachine);
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

    let content: gpui::Div = if machines_empty && !is_filtering {
      Self::render_empty(cx)
    } else if machines_empty && is_filtering {
      self.render_no_results(cx)
    } else {
      // List needs explicit height for virtualization
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
          .id("machine-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
