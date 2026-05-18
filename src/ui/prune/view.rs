use gpui::{Context, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Disableable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  scroll::ScrollableElement,
  switch::Switch,
  theme::ActiveTheme,
  v_flex,
};

use crate::docker::PruneResult;
use crate::services;
use crate::ui::components::form_section;

/// Options for prune operation
#[derive(Debug, Clone, Default)]
pub struct PruneOptions {
  // Docker options
  pub prune_containers: bool,
  pub prune_images: bool,
  pub prune_volumes: bool,
  pub prune_networks: bool,
  pub prune_build_cache: bool,
  pub build_cache_all: bool,
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
      && !self.prune_build_cache
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

/// First-class Prune view: pick what to clean up, run it, see the
/// result inline. Same options + service path as the old modal, just
/// reframed as a compact scrollable sidebar view.
pub struct PruneView {
  options: PruneOptions,
  result_display: PruneResultDisplay,
  disk_usage: Option<crate::docker::DiskUsageSummary>,
}

impl PruneView {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let view = Self {
      options: PruneOptions::default(),
      result_display: PruneResultDisplay::default(),
      disk_usage: None,
    };
    Self::refresh_disk_usage(cx);
    view
  }

  fn refresh_disk_usage(cx: &mut Context<'_, Self>) {
    // Skip when the global Tokio runtime hasn't been initialised yet
    // (gpui::test cases that build the view without the service layer).
    let Some(tokio_handle) = services::Tokio::try_runtime_handle() else {
      tracing::debug!("prune: skipping disk-usage refresh — Tokio not initialised");
      return;
    };
    let client = services::docker_client();
    cx.spawn(async move |this, cx| {
      let usage = cx
        .background_executor()
        .spawn(async move {
          tokio_handle.block_on(async {
            let guard = client.read().await;
            match guard.as_ref() {
              Some(c) => c.get_disk_usage().await.ok(),
              None => None,
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        if let Some(u) = usage {
          this.disk_usage = Some(u);
          cx.notify();
        }
      });
    })
    .detach();
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

  fn render_disk_usage(&self, cx: &Context<'_, Self>) -> Option<impl IntoElement> {
    let colors = cx.theme().colors;
    self.disk_usage.as_ref().map(|u| {
      let line = |label: &'static str, count: usize, size: i64, reclaimable: i64| {
        h_flex()
          .w_full()
          .py(px(3.))
          .gap(px(8.))
          .items_baseline()
          .child(
            div()
              .w(px(110.))
              .flex_shrink_0()
              .text_xs()
              .whitespace_nowrap()
              .text_color(colors.muted_foreground)
              .child(label),
          )
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_xs()
              .text_color(colors.foreground)
              .child(format!(
                "{count} · {} · {} reclaimable",
                bytesize::ByteSize(u64::try_from(size).unwrap_or(0)),
                bytesize::ByteSize(u64::try_from(reclaimable).unwrap_or(0))
              )),
          )
      };

      v_flex()
        .w_full()
        .child(line("Images", u.images_count, u.images_size, u.images_reclaimable))
        .child(line(
          "Containers",
          u.containers_count,
          u.containers_size,
          u.containers_reclaimable,
        ))
        .child(line(
          "Local volumes",
          u.volumes_count,
          u.volumes_size,
          u.volumes_reclaimable,
        ))
        .child(line(
          "Build cache",
          u.build_cache_count,
          u.build_cache_size,
          u.build_cache_reclaimable,
        ))
    })
  }

  fn render_result(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let display = &self.result_display;

    if display.is_loading {
      div()
        .w_full()
        .py(px(8.))
        .child(div().text_sm().text_color(colors.link).child("Pruning..."))
    } else if let Some(error) = &display.error {
      div()
        .w_full()
        .p(px(12.))
        .rounded(px(6.))
        .bg(colors.danger.opacity(0.1))
        .child(
          div()
            .text_sm()
            .text_color(colors.danger)
            .child(format!("Error: {error}")),
        )
    } else if let Some(result) = &display.result {
      let mut parts: Vec<String> = Vec::new();
      if !result.containers_deleted.is_empty() {
        parts.push(format!("{} containers", result.containers_deleted.len()));
      }
      if !result.images_deleted.is_empty() {
        parts.push(format!("{} images", result.images_deleted.len()));
      }
      if !result.volumes_deleted.is_empty() {
        parts.push(format!("{} volumes", result.volumes_deleted.len()));
      }
      if !result.networks_deleted.is_empty() {
        parts.push(format!("{} networks", result.networks_deleted.len()));
      }
      if !result.build_cache_deleted.is_empty() {
        parts.push(format!("{} build cache", result.build_cache_deleted.len()));
      }
      if !result.pods_deleted.is_empty() {
        parts.push(format!("{} pods", result.pods_deleted.len()));
      }
      if !result.deployments_deleted.is_empty() {
        parts.push(format!("{} deployments", result.deployments_deleted.len()));
      }
      if !result.services_deleted.is_empty() {
        parts.push(format!("{} services", result.services_deleted.len()));
      }
      let summary = if result.is_empty() {
        "Nothing to prune".to_string()
      } else {
        format!(
          "Removed {} · {} reclaimed",
          parts.join(", "),
          result.display_space_reclaimed()
        )
      };
      v_flex()
        .w_full()
        .p(px(12.))
        .gap(px(4.))
        .rounded(px(6.))
        .bg(colors.sidebar)
        .child(
          div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.success)
            .child("Prune completed"),
        )
        .child(div().text_sm().text_color(colors.secondary_foreground).child(summary))
    } else {
      div()
    }
  }
}

impl Render for PruneView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let o = &self.options;
    let can_prune = !o.is_empty() && !self.result_display.is_loading;
    let is_loading = self.result_display.is_loading;
    let disk_section = self.render_disk_usage(cx);

