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
use crate::kubernetes::{DeploymentInfo, PodInfo};
use crate::services;
use crate::state::{DeploymentDetailTab, DockerState, StateChanged, docker_state};

/// Detail view for a deployment with tabs
pub struct DeploymentDetail {
  docker_state: Entity<DockerState>,
  deployment: Option<DeploymentInfo>,
  active_tab: DeploymentDetailTab,
  yaml_content: String,
  yaml_editor: Option<Entity<InputState>>,
  last_synced_yaml: String,
}

impl DeploymentDetail {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Subscribe to state changes
    cx.subscribe(&docker_state, |this, ds, event: &StateChanged, cx| {
      match event {
        StateChanged::DeploymentYamlLoaded {
          deployment_name,
          namespace,
          yaml,
        } => {
          if let Some(ref dep) = this.deployment
            && dep.name == *deployment_name
            && dep.namespace == *namespace
          {
            yaml.clone_into(&mut this.yaml_content);
            cx.notify();
          }
        }
        StateChanged::DeploymentTabRequest {
          deployment_name,
          namespace,
          tab,
        } => {
          // Find and select the deployment
          let state = ds.read(cx);
          if let Some(dep) = state.get_deployment(deployment_name, namespace) {
            this.deployment = Some(dep.clone());
            this.active_tab = *tab;
            this.yaml_content.clear();
            cx.notify();
          }
        }
        StateChanged::DeploymentsUpdated => {
          // Refresh current deployment if still exists
          if let Some(ref current) = this.deployment {
            let state = ds.read(cx);
            if let Some(updated) = state.get_deployment(&current.name, &current.namespace) {
              this.deployment = Some(updated.clone());
              cx.notify();
            }
          }
        }
        _ => {}
      }
    })
    .detach();

    Self {
      docker_state,
      deployment: None,
      active_tab: DeploymentDetailTab::Info,
      yaml_content: String::new(),
      yaml_editor: None,
      last_synced_yaml: String::new(),
    }
  }

  pub fn set_deployment(&mut self, deployment: DeploymentInfo, cx: &mut Context<'_, Self>) {
    self.deployment = Some(deployment.clone());
    self.active_tab = DeploymentDetailTab::Info;
    self.yaml_content.clear();
    self.yaml_editor = None;
    self.last_synced_yaml.clear();

    // Load YAML
    services::get_deployment_yaml(deployment.name, deployment.namespace, cx);

    cx.notify();
  }

  /// Update deployment data without resetting tab state (for data refresh)
  pub fn update_deployment_data(&mut self, deployment: DeploymentInfo, cx: &mut Context<'_, Self>) {
    self.deployment = Some(deployment);
    cx.notify();
  }

  fn render_info_tab(deployment: &DeploymentInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
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

    let is_healthy = deployment.ready_replicas == deployment.replicas && deployment.replicas > 0;
    let status_color = if is_healthy {
      colors.success
    } else if deployment.ready_replicas > 0 {
      colors.warning
    } else {
      colors.danger
    };

    let mut content = v_flex()
      .w_full()
      .gap(px(4.))
      .child(info_row("Name", deployment.name.clone()))
      .child(info_row("Namespace", deployment.namespace.clone()))
      .child(info_row("Age", deployment.age.clone()));

    // Replica status section
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
                    .child(deployment.ready_replicas.to_string()),
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
                    .child(deployment.replicas.to_string()),
                )
                .child(div().text_xs().text_color(colors.muted_foreground).child("Desired")),
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
                    .text_color(colors.primary)
                    .child(deployment.updated_replicas.to_string()),
                )
                .child(div().text_xs().text_color(colors.muted_foreground).child("Updated")),
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
                    .text_color(colors.success)
                    .child(deployment.available_replicas.to_string()),
                )
                .child(div().text_xs().text_color(colors.muted_foreground).child("Available")),
            ),
        ),
    );

    // Images section
    if !deployment.images.is_empty() {
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
                deployment
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

    // Labels section
    if !deployment.labels.is_empty() {
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
                deployment
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

  fn render_pods_tab(&self, deployment: &DeploymentInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;

    // Get pods that are owned by this deployment
    // Pods are matched by the deployment's labels (which become the pod template labels)
    let state = self.docker_state.read(cx);
    let matching_pods: Vec<&PodInfo> = state
      .pods
      .iter()
      .filter(|pod| {
        // Pod must be in same namespace
        if pod.namespace != deployment.namespace {
          return false;
        }
        // Check if pod has owner reference matching deployment name
        // Or match by app label (common pattern)
        deployment
          .labels
          .iter()
          .any(|(key, value)| pod.labels.get(key).is_some_and(|v| v == value))
      })
      .collect();

    if matching_pods.is_empty() {
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
          .flex_shrink_0()
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Status"),
      )
      .child(
        div()
          .w(px(50.))
          .flex_shrink_0()
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Ready"),
      )
      .child(
        div()
          .w(px(60.))
          .flex_shrink_0()
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Restarts"),
      )
      .child(
        div()
          .w(px(50.))
          .flex_shrink_0()
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .child("Age"),
      )
      .child(
        div()
          .w(px(40.))
          .flex_shrink_0()
          .text_xs()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.muted_foreground)
          .text_right(),
      );

    let rows = matching_pods
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
            div().w(px(80.)).flex_shrink_0().child(
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
              .flex_shrink_0()
              .text_sm()
              .text_color(colors.foreground)
              .child(pod.ready.clone()),
          )
          .child(
            div()
              .w(px(60.))
              .flex_shrink_0()
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
              .flex_shrink_0()
              .text_sm()
              .text_color(colors.muted_foreground)
              .child(pod.age.clone()),
          )
          .child(
            div().w(px(40.)).flex_shrink_0().flex().justify_end().child(
              Button::new(("view-pod", i))
                .icon(IconName::Eye)
                .ghost()
                .xsmall()
                .on_click(move |_ev, _window, cx| {
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
            .child(format!("{} pod(s)", matching_pods.len())),
        )
        .child(v_flex().w_full().child(header).children(rows)),
    )
  }

  fn render_yaml_tab(&self, _deployment: &DeploymentInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;

    if self.yaml_content.is_empty() {
      return v_flex().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading YAML..."),
      );
    }

    if let Some(ref editor) = self.yaml_editor {
      return div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true));
    }

    // Fallback to plain text
    div().size_full().child(
      div()
        .size_full()
        .overflow_y_scrollbar()
        .bg(colors.sidebar)
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(self.yaml_content.clone()),
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
            .child("Select a Deployment"),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Click on a deployment to view details"),
        ),
    )
  }
}

