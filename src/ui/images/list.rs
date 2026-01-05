use gpui::{App, Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, IndexPath, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  list::{List, ListDelegate, ListEvent, ListItem, ListState},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::docker::ImageInfo;
use crate::services;
use crate::state::{DockerState, LoadState, Selection, StateChanged, docker_state};
use crate::ui::components::{render_error, render_loading};

/// Image list events emitted to parent
pub enum ImageListEvent {
  Selected(Box<ImageInfo>),
  PullImage,
}

/// Delegate for the image list
pub struct ImageListDelegate {
  docker_state: Entity<DockerState>,
  search_query: String,
  /// Cached list: (`section_index`, `is_in_use`, images)
  sections: Vec<(bool, Vec<ImageInfo>)>,
}

impl ImageListDelegate {
  fn rebuild_sections(&mut self, cx: &App) {
    let state = self.docker_state.read(cx);
    let containers = &state.containers;

    // Get all image IDs that are in use by containers
    let in_use_ids: std::collections::HashSet<String> = containers.iter().map(|c| c.image_id.clone()).collect();

    let mut in_use = Vec::new();
    let mut unused = Vec::new();

    // Filter images based on search query
    let query = self.search_query.to_lowercase();
    let images: Vec<&ImageInfo> = if query.is_empty() {
      state.images.iter().collect()
    } else {
      state
        .images
        .iter()
        .filter(|img| {
          img.display_name().to_lowercase().contains(&query)
            || img.id.to_lowercase().contains(&query)
            || img.repo_tags.iter().any(|t| t.to_lowercase().contains(&query))
        })
        .collect()
    };

    for image in images {
      if in_use_ids.contains(&image.id) {
        in_use.push(image.clone());
      } else {
        unused.push(image.clone());
      }
    }

    self.sections.clear();
    if !in_use.is_empty() {
      self.sections.push((true, in_use));
    }
    if !unused.is_empty() {
      self.sections.push((false, unused));
    }
  }

  fn get_image(&self, ix: IndexPath) -> Option<&ImageInfo> {
    self.sections.get(ix.section).and_then(|(_, images)| images.get(ix.row))
  }

  pub fn set_search_query(&mut self, query: String, cx: &App) {
    self.search_query = query;
    self.rebuild_sections(cx);
  }

  pub fn total_filtered_count(&self) -> usize {
    self.sections.iter().map(|(_, imgs)| imgs.len()).sum()
  }
}

impl ListDelegate for ImageListDelegate {
  type Item = ListItem;

  fn sections_count(&self, _cx: &App) -> usize {
    self.sections.len()
  }

  fn items_count(&self, section: usize, _cx: &App) -> usize {
    self.sections.get(section).map_or(0, |(_, imgs)| imgs.len())
  }

  fn render_section_header(
    &mut self,
    section: usize,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<impl IntoElement> {
    let colors = &cx.theme().colors;
    let (is_in_use, _) = self.sections.get(section)?;

    let title = if *is_in_use { "In Use" } else { "Unused" };

    Some(
      div()
        .w_full()
        .px(px(12.))
        .py(px(8.))
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.muted_foreground)
        .child(title),
    )
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let image = self.get_image(ix)?.clone();
    let colors = &cx.theme().colors;

    // Use global selection as single source of truth
    let global_selection = &self.docker_state.read(cx).selection;
    let is_selected = matches!(global_selection, Selection::Image(img) if img.id == image.id);
    let image_id = image.id.clone();

    // Display name (repo:tag or short id)
    let display_name = image.display_name();
    let size_text = image.display_size();

    // Age display
    let age_text = image.created.map_or_else(
      || "Unknown".to_string(),
      |created| {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(created);
        if duration.num_days() > 365 {
          format!("{} years ago", duration.num_days() / 365)
        } else if duration.num_days() > 30 {
          format!("{} months ago", duration.num_days() / 30)
        } else if duration.num_days() > 0 {
          format!("{} days ago", duration.num_days())
        } else if duration.num_hours() > 0 {
          format!("{} hours ago", duration.num_hours())
        } else {
          "just now".to_string()
        }
      },
    );

    // Platform badge
    let platform = image.architecture.as_ref().map(|arch| {
      if arch.contains("arm") || arch.contains("aarch") {
        "arm64"
      } else {
        "amd64"
      }
    });

    let item_content = h_flex()
      .w_full()
      .items_center()
      .gap(px(10.))
      .child(
        div()
          .size(px(36.))
          .flex_shrink_0()
          .rounded(px(8.))
          .bg(colors.primary)
          .flex()
          .items_center()
          .justify_center()
          .child(Icon::new(AppIcon::Image).text_color(colors.background)),
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
              .child(Label::new(display_name)),
          )
          .child(
            h_flex()
              .gap(px(8.))
              .text_xs()
              .text_color(colors.muted_foreground)
              .child(size_text)
              .child(age_text),
          ),
      )
      .when_some(platform, |el, plat| {
        el.child(
          div()
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(colors.primary)
            .text_xs()
            .text_color(colors.background)
            .child(plat),
        )
      });

    let id = image_id.clone();
    let row = ix.row;
    let section = ix.section;

    let item = ListItem::new(ix)
      .py(px(6.))
      .rounded(px(6.))
      .selected(is_selected)
      .child(item_content)
      .suffix(move |_, _| {
        let id = id.clone();
        div()
          .size(px(28.))
          .flex_shrink_0()
          .flex()
          .items_center()
          .justify_center()
          .child(
            Button::new(SharedString::from(format!("delete-{section}-{row}")))
              .icon(Icon::new(AppIcon::Trash))
              .ghost()
              .xsmall()
              .on_click(move |_ev, _window, cx| {
                services::delete_image(id.clone(), cx);
              }),
          )
      });

    Some(item)
  }

  fn set_selected_index(
    &mut self,
    _ix: Option<IndexPath>,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) {
    // Selection is managed globally via DockerState.selection
    cx.notify();
  }
}

