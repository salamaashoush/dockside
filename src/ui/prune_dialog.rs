use gpui::{App, Context, FocusHandle, Focusable, Hsla, ParentElement, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{h_flex, label::Label, scroll::ScrollableElement, switch::Switch, theme::ActiveTheme, v_flex};

use crate::docker::PruneResult;

/// Options for prune operation
#[derive(Debug, Clone, Default)]
pub struct PruneOptions {
  // Docker options
  pub prune_containers: bool,
  pub prune_images: bool,
  pub prune_volumes: bool,
  pub prune_networks: bool,
  pub images_dangling_only: bool,
  // Kubernetes options
  pub prune_k8s_pods: bool,
  pub prune_k8s_pods_all: bool, // If true, delete all pods; if false, only completed/failed
  pub prune_k8s_deployments: bool,
  pub prune_k8s_services: bool,
}

impl PruneOptions {
  pub fn is_empty(&self) -> bool {
    !self.prune_containers
      && !self.prune_images
      && !self.prune_volumes
      && !self.prune_networks
      && !self.prune_k8s_pods
      && !self.prune_k8s_deployments
      && !self.prune_k8s_services
  }
}

/// Result display for prune operations
#[derive(Debug, Clone, Default)]
pub struct PruneResultDisplay {
  pub result: Option<PruneResult>,
  pub is_loading: bool,
  pub error: Option<String>,
}

/// Dialog for prune operations
pub struct PruneDialog {
  focus_handle: FocusHandle,
  options: PruneOptions,
  result_display: PruneResultDisplay,
}

impl PruneDialog {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      options: PruneOptions::default(),
      result_display: PruneResultDisplay::default(),
    }
  }

  pub fn get_options(&self) -> PruneOptions {
    self.options.clone()
  }

  pub fn set_result(&mut self, result: PruneResult) {
    self.result_display.result = Some(result);
    self.result_display.is_loading = false;
    self.result_display.error = None;
  }

  pub fn set_error(&mut self, error: String) {
    self.result_display.error = Some(error);
    self.result_display.is_loading = false;
  }

  pub fn set_loading(&mut self, loading: bool) {
    self.result_display.is_loading = loading;
    if loading {
      self.result_display.result = None;
      self.result_display.error = None;
    }
  }
}

