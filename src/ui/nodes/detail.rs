use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::kubernetes::{NodeInfo, NodeTaint, is_reserved_node_label};
use crate::services;
use crate::state::{DockerState, NodeDetailTab, StateChanged, docker_state};

pub struct NodeDetail {
  docker_state: Entity<DockerState>,
  node: Option<NodeInfo>,
  active_tab: NodeDetailTab,
  yaml_content: String,
  yaml_editor: Option<Entity<InputState>>,
  last_synced_yaml: String,
  taint_key: Option<Entity<InputState>>,
  taint_value: Option<Entity<InputState>>,
  taint_effect: Option<Entity<InputState>>,
  label_key: Option<Entity<InputState>>,
  label_value: Option<Entity<InputState>>,
}

impl NodeDetail {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |this, ds, event: &StateChanged, cx| match event {
      StateChanged::NodeYamlLoaded { name, yaml } => {
        if let Some(ref n) = this.node
          && n.name == *name
        {
          yaml.clone_into(&mut this.yaml_content);
          cx.notify();
        }
      }
      StateChanged::NodeTabRequest { name, tab } => {
        let node = ds.read(cx).get_node(name).cloned();
        if let Some(n) = node {
          this.node = Some(n);
          this.active_tab = *tab;
          this.yaml_content.clear();
          if this.active_tab == NodeDetailTab::Yaml {
            services::get_node_yaml(name.clone(), cx);
          }
          cx.notify();
        }
      }
      StateChanged::NodesUpdated => {
        if let Some(ref current) = this.node {
          let updated = ds.read(cx).get_node(&current.name).cloned();
          if let Some(n) = updated {
            this.node = Some(n);
            cx.notify();
          } else {
            this.node = None;
            cx.notify();
          }
        }
      }
      _ => {}
    })
    .detach();

    Self {
      docker_state,
      node: None,
      active_tab: NodeDetailTab::Info,
      yaml_content: String::new(),
      yaml_editor: None,
      last_synced_yaml: String::new(),
      taint_key: None,
      taint_value: None,
      taint_effect: None,
      label_key: None,
      label_value: None,
    }
  }

  pub fn set_node(&mut self, node: NodeInfo, cx: &mut Context<'_, Self>) {
    self.node = Some(node);
    self.active_tab = NodeDetailTab::Info;
    self.yaml_content.clear();
    self.yaml_editor = None;
    self.last_synced_yaml.clear();
    cx.notify();
  }

  fn info_row(label: &str, value: String, cx: &Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    h_flex()
      .w_full()
      .py(px(8.))
      .gap(px(16.))
      .child(
        div()
          .w(px(160.))
          .flex_shrink_0()
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.muted_foreground)
          .child(label.to_string()),
      )
      .child(div().flex_1().text_sm().text_color(colors.foreground).child(value))
  }

  fn render_info(node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let roles = if node.roles.is_empty() {
      "worker".to_string()
    } else {
      node.roles.join(", ")
    };
    let content = v_flex()
      .w_full()
      .gap(px(4.))
      .child(Self::info_row("Name", node.name.clone(), cx))
      .child(Self::info_row("Status", node.status.clone(), cx))
      .child(Self::info_row(
        "Schedulable",
        if node.unschedulable { "No (cordoned)" } else { "Yes" }.to_string(),
        cx,
      ))
      .child(Self::info_row("Roles", roles, cx))
      .child(Self::info_row("Kubelet", node.version.clone(), cx))
      .child(Self::info_row("OS / Arch", format!("{} / {}", node.os, node.arch), cx))
      .child(Self::info_row("Kernel", node.kernel_version.clone(), cx))
      .child(Self::info_row("Container Runtime", node.container_runtime.clone(), cx))
      .child(Self::info_row(
        "Internal IP",
        node.internal_ip.clone().unwrap_or_else(|| "<none>".to_string()),
        cx,
      ))
      .child(Self::info_row(
        "Pod CIDR",
        node.pod_cidr.clone().unwrap_or_else(|| "<none>".to_string()),
        cx,
      ))
      .child(Self::info_row(
        "CPU (alloc / cap)",
        format!("{} / {}", node.cpu_allocatable, node.cpu_capacity),
        cx,
      ))
      .child(Self::info_row(
        "Memory (alloc / cap)",
        format!("{} / {}", node.mem_allocatable, node.mem_capacity),
        cx,
      ))
      .child(Self::info_row(
        "Pods (alloc / cap)",
        format!("{} / {}", node.pods_allocatable, node.pods_capacity),
        cx,
      ))
      .child(Self::info_row("Age", node.age.clone(), cx));

    div()
      .size_full()
      .child(div().w_full().h_full().p(px(16.)).overflow_y_scrollbar().child(content))
  }

  fn render_pods(&self, node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.docker_state.read(cx);
    let pods: Vec<_> = state
      .pods
      .iter()
      .filter(|p| p.node.as_deref() == Some(node.name.as_str()))
      .cloned()
      .collect();

    if pods.is_empty() {
      return div().size_full().flex().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No pods scheduled on this node"),
      );
    }

    let rows = pods
      .iter()
      .enumerate()
      .map(|(i, pod)| {
        let status_color = if pod.phase.is_running() {
          colors.success
        } else if pod.phase.is_pending() {
          colors.warning
        } else {
          colors.danger
        };
        let name = pod.name.clone();
        let ns = pod.namespace.clone();
        h_flex()
          .w_full()
          .py(px(8.))
          .px(px(12.))
          .gap(px(8.))
          .rounded(px(6.))
          .when(i % 2 == 1, |el| el.bg(colors.sidebar.opacity(0.3)))
          .hover(|el| el.bg(colors.sidebar))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_sm()
              .font_family("monospace")
              .text_ellipsis()
              .overflow_hidden()
              .whitespace_nowrap()
              .text_color(colors.foreground)
              .child(format!("{}/{}", pod.namespace, pod.name)),
          )
          .child(
            div().w(px(90.)).flex_shrink_0().child(
              div()
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(status_color.opacity(0.15))
                .text_xs()
                .text_color(status_color)
                .child(pod.phase.to_string()),
            ),
          )
          .child(
            div()
              .w(px(60.))
              .flex_shrink_0()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child(pod.ready.clone()),
          )
          .child(
            div().w(px(36.)).flex_shrink_0().flex().justify_end().child(
              Button::new(("view-pod", i))
                .icon(IconName::Eye)
                .ghost()
                .xsmall()
                .on_click(move |_ev, _w, cx| {
                  services::open_pod_info(name.clone(), ns.clone(), cx);
                }),
            ),
          )
      })
      .collect::<Vec<_>>();

    div().size_full().p(px(16.)).child(
      v_flex()
        .w_full()
        .gap(px(8.))
        .child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(format!("{} pod(s) on this node", pods.len())),
        )
        .child(v_flex().w_full().child(div().w_full().children(rows))),
    )
  }

  fn render_conditions(node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let rows = node
      .conditions
      .iter()
      .map(|c| {
        let ok = c.status == "True" && c.type_ == "Ready" || (c.status == "False" && c.type_ != "Ready");
        let color = if ok { colors.success } else { colors.warning };
        v_flex()
          .w_full()
          .gap(px(2.))
          .py(px(8.))
          .border_b_1()
          .border_color(colors.border)
          .child(
            h_flex()
              .w_full()
              .gap(px(8.))
              .items_center()
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_color(colors.foreground)
                  .child(c.type_.clone()),
              )
              .child(
                div()
                  .px(px(6.))
                  .py(px(1.))
                  .rounded(px(4.))
                  .bg(color.opacity(0.15))
                  .text_xs()
                  .text_color(color)
                  .child(c.status.clone()),
              )
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(c.reason.clone()),
              ),
          )
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child(c.message.clone()),
          )
      })
      .collect::<Vec<_>>();

    div().size_full().child(
      div()
        .w_full()
        .h_full()
        .p(px(16.))
        .overflow_y_scrollbar()
        .child(v_flex().w_full().children(rows)),
    )
  }

  fn render_taints(&self, node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let node_name = node.name.clone();
    let taints = node.taints.clone();

    let rows = taints
      .iter()
      .cloned()
      .enumerate()
      .map(|(i, t)| {
        let all = taints.clone();
        let name = node_name.clone();
        h_flex()
          .w_full()
          .py(px(6.))
          .px(px(8.))
          .gap(px(8.))
          .items_center()
          .rounded(px(6.))
          .when(i % 2 == 1, |el| el.bg(colors.sidebar.opacity(0.3)))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_sm()
              .font_family("monospace")
              .text_color(colors.foreground)
              .child(t.display()),
          )
          .child(
            Button::new(("rm-taint", i))
              .icon(IconName::Delete)
              .ghost()
              .xsmall()
              .on_click(move |_ev, _w, cx| {
                let remaining: Vec<NodeTaint> = all
                  .iter()
                  .enumerate()
                  .filter(|(j, _)| *j != i)
                  .map(|(_, x)| x.clone())
                  .collect();
                services::set_node_taints(name.clone(), remaining, cx);
              }),
          )
      })
      .collect::<Vec<_>>();

    let add_name = node_name.clone();
    let existing = taints.clone();
    let key_in = self.taint_key.clone();
    let val_in = self.taint_value.clone();
    let eff_in = self.taint_effect.clone();
    let add_row = h_flex()
      .w_full()
      .gap(px(8.))
      .items_center()
      .mt(px(12.))
      .when_some(self.taint_key.clone(), |el, i| {
        el.child(div().flex_1().child(Input::new(&i).small().w_full()))
      })
      .when_some(self.taint_value.clone(), |el, i| {
        el.child(div().flex_1().child(Input::new(&i).small().w_full()))
      })
      .when_some(self.taint_effect.clone(), |el, i| {
        el.child(div().w(px(150.)).child(Input::new(&i).small().w_full()))
      })
      .child(
        Button::new("add-taint")
          .label("Add")
          .primary()
          .small()
          .on_click(move |_ev, _w, cx| {
            let (Some(k), Some(v), Some(e)) = (&key_in, &val_in, &eff_in) else {
              return;
            };
            let key = k.read(cx).text().to_string().trim().to_string();
            if key.is_empty() {
              return;
            }
            let value = v.read(cx).text().to_string().trim().to_string();
            let mut effect = e.read(cx).text().to_string().trim().to_string();
            if effect.is_empty() {
              effect = "NoSchedule".to_string();
            }
            let mut next = existing.clone();
            next.push(NodeTaint { key, value, effect });
            services::set_node_taints(add_name.clone(), next, cx);
          }),
      );

    div().size_full().child(
      div().w_full().h_full().p(px(16.)).overflow_y_scrollbar().child(
        v_flex()
          .w_full()
          .gap(px(4.))
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("Taints applied to this node. Effect: NoSchedule | PreferNoSchedule | NoExecute"),
          )
          .children(rows)
          .child(add_row),
      ),
    )
  }

  fn render_labels(&self, node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let node_name = node.name.clone();

    let rows = node
      .labels
      .iter()
      .cloned()
      .enumerate()
      .map(|(i, (k, v))| {
        let reserved = is_reserved_node_label(&k);
        let name = node_name.clone();
        let key_for_rm = k.clone();
        h_flex()
          .w_full()
          .py(px(6.))
          .px(px(8.))
          .gap(px(8.))
          .items_center()
          .rounded(px(6.))
          .when(i % 2 == 1, |el| el.bg(colors.sidebar.opacity(0.3)))
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_sm()
              .font_family("monospace")
              .text_color(if reserved {
                colors.muted_foreground
              } else {
                colors.foreground
              })
              .child(format!("{k}={v}")),
          )
          .when(reserved, |el| {
            el.child(div().text_xs().text_color(colors.muted_foreground).child("read-only"))
          })
          .when(!reserved, |el| {
            el.child(
              Button::new(("rm-label", i))
                .icon(IconName::Delete)
                .ghost()
                .xsmall()
                .on_click(move |_ev, _w, cx| {
                  services::set_node_labels(name.clone(), vec![], vec![key_for_rm.clone()], cx);
                }),
            )
          })
      })
      .collect::<Vec<_>>();

    let add_name = node_name.clone();
    let key_in = self.label_key.clone();
    let val_in = self.label_value.clone();
    let add_row = h_flex()
      .w_full()
      .gap(px(8.))
      .items_center()
      .mt(px(12.))
      .when_some(self.label_key.clone(), |el, i| {
        el.child(div().flex_1().child(Input::new(&i).small().w_full()))
      })
      .when_some(self.label_value.clone(), |el, i| {
        el.child(div().flex_1().child(Input::new(&i).small().w_full()))
      })
      .child(
        Button::new("add-label")
          .label("Add")
          .primary()
          .small()
          .on_click(move |_ev, _w, cx| {
            let (Some(k), Some(v)) = (&key_in, &val_in) else {
              return;
            };
            let key = k.read(cx).text().to_string().trim().to_string();
            if key.is_empty() {
              return;
            }
            let value = v.read(cx).text().to_string().trim().to_string();
            services::set_node_labels(add_name.clone(), vec![(key, value)], vec![], cx);
          }),
      );

    div().size_full().child(
      div().w_full().h_full().p(px(16.)).overflow_y_scrollbar().child(
        v_flex()
          .w_full()
          .gap(px(4.))
          .child(
            div()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child("Reserved *.kubernetes.io labels are read-only."),
          )
          .children(rows)
          .child(add_row),
      ),
    )
  }

  fn render_events(&self, node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.docker_state.read(cx);
    let events: Vec<_> = state
      .events
      .iter()
      .filter(|e| e.object_kind == "Node" && e.object_name == node.name)
      .cloned()
      .collect();

    if events.is_empty() {
      return div().size_full().flex().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No events for this node"),
      );
    }

    let rows = events
      .iter()
      .map(|e| {
        let color = if e.event_type == "Warning" {
          colors.warning
        } else {
          colors.muted_foreground
        };
        v_flex()
          .w_full()
          .gap(px(2.))
          .py(px(8.))
          .border_b_1()
          .border_color(colors.border)
          .child(
            h_flex()
              .w_full()
              .gap(px(8.))
              .items_center()
              .child(
                div()
                  .px(px(6.))
                  .py(px(1.))
                  .rounded(px(4.))
                  .bg(color.opacity(0.15))
                  .text_xs()
                  .text_color(color)
                  .child(e.reason.clone()),
              )
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(format!("{} · x{}", e.age, e.count)),
              ),
          )
          .child(div().text_sm().text_color(colors.foreground).child(e.message.clone()))
      })
      .collect::<Vec<_>>();

    div().size_full().child(
      div()
        .w_full()
        .h_full()
        .p(px(16.))
        .overflow_y_scrollbar()
        .child(v_flex().w_full().children(rows)),
    )
  }

  fn render_yaml(&self, node: &NodeInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    if self.yaml_content.is_empty() {
      return v_flex().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading YAML..."),
      );
    }
    let name = node.name.clone();
    let editor_for_apply = self.yaml_editor.clone();
    let toolbar = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(6.))
      .gap(px(6.))
      .items_center()
      .justify_between()
      .border_b_1()
      .border_color(colors.border)
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Edit and Apply, or reload from cluster."),
      )
      .child(
        h_flex()
          .gap(px(6.))
          .child(
            Button::new("node-yaml-apply")
              .label("Apply")
              .primary()
              .small()
              .on_click({
                let name = name.clone();
                move |_ev, _w, cx| {
                  let Some(ref e) = editor_for_apply else { return };
                  let yaml = e.read(cx).text().to_string();
                  if !yaml.trim().is_empty() {
                    services::apply_node_yaml(name.clone(), yaml, cx);
                  }
                }
              }),
          )
          .child(
            Button::new("node-yaml-reload")
              .label("Reload")
              .ghost()
              .small()
              .on_click(move |_ev, _w, cx| {
                services::get_node_yaml(name.clone(), cx);
              }),
          ),
      );

    if let Some(ref editor) = self.yaml_editor {
      return v_flex().size_full().child(toolbar).child(
        div()
          .flex_1()
          .min_h_0()
          .child(Input::new(editor).size_full().appearance(false)),
      );
    }

    v_flex().size_full().child(toolbar).child(
      div().flex_1().min_h_0().child(
        div()
          .size_full()
          .overflow_y_scrollbar()
          .bg(colors.sidebar)
          .p(px(12.))
          .font_family("monospace")
          .text_xs()
          .text_color(colors.foreground)
          .child(self.yaml_content.clone()),
      ),
    )
  }

  fn render_empty(cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    div().size_full().flex().items_center().justify_center().child(
      v_flex()
        .items_center()
        .gap(px(16.))
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
                .size(px(48.))
                .text_color(colors.muted_foreground),
            ),
        )
        .child(
          div()
            .text_lg()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.secondary_foreground)
            .child("Select a Node"),
        ),
    )
  }
}

