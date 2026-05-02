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
use crate::kubernetes::CronJobInfo;
use crate::services;
use crate::state::{DockerState, LoadState, Selection, StateChanged, docker_state};
use crate::ui::components::{render_k8s_error, render_loading};

pub enum CronJobListEvent {
  Selected(CronJobInfo),
}

pub struct CronJobListDelegate {
  docker_state: Entity<DockerState>,
  search_query: String,
}

impl CronJobListDelegate {
  fn items(&self, cx: &App) -> Vec<CronJobInfo> {
    let state = self.docker_state.read(cx);
    if state.selected_namespace == "all" {
      state.cronjobs.clone()
    } else {
      state
        .cronjobs
        .iter()
        .filter(|c| c.namespace == state.selected_namespace)
        .cloned()
        .collect()
    }
  }

  fn filtered(&self, cx: &App) -> Vec<CronJobInfo> {
    let items = self.items(cx);
    if self.search_query.is_empty() {
      return items;
    }
    let q = self.search_query.to_lowercase();
    items
      .into_iter()
      .filter(|c| c.name.to_lowercase().contains(&q) || c.namespace.to_lowercase().contains(&q))
      .collect()
  }

  pub fn set_search_query(&mut self, query: String) {
    self.search_query = query;
  }
}

impl ListDelegate for CronJobListDelegate {
  type Item = ListItem;

  fn items_count(&self, _section: usize, cx: &App) -> usize {
    self.filtered(cx).len()
  }

  fn render_item(
    &mut self,
    ix: IndexPath,
    _window: &mut Window,
    cx: &mut Context<'_, ListState<Self>>,
  ) -> Option<Self::Item> {
    let items = self.filtered(cx);
    let c = items.get(ix.row)?;
    let colors = &cx.theme().colors;

    let global_selection = &self.docker_state.read(cx).selection;
    let is_selected = matches!(global_selection, Selection::CronJob { name, namespace } if *name == c.name && *namespace == c.namespace);
    let name_clone = c.name.clone();
    let ns_clone = c.namespace.clone();
    let suspend = c.suspend;
    let pin_favorite = crate::state::FavoriteRef::CronJob {
      name: c.name.clone(),
      namespace: c.namespace.clone(),
    };
    let pinned = services::is_favorite(&pin_favorite, cx);

    let icon_bg = if c.suspend {
      colors.muted_foreground
    } else if c.active > 0 {
      colors.success
    } else {
      colors.primary
    };
    let subtitle = format!("{} - {}", c.namespace, c.schedule);

    let row = ix.row;
    let menu_button = Button::new(("menu", row))
      .icon(IconName::Ellipsis)
      .ghost()
      .xsmall()
      .dropdown_menu(move |menu, _window, _cx| {
        let name = name_clone.clone();
        let ns = ns_clone.clone();
        let suspend_label = if suspend { "Resume" } else { "Suspend" };
        let new_suspend = !suspend;
        menu
          .item(
            PopupMenuItem::new("Trigger Now")
              .icon(Icon::new(AppIcon::Play))
              .on_click({
                let name = name.clone();
                let ns = ns.clone();
                move |_, _, cx| {
                  services::trigger_cronjob(name.clone(), ns.clone(), cx);
                }
              }),
          )
          .item(
            PopupMenuItem::new(suspend_label)
              .icon(Icon::new(AppIcon::Pause))
              .on_click({
                let name = name.clone();
                let ns = ns.clone();
                move |_, _, cx| {
                  services::set_cronjob_suspend(name.clone(), ns.clone(), new_suspend, cx);
                }
              }),
          )
          .separator()
          .item(PopupMenuItem::new("View YAML").icon(IconName::File).on_click({
            let name = name.clone();
            let ns = ns.clone();
            move |_, _, cx| {
              services::open_cronjob_yaml(name.clone(), ns.clone(), cx);
            }
          }))
          .separator()
          .item(
            PopupMenuItem::new(if pinned {
              "Unpin from Dashboard"
            } else {
              "Pin to Dashboard"
            })
            .icon(IconName::Star)
            .on_click({
              let pin = pin_favorite.clone();
              move |_, _, cx| {
                services::toggle_favorite(pin.clone(), cx);
              }
            }),
          )
          .separator()
          .item(PopupMenuItem::new("Delete").icon(Icon::new(AppIcon::Trash)).on_click({
            let name = name.clone();
            let ns = ns.clone();
            move |_, _, cx| {
              services::delete_cronjob(name.clone(), ns.clone(), cx);
            }
          }))
      });

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
              .child(Icon::new(AppIcon::Pod).text_color(colors.background)),
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
                  .child(c.name.clone()),
              )
              .child(
                div()
                  .text_xs()
                  .font_family("monospace")
                  .text_color(colors.muted_foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(subtitle),
              ),
          )
          .when(c.suspend, |el| {
            el.child(
              div()
                .flex_shrink_0()
                .px(px(8.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(colors.warning.opacity(0.2))
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.warning)
                .child("Suspended"),
            )
          }),
      )
      .child(div().flex_shrink_0().child(menu_button));

