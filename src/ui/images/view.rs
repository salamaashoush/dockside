use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  theme::ActiveTheme,
};

use crate::docker::ImageInfo;
use crate::services;
use crate::state::{DockerState, ImageInspectData, StateChanged, docker_state};

use super::detail::ImageDetail;
use super::list::{ImageList, ImageListEvent};
use super::pull_dialog::PullImageDialog;

/// Self-contained Images view - handles list, detail, and all state
pub struct ImagesView {
  _docker_state: Entity<DockerState>,
  image_list: Entity<ImageList>,
  selected_image: Option<ImageInfo>,
  inspect_data: Option<ImageInspectData>,
  active_tab: usize,
}

impl ImagesView {
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
          if let Some(ref selected) = this.selected_image {
            let state = state.read(cx);
            if state.images.iter().any(|i| i.id == selected.id) {
              // Update the selected image info
              if let Some(updated) = state.images.iter().find(|i| i.id == selected.id) {
                this.selected_image = Some(updated.clone());
              }
            } else {
              this.selected_image = None;
              this.inspect_data = None;
              this.active_tab = 0;
            }
          }
          cx.notify();
        }
        StateChanged::ImageInspectLoaded { image_id, data } => {
          if let Some(ref selected) = this.selected_image
            && &selected.id == image_id
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
      _docker_state: docker_state,
      image_list,
      selected_image: None,
      inspect_data: None,
      active_tab: 0,
    }
  }

  fn show_pull_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    let dialog_entity = cx.new(PullImageDialog::new);

    window.open_dialog(cx, move |dialog, _window, _cx| {
      let dialog_clone = dialog_entity.clone();

      dialog
        .title("Pull Image")
        .min_w(px(500.))
        .child(dialog_entity.clone())
        .footer(move |_dialog_state, _, _window, _cx| {
          let dialog_for_pull = dialog_clone.clone();

          vec![
            Button::new("pull")
              .label("Pull")
              .primary()
              .on_click({
                let dialog = dialog_for_pull.clone();
                move |_ev, window, cx| {
                  let options = dialog.read(cx).get_options(cx);
                  if !options.image.is_empty() {
                    services::pull_image(options.image, options.platform.as_docker_arg().map(String::from), cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn on_select_image(&mut self, image: &ImageInfo, cx: &mut Context<'_, Self>) {
    self.selected_image = Some(image.clone());
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
    let selected_image = self.selected_image.clone();
    let inspect_data = self.inspect_data.clone();
    let active_tab = self.active_tab;

    // Build detail panel
    let detail = ImageDetail::new()
      .image(selected_image)
      .inspect_data(inspect_data)
      .active_tab(active_tab)
      .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
        this.on_tab_change(*tab, cx);
      }))
      .on_delete(cx.listener(|this, id: &str, _window, cx| {
        services::delete_image(id.to_string(), cx);
        this.selected_image = None;
        this.inspect_data = None;
        this.active_tab = 0;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Image list - fixed width with border
        div()
          .w(px(320.))
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .border_r_1()
          .border_color(colors.border)
          .child(self.image_list.clone()),
      )
      .child(
        // Right: Detail panel - flexible width
        div()
          .flex_1()
          .h_full()
          .overflow_hidden()
          .child(detail.render(window, cx)),
      )
  }
}