/// Self-contained image list component
pub struct ImageList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<ImageListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl ImageList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let mut delegate = ImageListDelegate {
      docker_state: docker_state.clone(),
      search_query: String::new(),
      sections: Vec::new(),
    };

    // Build initial sections
    delegate.rebuild_sections(cx);

    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    // Subscribe to list events
    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        if let Some(image) = delegate.get_image(*ix) {
          cx.emit(ImageListEvent::Selected(Box::new(image.clone())));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    // Subscribe to docker state changes to refresh list
    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::ImagesUpdated | StateChanged::ContainersUpdated | StateChanged::SelectionChanged
      ) {
        this.list_state.update(cx, |state, cx| {
          state.delegate_mut().rebuild_sections(cx);
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search images..."));
      self.search_input = Some(input_state);
    }
  }

  fn sync_search_query(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(input) = &self.search_input {
      let current_text = input.read(cx).text().to_string();
      if current_text != self.search_query {
        current_text.clone_into(&mut self.search_query);
        self.list_state.update(cx, |state, cx| {
          state.delegate_mut().set_search_query(current_text, cx);
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
        state.delegate_mut().set_search_query(String::new(), cx);
        cx.notify();
      });
    }
    cx.notify();
  }

  fn render_empty(cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;

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
            Icon::new(AppIcon::Image)
              .size(px(32.))
              .text_color(colors.muted_foreground),
          ),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child("No Images"),
      )
      .child(
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
                  .child("docker pull nginx:latest"),
              )
              .child(
                Button::new("copy-example")
                  .icon(IconName::Copy)
                  .ghost()
                  .xsmall()
                  .on_click(|_ev, _window, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string("docker pull nginx:latest".to_string()));
                  }),
              ),
          ),
      )
  }

  fn calculate_total_size(&self, cx: &App) -> String {
    let state = self.docker_state.read(cx);
    let total: i64 = state.images.iter().map(|i| i.size).sum();
    bytesize::ByteSize(u64::try_from(total).unwrap_or(0)).to_string()
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
          .child(format!("No images match \"{}\"", self.search_query)),
      )
  }
}

impl gpui::EventEmitter<ImageListEvent> for ImageList {}

impl Render for ImageList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.images.len();
    let images_state = state.images_state.clone();
    let total_size = self.calculate_total_size(cx);
    let colors = cx.theme().colors;

    // Get filtered count
    let filtered_count = self.list_state.read(cx).delegate().total_filtered_count();
    let is_filtering = !self.search_query.is_empty();
    let images_empty = total_count == 0;
    let filtered_empty = filtered_count == 0;

    let search_visible = self.search_visible;

    // Ensure search input exists if visible and sync query
    if search_visible {
      self.ensure_search_input(window, cx);
      self.sync_search_query(cx);
    }

    let subtitle = match &images_state {
      LoadState::NotLoaded | LoadState::Loading => "Loading...".to_string(),
      LoadState::Error(_) => "Error loading".to_string(),
      LoadState::Loaded => {
        if is_filtering {
          format!("{filtered_count} of {total_count} ({total_size} total)")
        } else {
          format!("{total_size} total")
        }
      }
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
          .child(Label::new("Images"))
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
            Button::new("pull")
              .icon(Icon::new(AppIcon::Plus))
              .ghost()
              .compact()
              .on_click(cx.listener(|_this, _ev, _window, cx| {
                cx.emit(ImageListEvent::PullImage);
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

    let content: gpui::Div = match &images_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("images", cx),
      LoadState::Error(e) => {
        let error_msg = e.clone();
        render_error(
          "images",
          &error_msg,
          |_ev, _window, cx| {
            services::refresh_images(cx);
          },
          cx,
        )
      }
      LoadState::Loaded => {
        if images_empty && !is_filtering {
          Self::render_empty(cx)
        } else if filtered_empty && is_filtering {
          self.render_no_results(cx)
        } else {
          div().size_full().p(px(8.)).child(List::new(&self.list_state))
        }
      }
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
          .id("image-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