    let item = ListItem::new(ix)
      .py(px(6.))
      .rounded(px(6.))
      .overflow_hidden()
      .selected(is_selected)
      .child(item_content);

    Some(item)
  }

  fn set_selected_index(&mut self, _ix: Option<IndexPath>, _w: &mut Window, cx: &mut Context<'_, ListState<Self>>) {
    cx.notify();
  }
}

pub struct CronJobList {
  docker_state: Entity<DockerState>,
  list_state: Entity<ListState<CronJobListDelegate>>,
  search_input: Option<Entity<InputState>>,
  search_visible: bool,
  search_query: String,
}

impl CronJobList {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    let delegate = CronJobListDelegate {
      docker_state: docker_state.clone(),
      search_query: String::new(),
    };
    let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

    cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| match event {
      ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
        let delegate = state.read(cx).delegate();
        let filtered = delegate.filtered(cx);
        if let Some(c) = filtered.get(ix.row) {
          cx.emit(CronJobListEvent::Selected(c.clone()));
        }
      }
      ListEvent::Cancel => {}
    })
    .detach();

    cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
      if matches!(
        event,
        StateChanged::CronJobsUpdated
          | StateChanged::NamespacesUpdated
          | StateChanged::MachinesUpdated
          | StateChanged::SelectionChanged
      ) {
        this.list_state.update(cx, |_state, cx| cx.notify());
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
      let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search cronjobs..."));
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
            Icon::new(AppIcon::Pod)
              .size(px(32.))
              .text_color(colors.muted_foreground),
          ),
      )
      .child(
        div()
          .text_xl()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.secondary_foreground)
          .child("No CronJobs"),
      )
  }
}

impl gpui::EventEmitter<CronJobListEvent> for CronJobList {}

impl Render for CronJobList {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let state = self.docker_state.read(cx);
    let total_count = state.cronjobs.len();
    let load_state = state.cronjobs_state.clone();
    let filtered_count = self.list_state.read(cx).delegate().filtered(cx).len();
    let is_filtering = !self.search_query.is_empty();
    let empty = filtered_count == 0;

    let subtitle = match &load_state {
      LoadState::NotLoaded | LoadState::Loading => "Loading...".to_string(),
      LoadState::Error(_) => "Error loading".to_string(),
      LoadState::Loaded => {
        if is_filtering {
          format!("{filtered_count} of {total_count}")
        } else {
          format!("{total_count} total")
        }
      }
    };

    let colors = cx.theme().colors;
    let search_visible = self.search_visible;
    if search_visible {
      self.ensure_search_input(window, cx);
      self.sync_search_query(cx);
    }

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
          .child(Label::new("CronJobs"))
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
              .on_click(cx.listener(|this, _ev, window, cx| this.toggle_search(window, cx))),
          )
          .child(
            Button::new("cj-toolbar-actions")
              .icon(IconName::Ellipsis)
              .ghost()
              .compact()
              .dropdown_menu(|menu, _, _| {
                menu.item(
                  PopupMenuItem::new("Refresh")
                    .icon(Icon::new(AppIcon::Refresh))
                    .on_click(|_, _, cx| services::refresh_cronjobs(cx)),
                )
              }),
          ),
      );

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
            Icon::new(AppIcon::Search)
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
                .on_click(cx.listener(|this, _ev, window, cx| this.toggle_search(window, cx))),
            )
          }),
      )
    } else {
      None
    };

    let content: gpui::Div = match &load_state {
      LoadState::NotLoaded | LoadState::Loading => render_loading("cronjobs", cx),
      LoadState::Error(e) => render_k8s_error("cronjobs", &e.clone(), |_ev, _w, cx| services::refresh_cronjobs(cx), cx),
      LoadState::Loaded => {
        if empty {
          Self::render_empty(cx)
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
          .id("cronjobs-list-scroll")
          .flex_1()
          .min_h_0()
          .overflow_hidden()
          .child(content),
      )
  }
}