impl Render for DeploymentDetail {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Create yaml editor if needed
    if self.yaml_editor.is_none() && self.deployment.is_some() {
      self.yaml_editor = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .code_editor("yaml")
          .line_number(true)
          .searchable(true)
          .soft_wrap(false)
      }));
    }

    // Sync yaml editor content
    if let Some(ref editor) = self.yaml_editor
      && !self.yaml_content.is_empty()
      && self.last_synced_yaml != self.yaml_content
    {
      let yaml_clone = self.yaml_content.clone();
      editor.update(cx, |state, cx| {
        state.replace(&yaml_clone, window, cx);
      });
      self.last_synced_yaml = self.yaml_content.clone();
    }

    let colors = cx.theme().colors;

    let Some(deployment) = self.deployment.clone() else {
      return div().size_full().child(Self::render_empty(cx));
    };

    let active_tab = self.active_tab;

    // Tab bar
    let tab_bar = TabBar::new("deployment-tabs")
      .flex_1()
      .py(px(0.))
      .child(
        Tab::new()
          .label("Info")
          .selected(active_tab == DeploymentDetailTab::Info)
          .on_click(cx.listener(|this, _ev, _window, cx| {
            this.active_tab = DeploymentDetailTab::Info;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Pods")
          .selected(active_tab == DeploymentDetailTab::Pods)
          .on_click(cx.listener(|this, _ev, _window, cx| {
            this.active_tab = DeploymentDetailTab::Pods;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("YAML")
          .selected(active_tab == DeploymentDetailTab::Yaml)
          .on_click(cx.listener(|this, _ev, _window, cx| {
            this.active_tab = DeploymentDetailTab::Yaml;
            if let Some(ref dep) = this.deployment {
              services::get_deployment_yaml(dep.name.clone(), dep.namespace.clone(), cx);
            }
          })),
      );

    // Tab content
    let content = match active_tab {
      DeploymentDetailTab::Info => Self::render_info_tab(&deployment, cx),
      DeploymentDetailTab::Pods => self.render_pods_tab(&deployment, cx),
      DeploymentDetailTab::Yaml => self.render_yaml_tab(&deployment, cx),
    };

    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      .child(
        h_flex()
          .w_full()
          .px(px(16.))
          .py(px(8.))
          .gap(px(12.))
          .items_center()
          .border_b_1()
          .border_color(colors.border)
          .flex_shrink_0()
          .child(tab_bar),
      )
      .child(div().flex_1().min_h_0().overflow_hidden().child(content))
  }
}
