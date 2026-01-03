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
use crate::kubernetes::DeploymentInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};

/// Deployment list events emitted to parent
pub enum DeploymentListEvent {
  Selected(DeploymentInfo),
  NewDeployment,
}

/// Delegate for the deployment list
pub struct DeploymentListDelegate {
  docker_state: Entity<DockerState>,
  selected_index: Option<IndexPath>,
  search_query: String,
}

impl DeploymentListDelegate {
  fn deployments(&self, cx: &App) -> Vec<DeploymentInfo> {
    let state = self.docker_state.read(cx);
    if state.selected_namespace == "all" {
      state.deployments.clone()
    } else {
      state
        .deployments
        .iter()
        .filter(|d| d.namespace == state.selected_namespace)
        .cloned()
        .collect()
    }
  }

  fn filtered_deployments(&self, cx: &App) -> Vec<DeploymentInfo> {
    let deployments = self.deployments(cx);
    if self.search_query.is_empty() {
      return deployments;
    }

    let query = self.search_query.to_lowercase();
    deployments
      .iter()
      .filter(|d| d.name.to_lowercase().contains(&query) || d.namespace.to_lowercase().contains(&query))
      .cloned()
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
    self.selected_index = None;
  }
}

impl ListDelegate for DeploymentListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered_deployments(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let deployments = self.filtered_deployments(cx);
    let deployment = deployments.get(ix.row)?;
    let colors = &cx.theme().colors;

    let is_selected = self.selected_index == Some(ix);
    let deployment_name = deployment.name.clone();
    let deployment_namespace = deployment.namespace.clone();

    // Check if this is a system deployment (don't allow delete)
    let is_system_deployment = matches!(
      deployment.namespace.as_str(),
      "kube-system" | "kube-public" | "kube-node-lease"
    );

    // Color based on ready status
    let is_healthy = deployment.ready_replicas == deployment.replicas && deployment.replicas > 0;
    let icon_bg = if is_healthy {
      colors.success
    } else if deployment.ready_replicas > 0 {
      colors.warning
    } else {
      colors.danger
    };

    let ready_display = deployment.ready_display();
    let subtitle = format!("{} - {}", deployment.namespace, deployment.age);
    let current_replicas = deployment.replicas;

    // Three-dot menu button
    let row = ix.row;
    let menu_button = Button::new(("menu", row))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _window, _cx| {
        let name = deployment_name.clone();
        let ns = deployment_namespace.clone();

        let mut menu = menu
          .item(PopupMenuItem::new("Scale").icon(IconName::Settings2).on_click({
            let name = name.clone();
            let ns = ns.clone();
            move |_, _, cx| {
              services::request_scale_dialog(name.clone(), ns.clone(), current_replicas, cx);
            }
          }))
          .item(
            PopupMenuItem::new("Restart")
              .icon(Icon::new(AppIcon::Restart))
              .on_click({
                let name = name.clone();
                let ns = ns.clone();
                move |_, _, cx| {
                  services::restart_deployment(name.clone(), ns.clone(), cx);
                }
              }),
          )
          .separator()
          .item(PopupMenuItem::new("View YAML").icon(IconName::File).on_click({
            let name = name.clone();
            let ns = ns.clone();
            move |_, _, cx| {
              services::open_deployment_yaml(name.clone(), ns.clone(), cx);
            }
          }));

        // Only show delete for non-system deployments
        if !is_system_deployment {
          menu = menu
            .separator()
            .item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click({
              let name = name.clone();
              let ns = ns.clone();
              move |_, _, cx| {
                services::delete_deployment(name.clone(), ns.clone(), cx);
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
                            .child(Icon::new(AppIcon::Deployment).text_color(colors.background)),
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
                                    .child(deployment.name.clone()),
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
                    // Ready badge
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
                            .child(ready_display),
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

/// Self-contained deployment list component
pub struct DeploymentList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<DeploymentListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl DeploymentList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = DeploymentListDelegate {
      docker_state: docker_state.clone(),
      selected_index: None,
      search_query: String::new(),
    };

    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    // Subscribe to list events
    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered_deployments(cx);
        if let Some(deployment) = filtered.get(ix.row) {
          cx.emit(DeploymentListEvent::Selected(deployment.clone()));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    // Subscribe to docker state changes to refresh list
    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::DeploymentsUpdated | StateChanged::NamespacesUpdated | StateChanged::MachinesUpdated
      ) {
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search deployments..."));
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
    let state = self.docker_state.read(cx);

    // Check if there's a running VM without K8s that we can enable K8s on
    let running_vm_without_k8s = state
      .colima_vms
      .iter()
      .find(|vm| vm.status.is_running() && !vm.kubernetes)
      .map(|vm| vm.name.clone());

    // Check if there's a running VM WITH K8s enabled (but API not responding)
    let running_vm_with_k8s = state
      .colima_vms
      .iter()
      .find(|vm| vm.status.is_running() && vm.kubernetes)
      .map(|vm| vm.name.clone());

    let (title, show_example) = if state.k8s_available {
      ("No Deployments", true)
    } else {
      ("Kubernetes Unavailable", false)
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
            Icon::new(AppIcon::Deployment)
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
      .when(show_example, |el| {
        el.child(
          v_flex()
            .w_full()
            .items_center()
            .gap(px(8.))
            .mt(px(24.))
            .child(
              div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.secondary_foreground)
                .child("Get started with an example"),
            )
            .child(
              h_flex()
                .items_center()
                .gap(px(4.))
                .child(
                  div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(colors.sidebar)
                    .font_family("monospace")
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("kubectl create deployment nginx --image=nginx"),
                )
                .child(
                  Button::new("copy-example")
                    .icon(IconName::Copy)
                    .ghost()
                    .xsmall()
                    .on_click(|_ev, _window, cx| {
                      cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                        "kubectl create deployment nginx --image=nginx".to_string(),
                      ));
                    }),
                ),
            ),
        )
      })
      .when(!show_example, |el| {
        let message = if running_vm_without_k8s.is_some() {
          "Kubernetes is not enabled on the current machine"
        } else if running_vm_with_k8s.is_some() {
          "Kubernetes API is not responding"
        } else {
          "Start a Colima VM with Kubernetes enabled"
        };

        el.child(
          v_flex()
            .items_center()
            .gap(px(12.))
            .child(div().text_sm().text_color(colors.muted_foreground).child(message))
            .when_some(running_vm_without_k8s.clone(), |el, vm_name| {
              el.child(
                Button::new("enable-k8s")
                  .label(format!("Enable Kubernetes on '{vm_name}'"))
                  .primary()
                  .on_click(move |_ev, _window, cx| {
                    services::enable_kubernetes(vm_name.clone(), cx);
                  }),
              )
            })
            .when(
              running_vm_without_k8s.is_none() && running_vm_with_k8s.is_some(),
              |el| {
                let vm_name = running_vm_with_k8s.clone().unwrap();
                el.child(
                  Button::new("restart-k8s")
                    .label(format!("Restart '{vm_name}'"))
                    .primary()
                    .on_click(move |_ev, _window, cx| {
                      services::restart_machine(vm_name.clone(), cx);
                    }),
                )
              },
            ),
        )
      })
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
          .child(format!("No deployments match \"{}\"", self.search_query)),
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
          for ns in &namespaces {
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

impl gpui::EventEmitter<DeploymentListEvent> for DeploymentList {}

impl Render for DeploymentList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.deployments.len();

    let filtered_count = self.list_state.read(cx).delegate().filtered_deployments(cx).len();
    let is_filtering = !self.search_query.is_empty();
    let deployments_empty = filtered_count == 0;

    let subtitle = if is_filtering {
      format!("{filtered_count} of {total_count}")
    } else {
      format!("{total_count} total")
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
          .child(Label::new("Deployments"))
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
              .when(!search_visible, ButtonVariants::ghost)
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
                services::refresh_deployments(cx);
              })),
          )
          .child(
            Button::new("add")
              .icon(Icon::new(AppIcon::Plus))
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, _window, cx| {
                cx.emit(DeploymentListEvent::NewDeployment);
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

    let content: gpui::Div = if deployments_empty && !is_filtering {
      self.render_empty(cx)
    } else if deployments_empty && is_filtering {
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
          .id("deployment-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
