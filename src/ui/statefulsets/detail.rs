use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  menu::{DropdownMenu, PopupMenuItem},
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::kubernetes::{PodInfo, StatefulSetInfo};
use crate::services;
use crate::state::{DockerState, StateChanged, StatefulSetDetailTab, docker_state};

pub struct StatefulSetDetail {
  docker_state: Entity<DockerState>,
  item: Option<StatefulSetInfo>,
  active_tab: StatefulSetDetailTab,
  yaml_content: String,
  yaml_editor: Option<Entity<InputState>>,
  last_synced_yaml: String,
}

impl StatefulSetDetail {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |this, ds, event: &StateChanged, cx| match event {
      StateChanged::StatefulSetYamlLoaded { name, namespace, yaml } => {
        if let Some(ref s) = this.item
          && s.name == *name
          && s.namespace == *namespace
        {
          yaml.clone_into(&mut this.yaml_content);
          cx.notify();
        }
      }
      StateChanged::StatefulSetTabRequest { name, namespace, tab } => {
        let state = ds.read(cx);
        if let Some(s) = state.get_statefulset(name, namespace) {
          this.item = Some(s.clone());
          this.active_tab = *tab;
          this.yaml_content.clear();
          cx.notify();
        }
      }
      StateChanged::StatefulSetsUpdated => {
        if let Some(ref current) = this.item {
          let state = ds.read(cx);
          if let Some(updated) = state.get_statefulset(&current.name, &current.namespace) {
            this.item = Some(updated.clone());
            cx.notify();
          }
        }
      }
      _ => {}
    })
    .detach();

    Self {
      docker_state,
      item: None,
      active_tab: StatefulSetDetailTab::Info,
      yaml_content: String::new(),
      yaml_editor: None,
      last_synced_yaml: String::new(),
    }
  }

  pub fn set_item(&mut self, item: StatefulSetInfo, cx: &mut Context<'_, Self>) {
    self.item = Some(item.clone());
    self.active_tab = StatefulSetDetailTab::Info;
    self.yaml_content.clear();
    self.yaml_editor = None;
    self.last_synced_yaml.clear();

    services::get_statefulset_yaml(item.name, item.namespace, cx);
    cx.notify();
  }

  pub fn update_item(&mut self, item: StatefulSetInfo, cx: &mut Context<'_, Self>) {
    self.item = Some(item);
    cx.notify();
  }

  fn render_info_tab(item: &StatefulSetInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let info_row = |label: &str, value: String| {
      h_flex()
        .w_full()
        .py(px(8.))
        .gap(px(16.))
        .child(
          div()
            .w(px(140.))
            .flex_shrink_0()
            .text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.muted_foreground)
            .child(label.to_string()),
        )
        .child(div().flex_1().text_sm().text_color(colors.foreground).child(value))
    };

    let is_healthy = item.ready_replicas == item.replicas && item.replicas > 0;
    let status_color = if is_healthy {
      colors.success
    } else if item.ready_replicas > 0 {
      colors.warning
    } else {
      colors.danger
    };

    let mut content = v_flex()
      .w_full()
      .gap(px(4.))
      .child(info_row("Name", item.name.clone()))
      .child(info_row("Namespace", item.namespace.clone()))
      .child(info_row("Age", item.age.clone()));

    content = content.child(
      v_flex()
        .w_full()
        .mt(px(16.))
        .gap(px(8.))
        .child(
          div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.foreground)
            .child("Replicas"),
        )
        .child(
          h_flex()
            .w_full()
            .gap(px(16.))
            .child(
              v_flex()
                .items_center()
                .p(px(16.))
                .rounded(px(8.))
                .bg(colors.sidebar)
                .child(
                  div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(status_color)
                    .child(item.ready_replicas.to_string()),
                )
                .child(div().text_xs().text_color(colors.muted_foreground).child("Ready")),
            )
            .child(
              v_flex()
                .items_center()
                .p(px(16.))
                .rounded(px(8.))
                .bg(colors.sidebar)
                .child(
                  div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.foreground)
                    .child(item.replicas.to_string()),
                )
                .child(div().text_xs().text_color(colors.muted_foreground).child("Desired")),
            ),
        ),
    );

    if !item.images.is_empty() {
      content = content.child(
        v_flex()
          .w_full()
          .mt(px(16.))
          .gap(px(8.))
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::SEMIBOLD)
              .text_color(colors.foreground)
              .child("Images"),
          )
          .child(
            div().w_full().p(px(12.)).rounded(px(8.)).bg(colors.sidebar).child(
              v_flex().gap(px(4.)).children(
                item
                  .images
                  .iter()
                  .map(|img| {
                    div()
                      .text_xs()
                      .font_family("monospace")
                      .text_color(colors.foreground)
                      .child(img.clone())
                  })
                  .collect::<Vec<_>>(),
              ),
            ),
          ),
      );
    }

    if !item.labels.is_empty() {
      content = content.child(
        v_flex()
          .w_full()
          .mt(px(16.))
          .gap(px(8.))
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::SEMIBOLD)
              .text_color(colors.foreground)
              .child("Labels"),
          )
          .child(
            div().w_full().p(px(12.)).rounded(px(8.)).bg(colors.sidebar).child(
              v_flex().gap(px(4.)).children(
                item
                  .labels
                  .iter()
                  .map(|(k, v)| {
                    div()
                      .text_xs()
                      .font_family("monospace")
                      .text_color(colors.muted_foreground)
                      .child(format!("{k}={v}"))
                  })
                  .collect::<Vec<_>>(),
              ),
            ),
          ),
      );
    }

    div()
      .size_full()
      .child(div().w_full().h_full().p(px(16.)).overflow_y_scrollbar().child(content))
  }

  fn render_pods_tab(&self, item: &StatefulSetInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.docker_state.read(cx);
    let matching: Vec<&PodInfo> = state
      .pods
      .iter()
      .filter(|p| {
        if p.namespace != item.namespace {
          return false;
        }
        item
          .labels
          .iter()
          .any(|(k, v)| p.labels.get(k).is_some_and(|pv| pv == v))
      })
      .collect();

    if matching.is_empty() {
      return div().size_full().flex().items_center().justify_center().child(
        v_flex()
          .items_center()
          .gap(px(8.))
          .child(
            Icon::new(AppIcon::Pod)
              .size(px(32.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child("No pods found"),
          ),
      );
    }

    let header = h_flex()
      .w_full()
      .py(px(8.))
      .px(px(12.))
      .gap(px(8.))
      .bg(colors.sidebar)
      .rounded_t(px(8.))
      .child(
        div()
          .flex_1()
          .min_w_0()
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Pod Name"),
      )
      .child(
        div()
          .w(px(80.))
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Status"),
      )
      .child(
        div()
          .w(px(50.))
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Ready"),
      )
      .child(
        div()
          .w(px(60.))
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Restarts"),
      )
      .child(
        div()
          .w(px(50.))
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Age"),
      )
      .child(div().w(px(40.)));

    let rows = matching
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
        let pod_name = pod.name.clone();
        let pod_namespace = pod.namespace.clone();

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
              .text_color(colors.foreground)
              .font_family("monospace")
              .text_ellipsis()
              .overflow_hidden()
              .whitespace_nowrap()
              .child(pod.name.clone()),
          )
          .child(
            div().w(px(80.)).child(
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
              .w(px(50.))
              .text_sm()
              .text_color(colors.foreground)
              .child(pod.ready.clone()),
          )
          .child(
            div()
              .w(px(60.))
              .text_sm()
              .text_color(if pod.restarts > 0 {
                colors.warning
              } else {
                colors.muted_foreground
              })
              .child(pod.restarts.to_string()),
          )
          .child(
            div()
              .w(px(50.))
              .text_sm()
              .text_color(colors.muted_foreground)
              .child(pod.age.clone()),
          )
          .child(
            div().w(px(40.)).flex().justify_end().child(
              Button::new(("view-pod", i))
                .icon(IconName::Eye)
                .ghost()
                .xsmall()
                .on_click(move |_ev, _w, cx| {
                  services::open_pod_info(pod_name.clone(), pod_namespace.clone(), cx);
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
            .child(format!("{} pod(s)", matching.len())),
        )
        .child(v_flex().w_full().child(header).children(rows)),
    )
  }

  fn render_yaml_tab(&self, item: &StatefulSetInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    if self.yaml_content.is_empty() {
      return v_flex().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading YAML..."),
      );
    }

    let name = item.name.clone();
    let namespace = item.namespace.clone();
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
          .child("Edit YAML and Apply, or use the menu for rollout actions."),
      )
      .child(
        Button::new("yaml-actions")
          .icon(IconName::Ellipsis)
          .ghost()
          .compact()
          .dropdown_menu({
            let name = name.clone();
            let namespace = namespace.clone();
            let editor = editor_for_apply.clone();
            move |menu, _w, _cx| {
              let apply_name = name.clone();
              let apply_namespace = namespace.clone();
              let apply_editor = editor.clone();
              let restart_name = name.clone();
              let restart_namespace = namespace.clone();
              let reload_name = name.clone();
              let reload_namespace = namespace.clone();
              menu
                .item(
                  PopupMenuItem::new("Apply YAML")
                    .icon(Icon::new(AppIcon::Refresh))
                    .on_click(move |_, _, cx| {
                      let Some(ref e) = apply_editor else { return };
                      let yaml: String = e.read(cx).text().to_string();
                      if !yaml.trim().is_empty() {
                        services::apply_statefulset_yaml(apply_name.clone(), apply_namespace.clone(), yaml, cx);
                      }
                    }),
                )
                .item(
                  PopupMenuItem::new("Rolling Restart")
                    .icon(Icon::new(AppIcon::Restart))
                    .on_click(move |_, _, cx| {
                      services::rollout_restart_statefulset(restart_name.clone(), restart_namespace.clone(), cx);
                    }),
                )
                .separator()
                .item(PopupMenuItem::new("Reload from Cluster").on_click(move |_, _, cx| {
                  services::get_statefulset_yaml(reload_name.clone(), reload_namespace.clone(), cx);
                }))
            }
          }),
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
              Icon::new(AppIcon::Deployment)
                .size(px(48.))
                .text_color(colors.muted_foreground),
            ),
        )
        .child(
          div()
            .text_lg()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.secondary_foreground)
            .child("Select a StatefulSet"),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Click on a statefulset to view details"),
        ),
    )
  }
}