    // Label + description on the left, switch pinned to the far right
    // of the full-width row.
    let opt = |label: &'static str, desc: &'static str, on: gpui::AnyElement| {
      h_flex()
        .w_full()
        .py(px(8.))
        .gap(px(16.))
        .items_center()
        .justify_between()
        .child(
          v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(1.))
            .child(div().text_sm().text_color(colors.foreground).child(label))
            .child(div().text_xs().text_color(colors.muted_foreground).child(desc)),
        )
        .child(div().flex_shrink_0().child(on))
    };
    // Indented secondary toggle under its parent option.
    let sub = |label: &'static str, on: gpui::AnyElement| {
      h_flex()
        .w_full()
        .py(px(6.))
        .pl(px(20.))
        .gap(px(12.))
        .items_center()
        .justify_between()
        .child(
          div()
            .flex_1()
            .min_w_0()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(label),
        )
        .child(div().flex_shrink_0().child(on))
    };
    let warn = |text: &'static str| {
      div()
        .w_full()
        .pl(px(20.))
        .pb(px(4.))
        .text_xs()
        .text_color(colors.danger)
        .child(text)
    };

    let images_checked = o.prune_images;
    let volumes_checked = o.prune_volumes;
    let build_cache_checked = o.prune_build_cache;
    let k8s_pods_checked = o.prune_k8s_pods;
    let k8s_pods_all = o.prune_k8s_pods_all;
    let k8s_deployments_checked = o.prune_k8s_deployments;

    div()
      .size_full()
      .bg(colors.background)
      .flex()
      .flex_col()
      // Title bar
      .child(
        h_flex()
          .w_full()
          .h(px(56.))
          .px(px(20.))
          .gap(px(16.))
          .items_center()
          .justify_between()
          .border_b_1()
          .border_color(colors.border)
          .child(
            v_flex()
              .gap(px(2.))
              .child(
                Label::new("Prune")
                  .text_color(colors.foreground)
                  .font_weight(gpui::FontWeight::SEMIBOLD),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("Remove unused Docker and Kubernetes resources to free up disk space"),
              ),
          )
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(
                Button::new("prune-refresh")
                  .label("Refresh")
                  .ghost()
                  .small()
                  .on_click(cx.listener(|_this, _ev, _window, cx| {
                    Self::refresh_disk_usage(cx);
                  })),
              )
              .child(
                Button::new("prune-run")
                  .label(if is_loading { "Pruning..." } else { "Prune" })
                  .primary()
                  .disabled(!can_prune)
                  .on_click(cx.listener(|this, _ev, _window, cx| {
                    let options = this.get_options();
                    if options.is_empty() || this.result_display.is_loading {
                      return;
                    }
                    this.set_loading(true);
                    services::prune_docker(cx.entity(), &options, cx);
                    cx.notify();
                  })),
              ),
          ),
      )
      // Scrollable content
      .child(
        div()
          .id("prune-scroll")
          .flex_1()
          .overflow_y_scrollbar()
          .px(px(20.))
          .py(px(16.))
          .child(
            v_flex()
              .w_full()
              .when_some(disk_section, |el, section| {
                el.child(form_section("Disk usage", cx)).child(section)
              })
              .child(form_section("Docker", cx))
              .child(opt(
                "Containers",
                "Remove all stopped containers",
                Switch::new("containers")
                  .checked(o.prune_containers)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_containers = *c;
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .child(opt(
                "Images",
                "Remove unused images",
                Switch::new("images")
                  .checked(images_checked)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_images = *c;
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .when(images_checked, |el| {
                el.child(sub(
                  "Only dangling images (untagged)",
                  Switch::new("dangling-only")
                    .checked(o.images_dangling_only)
                    .on_click(cx.listener(|this, c: &bool, _w, cx| {
                      this.options.images_dangling_only = *c;
                      cx.notify();
                    }))
                    .into_any_element(),
                ))
              })
              .child(opt(
                "Volumes",
                "Remove unused volumes (data loss)",
                Switch::new("volumes")
                  .checked(volumes_checked)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_volumes = *c;
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .when(volumes_checked, |el| {
                el.child(warn("Removing volumes permanently deletes their data."))
              })
              .child(opt(
                "Networks",
                "Remove unused networks",
                Switch::new("networks")
                  .checked(o.prune_networks)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_networks = *c;
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .child(opt(
                "Build cache",
                "Remove BuildKit build cache",
                Switch::new("build-cache")
                  .checked(build_cache_checked)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_build_cache = *c;
                    if !*c {
                      this.options.build_cache_all = false;
                    }
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .when(build_cache_checked, |el| {
                el.child(sub(
                  "Remove ALL build cache (including in-use)",
                  Switch::new("build-cache-all")
                    .checked(o.build_cache_all)
                    .on_click(cx.listener(|this, c: &bool, _w, cx| {
                      this.options.build_cache_all = *c;
                      cx.notify();
                    }))
                    .into_any_element(),
                ))
              })
              .child(form_section("Kubernetes", cx))
              .child(opt(
                "Pods",
                "Remove completed/failed pods",
                Switch::new("k8s-pods")
                  .checked(k8s_pods_checked)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_k8s_pods = *c;
                    if !*c {
                      this.options.prune_k8s_pods_all = false;
                    }
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .when(k8s_pods_checked, |el| {
                el.child(sub(
                  "Delete ALL pods (including running)",
                  Switch::new("k8s-pods-all")
                    .checked(k8s_pods_all)
                    .on_click(cx.listener(|this, c: &bool, _w, cx| {
                      this.options.prune_k8s_pods_all = *c;
                      cx.notify();
                    }))
                    .into_any_element(),
                ))
              })
              .when(k8s_pods_checked && k8s_pods_all, |el| {
                el.child(warn("This deletes ALL pods including running workloads."))
              })
              .child(opt(
                "Deployments",
                "Remove all deployments (and their pods)",
                Switch::new("k8s-deployments")
                  .checked(k8s_deployments_checked)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_k8s_deployments = *c;
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .when(k8s_deployments_checked, |el| {
                el.child(warn("Deleting deployments also deletes their pods."))
              })
              .child(opt(
                "Services",
                "Remove all services",
                Switch::new("k8s-services")
                  .checked(o.prune_k8s_services)
                  .on_click(cx.listener(|this, c: &bool, _w, cx| {
                    this.options.prune_k8s_services = *c;
                    cx.notify();
                  }))
                  .into_any_element(),
              ))
              .child(div().h(px(8.)))
              .child(self.render_result(cx)),
          ),
      )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_prune_options_default() {
    let options = PruneOptions::default();
    assert!(!options.prune_containers);
    assert!(!options.prune_images);
    assert!(!options.prune_volumes);
    assert!(!options.prune_networks);
    assert!(!options.images_dangling_only);
    assert!(!options.prune_k8s_pods);
    assert!(!options.prune_k8s_pods_all);
    assert!(!options.prune_k8s_deployments);
    assert!(!options.prune_k8s_services);
  }

  #[test]
  fn test_prune_options_is_empty() {
    let empty = PruneOptions::default();
    assert!(empty.is_empty());

    let with_containers = PruneOptions {
      prune_containers: true,
      ..Default::default()
    };
    assert!(!with_containers.is_empty());

    let with_images = PruneOptions {
      prune_images: true,
      ..Default::default()
    };
    assert!(!with_images.is_empty());

    let with_volumes = PruneOptions {
      prune_volumes: true,
      ..Default::default()
    };
    assert!(!with_volumes.is_empty());

    let with_networks = PruneOptions {
      prune_networks: true,
      ..Default::default()
    };
    assert!(!with_networks.is_empty());

    let with_k8s_pods = PruneOptions {
      prune_k8s_pods: true,
      ..Default::default()
    };
    assert!(!with_k8s_pods.is_empty());

    let with_k8s_deployments = PruneOptions {
      prune_k8s_deployments: true,
      ..Default::default()
    };
    assert!(!with_k8s_deployments.is_empty());

    let with_k8s_services = PruneOptions {
      prune_k8s_services: true,
      ..Default::default()
    };
    assert!(!with_k8s_services.is_empty());
  }

  #[test]
  fn test_prune_options_multiple_selections() {
    let options = PruneOptions {
      prune_containers: true,
      prune_images: true,
      prune_networks: true,
      images_dangling_only: true,
      ..Default::default()
    };
    assert!(!options.is_empty());
    assert!(options.prune_containers);
    assert!(options.prune_images);
    assert!(options.prune_networks);
    assert!(options.images_dangling_only);
    assert!(!options.prune_volumes);
  }

  #[test]
  fn test_prune_result_display_default() {
    let display = PruneResultDisplay::default();
    assert!(display.result.is_none());
    assert!(!display.is_loading);
    assert!(display.error.is_none());
  }

  #[gpui::test]
  fn test_prune_view_creation(cx: &mut gpui::TestAppContext) {
    let view = cx.new(PruneView::new);

    view.read_with(cx, |view, _| {
      let options = view.get_options();
      assert!(options.is_empty());
      assert!(!view.result_display.is_loading);
      assert!(view.result_display.result.is_none());
      assert!(view.result_display.error.is_none());
    });
  }

  #[gpui::test]
  fn test_prune_view_set_loading(cx: &mut gpui::TestAppContext) {
    let view = cx.new(PruneView::new);

    view.update(cx, |view, _| {
      view.set_loading(true);
    });
    view.read_with(cx, |view, _| {
      assert!(view.result_display.is_loading);
      assert!(view.result_display.result.is_none());
      assert!(view.result_display.error.is_none());
    });

    view.update(cx, |view, _| {
      view.set_loading(false);
    });
    view.read_with(cx, |view, _| {
      assert!(!view.result_display.is_loading);
    });
  }

  #[gpui::test]
  fn test_prune_view_set_result(cx: &mut gpui::TestAppContext) {
    let view = cx.new(PruneView::new);

    let result = PruneResult {
      containers_deleted: vec!["container1".to_string()],
      images_deleted: vec!["image1".to_string(), "image2".to_string()],
      space_reclaimed: 1024 * 1024 * 100,
      ..Default::default()
    };

    view.update(cx, |view, _| {
      view.set_result(result.clone());
    });

    view.read_with(cx, |view, _| {
      assert!(!view.result_display.is_loading);
      assert!(view.result_display.error.is_none());
      let stored = view.result_display.result.as_ref().unwrap();
      assert_eq!(stored.containers_deleted.len(), 1);
      assert_eq!(stored.images_deleted.len(), 2);
      assert_eq!(stored.space_reclaimed, 1024 * 1024 * 100);
    });
  }

  #[gpui::test]
  fn test_prune_view_set_error(cx: &mut gpui::TestAppContext) {
    let view = cx.new(PruneView::new);

    view.update(cx, |view, _| {
      view.set_loading(true);
    });
    view.update(cx, |view, _| {
      view.set_error("Connection refused".to_string());
    });

    view.read_with(cx, |view, _| {
      assert!(!view.result_display.is_loading);
      assert!(view.result_display.result.is_none());
      assert_eq!(view.result_display.error, Some("Connection refused".to_string()));
    });
  }

  #[gpui::test]
  fn test_prune_view_options_mutation(cx: &mut gpui::TestAppContext) {
    let view = cx.new(PruneView::new);

    view.update(cx, |view, _| {
      view.options.prune_containers = true;
      view.options.prune_images = true;
      view.options.images_dangling_only = true;
    });

    view.read_with(cx, |view, _| {
      let options = view.get_options();
      assert!(!options.is_empty());
      assert!(options.prune_containers);
      assert!(options.prune_images);
      assert!(options.images_dangling_only);
      assert!(!options.prune_volumes);
      assert!(!options.prune_networks);
    });
  }

  #[gpui::test]
  fn test_prune_view_loading_clears_previous_state(cx: &mut gpui::TestAppContext) {
    let view = cx.new(PruneView::new);

    view.update(cx, |view, _| {
      view.set_error("Previous error".to_string());
    });
    view.update(cx, |view, _| {
      view.set_loading(true);
    });

    view.read_with(cx, |view, _| {
      assert!(view.result_display.is_loading);
      assert!(view.result_display.error.is_none());
      assert!(view.result_display.result.is_none());
    });
  }
}