impl Focusable for PruneDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for PruneDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    // Docker options
    let containers_checked = self.options.prune_containers;
    let images_checked = self.options.prune_images;
    let volumes_checked = self.options.prune_volumes;
    let networks_checked = self.options.prune_networks;
    let dangling_only = self.options.images_dangling_only;
    // Kubernetes options
    let k8s_pods_checked = self.options.prune_k8s_pods;
    let k8s_pods_all = self.options.prune_k8s_pods_all;
    let k8s_deployments_checked = self.options.prune_k8s_deployments;
    let k8s_services_checked = self.options.prune_k8s_services;

    // Helper to render form row
    let render_form_row = |label: &'static str,
                           description: &'static str,
                           content: gpui::AnyElement,
                           border: Hsla,
                           fg: Hsla,
                           muted: Hsla| {
      h_flex()
        .w_full()
        .py(px(12.))
        .px(px(16.))
        .justify_between()
        .items_center()
        .border_b_1()
        .border_color(border)
        .child(
          v_flex()
            .gap(px(2.))
            .child(Label::new(label).text_color(fg))
            .child(div().text_xs().text_color(muted).child(description)),
        )
        .child(content)
    };

    // Render result section
    let result_section = {
      let display = &self.result_display;

      if display.is_loading {
        div()
          .w_full()
          .p(px(16.))
          .child(div().text_sm().text_color(colors.link).child("Pruning..."))
      } else if let Some(error) = &display.error {
        div()
          .w_full()
          .p(px(16.))
          .bg(colors.danger.opacity(0.1))
          .rounded(px(4.))
          .child(
            div()
              .text_sm()
              .text_color(colors.danger)
              .child(format!("Error: {error}")),
          )
      } else if let Some(result) = &display.result {
        v_flex()
          .w_full()
          .p(px(16.))
          .gap(px(8.))
          .bg(colors.sidebar)
          .rounded(px(4.))
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::SEMIBOLD)
              .text_color(colors.success)
              .child("Prune completed"),
          )
          .when(!result.containers_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Containers removed: {}", result.containers_deleted.len())),
            )
          })
          .when(!result.images_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Images removed: {}", result.images_deleted.len())),
            )
          })
          .when(!result.volumes_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Volumes removed: {}", result.volumes_deleted.len())),
            )
          })
          .when(!result.networks_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Networks removed: {}", result.networks_deleted.len())),
            )
          })
          .when(!result.pods_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Pods removed: {}", result.pods_deleted.len())),
            )
          })
          .when(!result.deployments_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Deployments removed: {}", result.deployments_deleted.len())),
            )
          })
          .when(!result.services_deleted.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.secondary_foreground)
                .child(format!("Services removed: {}", result.services_deleted.len())),
            )
          })
          .child(
            div()
              .text_sm()
              .text_color(colors.link)
              .child(format!("Space reclaimed: {}", result.display_space_reclaimed())),
          )
          .when(result.is_empty(), |this| {
            this.child(
              div()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("Nothing to prune"),
            )
          })
      } else {
        div()
      }
    };

    v_flex()
            .w_full()
            .max_h(px(500.))
            .overflow_y_scrollbar()
            // Header description
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Select what to clean up. This will remove unused Docker resources to free up disk space."),
            )
            // Containers
            .child(render_form_row(
                "Containers",
                "Remove all stopped containers",
                Switch::new("containers")
                    .checked(containers_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_containers = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Images
            .child(render_form_row(
                "Images",
                "Remove unused images",
                Switch::new("images")
                    .checked(images_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_images = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Images dangling only option (sub-option)
            .when(images_checked, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .py(px(8.))
                        .px(px(32.))
                        .justify_between()
                        .items_center()
                        .border_b_1()
                        .border_color(colors.border)
                        .bg(colors.sidebar)
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("Only dangling images (untagged)"),
                        )
                        .child(
                            Switch::new("dangling-only")
                                .checked(dangling_only)
                                .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                    this.options.images_dangling_only = *checked;
                                    cx.notify();
                                })),
                        ),
                )
            })
            // Volumes
            .child(render_form_row(
                "Volumes",
                "Remove unused volumes (warning: data loss)",
                Switch::new("volumes")
                    .checked(volumes_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_volumes = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Warning for volumes
            .when(volumes_checked, |this| {
                this.child(
                    div()
                        .w_full()
                        .px(px(16.))
                        .py(px(8.))
                        .bg(colors.danger.opacity(0.1))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.danger)
                                .child("Warning: Removing volumes will permanently delete data!"),
                        ),
                )
            })
            // Networks
            .child(render_form_row(
                "Networks",
                "Remove unused networks",
                Switch::new("networks")
                    .checked(networks_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_networks = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Kubernetes section header
            .child(
                div()
                    .w_full()
                    .py(px(8.))
                    .px(px(16.))
                    .mt(px(8.))
                    .bg(colors.sidebar)
                    .child(div().text_xs().text_color(colors.muted_foreground).child("Kubernetes")),
            )
            // Pods
            .child(render_form_row(
                "Pods",
                "Remove completed/failed pods",
                Switch::new("k8s-pods")
                    .checked(k8s_pods_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_k8s_pods = *checked;
                        if !*checked {
                            this.options.prune_k8s_pods_all = false;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Pods "all" sub-option
            .when(k8s_pods_checked, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .py(px(8.))
                        .px(px(32.))
                        .justify_between()
                        .items_center()
                        .border_b_1()
                        .border_color(colors.border)
                        .bg(colors.sidebar)
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("Delete ALL pods (including running)"),
                        )
                        .child(
                            Switch::new("k8s-pods-all")
                                .checked(k8s_pods_all)
                                .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                    this.options.prune_k8s_pods_all = *checked;
                                    cx.notify();
                                })),
                        ),
                )
            })
            // Warning for all pods
            .when(k8s_pods_checked && k8s_pods_all, |this| {
                this.child(
                    div()
                        .w_full()
                        .px(px(16.))
                        .py(px(8.))
                        .bg(colors.danger.opacity(0.1))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.danger)
                                .child("Warning: This will delete ALL pods including running workloads!"),
                        ),
                )
            })
            // Deployments
            .child(render_form_row(
                "Deployments",
                "Remove all deployments (and their pods)",
                Switch::new("k8s-deployments")
                    .checked(k8s_deployments_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_k8s_deployments = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Warning for deployments
            .when(k8s_deployments_checked, |this| {
                this.child(
                    div()
                        .w_full()
                        .px(px(16.))
                        .py(px(8.))
                        .bg(colors.danger.opacity(0.1))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.danger)
                                .child("Warning: Deleting deployments will also delete their pods!"),
                        ),
                )
            })
            // Services
            .child(render_form_row(
                "Services",
                "Remove all services",
                Switch::new("k8s-services")
                    .checked(k8s_services_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_k8s_services = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Result display
            .child(result_section)
  }
}