impl Render for NodeDetail {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    if self.yaml_editor.is_none() && self.node.is_some() {
      self.yaml_editor = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .code_editor("yaml")
          .line_number(true)
          .searchable(true)
          .soft_wrap(false)
      }));
    }
    if self.taint_key.is_none() {
      self.taint_key = Some(cx.new(|cx| InputState::new(window, cx).placeholder("key")));
      self.taint_value = Some(cx.new(|cx| InputState::new(window, cx).placeholder("value (optional)")));
      self.taint_effect = Some(cx.new(|cx| InputState::new(window, cx).placeholder("NoSchedule")));
      self.label_key = Some(cx.new(|cx| InputState::new(window, cx).placeholder("key")));
      self.label_value = Some(cx.new(|cx| InputState::new(window, cx).placeholder("value")));
    }

    if let Some(ref editor) = self.yaml_editor
      && !self.yaml_content.is_empty()
      && self.last_synced_yaml != self.yaml_content
    {
      let yaml_clone = self.yaml_content.clone();
      editor.update(cx, |state, cx| {
        state.set_value(yaml_clone.clone(), window, cx);
      });
      self.last_synced_yaml = self.yaml_content.clone();
    }

    let Some(node) = self.node.clone() else {
      return div().size_full().child(Self::render_empty(cx));
    };

    let active = self.active_tab;
    let make_tab = |tab: NodeDetailTab, cx: &mut Context<'_, Self>| {
      Tab::new()
        .label(tab.label())
        .selected(active == tab)
        .on_click(cx.listener(move |this, _ev, _w, cx| {
          this.active_tab = tab;
          if tab == NodeDetailTab::Yaml
            && let Some(ref n) = this.node
          {
            services::get_node_yaml(n.name.clone(), cx);
          }
          cx.notify();
        }))
    };

    let tab_bar = TabBar::new("node-tabs")
      .flex_1()
      .py(px(0.))
      .child(make_tab(NodeDetailTab::Info, cx))
      .child(make_tab(NodeDetailTab::Pods, cx))
      .child(make_tab(NodeDetailTab::Conditions, cx))
      .child(make_tab(NodeDetailTab::Taints, cx))
      .child(make_tab(NodeDetailTab::Labels, cx))
      .child(make_tab(NodeDetailTab::Yaml, cx))
      .child(make_tab(NodeDetailTab::Events, cx));

    let content = match active {
      NodeDetailTab::Info => Self::render_info(&node, cx),
      NodeDetailTab::Pods => self.render_pods(&node, cx),
      NodeDetailTab::Conditions => Self::render_conditions(&node, cx),
      NodeDetailTab::Taints => self.render_taints(&node, cx),
      NodeDetailTab::Labels => self.render_labels(&node, cx),
      NodeDetailTab::Yaml => self.render_yaml(&node, cx),
      NodeDetailTab::Events => self.render_events(&node, cx),
    };

    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      .child(div().w_full().flex_shrink_0().child(tab_bar))
      .child(div().flex_1().min_h_0().overflow_hidden().child(content))
  }
}
