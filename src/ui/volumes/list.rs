use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
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
use crate::docker::VolumeInfo;
use crate::services;
use crate::state::{DockerState, LoadState, Selection, StateChanged, docker_state};
use crate::ui::components::{render_error, render_loading};

/// Volume list events emitted to parent
pub enum VolumeListEvent {
  Selected(Box<VolumeInfo>),
  NewVolume,
}

/// Delegate for the volume list
pub struct VolumeListDelegate {
  docker_state: Entity<DockerState>,
  search_query: String,
}

impl VolumeListDelegate {
  fn volumes<'a>(&self, cx: &'a App) -> &'a Vec<VolumeInfo> {
    &self.docker_state.read(cx).volumes
  }

  fn filtered_volumes(&self, cx: &App) -> Vec<VolumeInfo> {
    let volumes = self.volumes(cx);
    if self.search_query.is_empty() {
      return volumes.clone();
    }

    let query = self.search_query.to_lowercase();
    volumes
      .iter()
      .filter(|v| {
        v.name.to_lowercase().contains(&query)
          || v.driver.to_lowercase().contains(&query)
          || v.mountpoint.to_lowercase().contains(&query)
      })
      .cloned()
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
  }
}

impl ListDelegate for VolumeListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered_volumes(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let volumes = self.filtered_volumes(cx);
    let volume = volumes.get(ix.row)?;
    let colors = &cx.theme().colors;

    // Use global selection as single source of truth
    let global_selection = &self.docker_state.read(cx).selection;
    let is_selected = matches!(global_selection, Selection::Volume(name) if *name == volume.name);
    let is_in_use = volume.is_in_use();
    let volume_name = volume.name.clone();

    let icon_bg = if is_in_use {
      colors.primary
    } else {
      colors.muted_foreground
    };

    let size_text = volume.display_size();

    // Delete button
    let name = volume_name.clone();
    let row = ix.row;

    let delete_button = Button::new(("delete", row))
      .icon(Icon::new(AppIcon::Trash))
      .ghost()
      .xsmall()
      .on_click(move |_ev, _window, cx| {
        services::delete_volume(name.clone(), cx);
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
          .bg(icon_bg)
          .flex()
          .items_center()
          .justify_center()
          .child(Icon::new(AppIcon::Volume).text_color(colors.background)),
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
              .whitespace_nowrap()
              .child(Label::new(volume.name.clone())),
          )
          .child(
            div()
              .w_full()
              .overflow_hidden()
              .text_ellipsis()
              .whitespace_nowrap()
              .text_xs()
              .text_color(colors.muted_foreground)
              .child(size_text),
          ),
      )
      .child(div().flex_shrink_0().child(delete_button));

    let item = ListItem::new(ix)
      .py(px(6.))
      .rounded(px(6.))
      .overflow_hidden()
      .selected(is_selected)
      .child(item_content);

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

/// Self-contained volume list component
pub struct VolumeList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<VolumeListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl VolumeList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = VolumeListDelegate {
      docker_state: docker_state.clone(),
      search_query: String::new(),
    };

    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    // Subscribe to list events
    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered_volumes(cx);
        if let Some(volume) = filtered.get(ix.row) {
          cx.emit(VolumeListEvent::Selected(Box::new(volume.clone())));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    // Subscribe to docker state changes to refresh list
    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::VolumesUpdated | StateChanged::SelectionChanged) {
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search volumes..."));
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
            Icon::new(AppIcon::Volume)
              .size(px(32.))
              .text_color(colors.muted_foreground),
          ),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child("No Volumes"),
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
                  .child("docker volume create mydata"),
              )
              .child(
                Button::new("copy-example")
                  .icon(IconName::Copy)
                  .ghost()
                  .xsmall()
                  .on_click(|_ev, _window, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                      "docker volume create mydata".to_string(),
                    ));
                  }),
              ),
          ),
      )
  }

  fn calculate_total_size(&self, cx: &App) -> String {
    let state = self.docker_state.read(cx);
    let total: i64 = state
      .volumes
      .iter()
      .filter_map(|v| v.usage_data.as_ref().map(|u| u.size))
      .sum();
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
          .child(format!("No volumes match \"{}\"", self.search_query)),
      )
  }
}

impl gpui::EventEmitter<VolumeListEvent> for VolumeList {}

impl Render for VolumeList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.volumes.len();
    let volumes_state = state.volumes_state.clone();
    let total_size = self.calculate_total_size(cx);
    let colors = cx.theme().colors;

    // Get filtered count
    let filtered_count = self.list_state.read(cx).delegate().filtered_volumes(cx).len();
    let is_filtering = !self.search_query.is_empty();
    let volumes_empty = filtered_count == 0;

    let search_visible = self.search_visible;

    // Ensure search input exists if visible and sync query
    if search_visible {
      self.ensure_search_input(window, cx);
      self.sync_search_query(cx);
    }

    let subtitle = match &volumes_state {
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
          .child(Label::new("Volumes"))
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
                cx.emit(VolumeListEvent::NewVolume);
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

    let content: gpui::Div = match &volumes_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("volumes", cx),
      LoadState::Error(e) => {
        let error_msg = e.clone();
        render_error(
          "volumes",
          &error_msg,
          |_ev, _window, cx| {
            services::refresh_volumes(cx);
          },
          cx,
        )
      }
      LoadState::Loaded => {
        if volumes_empty && !is_filtering {
          Self::render_empty(cx)
        } else if volumes_empty && is_filtering {
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
          .id("volume-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
