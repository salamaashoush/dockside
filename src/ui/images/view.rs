use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::theme::ActiveTheme;

use crate::docker::ImageInfo;
use crate::services;
use crate::state::{DockerState, ImageInspectData, Selection, StateChanged, docker_state};
use crate::ui::dialogs;

use super::detail::ImageDetail;
use super::list::{ImageList, ImageListEvent};

/// Self-contained Images view - handles list, detail, and all state
pub struct ImagesView {
  docker_state: Entity<DockerState>,
  image_list: Entity<ImageList>,
  // View-specific state (not selection - that's in global DockerState)
  inspect_data: Option<ImageInspectData>,
  active_tab: usize,
}

impl ImagesView {
  /// Get the currently selected image from global state
  fn selected_image(&self, cx: &App) -> Option<ImageInfo> {
    match &self.docker_state.read(cx).selection {
      Selection::Image(i) => Some(i.clone()),
      _ => None,
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Create image list entity
    let image_list = cx.new(|cx| ImageList::new(window, cx));

    // Subscribe to image list events
    cx.subscribe_in(
      &image_list,
      window,
      |this, _list, event: &ImageListEvent, window, cx| match event {
        ImageListEvent::Selected(image) => {
          this.on_select_image(image.as_ref(), cx);
        }
        ImageListEvent::PullImage => {
          Self::show_pull_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to state changes
    cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
      match event {
        StateChanged::ImagesUpdated => {
          // If selected image was deleted, clear selection
          let selected_id = {
            if let Selection::Image(ref img) = this.docker_state.read(cx).selection {
              Some(img.id.clone())
            } else {
              None
            }
          };

          if let Some(id) = selected_id {
            let updated = state.read(cx).images.iter().find(|i| i.id == id).cloned();
            if let Some(image) = updated {
              this.docker_state.update(cx, |s, _| {
                s.set_selection(Selection::Image(image));
              });
            } else {
              this.docker_state.update(cx, |s, _| {
                s.set_selection(Selection::None);
              });
              this.inspect_data = None;
              this.active_tab = 0;
            }
          }
          cx.notify();
        }
        StateChanged::ImageInspectLoaded { image_id, data } => {
          if let Some(selected) = this.selected_image(cx)
            && selected.id == *image_id
          {
            this.inspect_data = Some(data.clone());
            cx.notify();
          }
        }
        _ => {}
      }
    })
    .detach();

    Self {
      docker_state,
      image_list,
      inspect_data: None,
      active_tab: 0,
    }
  }

  fn show_pull_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    dialogs::open_pull_image_dialog(window, cx);
  }

  fn on_select_image(&mut self, image: &ImageInfo, cx: &mut Context<'_, Self>) {
    // Update global selection (single source of truth)
    self.docker_state.update(cx, |state, _cx| {
      state.set_selection(Selection::Image(image.clone()));
    });

    // Reset view-specific state
    self.inspect_data = None;
    self.active_tab = 0;

    // Load inspect data
    services::inspect_image(image.id.clone(), cx);

    cx.notify();
  }

  fn on_tab_change(&mut self, tab: usize, cx: &mut Context<'_, Self>) {
    self.active_tab = tab;
    cx.notify();
  }
}

impl Render for ImagesView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let selected_image = self.selected_image(cx);
    let inspect_data = self.inspect_data.clone();
    let active_tab = self.active_tab;
    let has_selection = selected_image.is_some();

    // Build detail panel
    let detail = ImageDetail::new()
      .image(selected_image)
      .inspect_data(inspect_data)
      .active_tab(active_tab)
      .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
        this.on_tab_change(*tab, cx);
      }))
      .on_delete(cx.listener(|this, _id: &str, _window, cx| {
        // Clear selection in global state
        this.docker_state.update(cx, |s, _| s.set_selection(Selection::None));
        this.inspect_data = None;
        this.active_tab = 0;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Image list - fixed width when selected, full width when not
        div()
          .when(has_selection, |el| {
            el.w(px(320.)).border_r_1().border_color(colors.border)
          })
          .when(!has_selection, gpui::Styled::flex_1)
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .child(self.image_list.clone()),
      )
      .when(has_selection, |el| {
        el.child(
          // Right: Detail panel - only shown when selection exists
          div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .child(detail.render(window, cx)),
        )
      })
  }
}