impl Render for StatefulSetDetail {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    if self.yaml_editor.is_none() && self.item.is_some() {
      self.yaml_editor = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .code_editor("yaml")
          .line_number(true)
          .searchable(true)
          .soft_wrap(false)
      }));
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

    let Some(item) = self.item.clone() else {
      return div().size_full().child(Self::render_empty(cx));
    };

    let active_tab = self.active_tab;

    let tab_bar = TabBar::new("statefulset-tabs")
      .flex_1()
      .py(px(0.))
      .child(
        Tab::new()
          .label("Info")
          .selected(active_tab == StatefulSetDetailTab::Info)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = StatefulSetDetailTab::Info;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Pods")
          .selected(active_tab == StatefulSetDetailTab::Pods)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = StatefulSetDetailTab::Pods;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("YAML")
          .selected(active_tab == StatefulSetDetailTab::Yaml)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = StatefulSetDetailTab::Yaml;
            if let Some(ref item) = this.item {
              services::get_statefulset_yaml(item.name.clone(), item.namespace.clone(), cx);
            }
          })),
      );

    let content = match active_tab {
      StatefulSetDetailTab::Info => Self::render_info_tab(&item, cx),
      StatefulSetDetailTab::Pods => self.render_pods_tab(&item, cx),
      StatefulSetDetailTab::Yaml => self.render_yaml_tab(&item, cx),
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
