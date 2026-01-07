use gpui::{App, Entity, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::InputState,
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::docker::{VolumeFileEntry, VolumeInfo};
use crate::ui::components::{FileExplorer, FileExplorerConfig, FileExplorerState};

type VolumeActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;
type FileNavigateCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type FileSelectCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type CloseViewerCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type SymlinkClickCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// State for volume detail tabs
#[derive(Debug, Clone, Default)]
pub struct VolumeTabState {
  pub current_path: String,
  pub files: Vec<VolumeFileEntry>,
  pub files_loading: bool,
  /// Selected file path for viewing
  pub selected_file: Option<String>,
  /// Content of selected file
  pub file_content: String,
  /// Whether file content is loading
  pub file_content_loading: bool,
}

impl VolumeTabState {
  pub fn new() -> Self {
    Self {
      current_path: "/".to_string(),
      ..Default::default()
    }
  }
}

pub struct VolumeDetail {
  volume: Option<VolumeInfo>,
  active_tab: usize,
  volume_state: Option<VolumeTabState>,
  file_content_editor: Option<Entity<InputState>>,
  on_delete: Option<VolumeActionCallback>,
  on_tab_change: Option<TabChangeCallback>,
  on_navigate_path: Option<FileNavigateCallback>,
  on_file_select: Option<FileSelectCallback>,
  on_close_file_viewer: Option<CloseViewerCallback>,
  on_symlink_click: Option<SymlinkClickCallback>,
}

impl VolumeDetail {
  pub fn new() -> Self {
    Self {
      volume: None,
      active_tab: 0,
      volume_state: None,
      file_content_editor: None,
      on_delete: None,
      on_tab_change: None,
      on_navigate_path: None,
      on_file_select: None,
      on_close_file_viewer: None,
      on_symlink_click: None,
    }
  }

  pub fn volume(mut self, volume: Option<VolumeInfo>) -> Self {
    self.volume = volume;
    self
  }

  pub fn active_tab(mut self, tab: usize) -> Self {
    self.active_tab = tab;
    self
  }

  pub fn volume_state(mut self, state: VolumeTabState) -> Self {
    self.volume_state = Some(state);
    self
  }

  pub fn file_content_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.file_content_editor = editor;
    self
  }

  pub fn on_delete<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_delete = Some(Rc::new(callback));
    self
  }

  pub fn on_tab_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&usize, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  pub fn on_navigate_path<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_navigate_path = Some(Rc::new(callback));
    self
  }

  pub fn on_file_select<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_file_select = Some(Rc::new(callback));
    self
  }

  pub fn on_close_file_viewer<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_close_file_viewer = Some(Rc::new(callback));
    self
  }

  pub fn on_symlink_click<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_symlink_click = Some(Rc::new(callback));
    self
  }

  fn render_empty(cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .items_center()
      .justify_center()
      .child(
        v_flex()
          .items_center()
          .gap(px(16.))
          .child(
            Icon::new(AppIcon::Volume)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select a volume to view details"),
          ),
      )
  }

  fn render_info_tab(volume: &VolumeInfo, cx: &App) -> gpui::Div {
    let _colors = &cx.theme().colors;

    // Basic info rows
    let mut basic_info = vec![("Name", volume.name.clone()), ("Size", volume.display_size())];

    if let Some(created) = volume.created {
      basic_info.insert(1, ("Created", created.format("%Y-%m-%d %H:%M:%S").to_string()));
    }

    v_flex()
            .flex_1()
            .w_full()
            .p(px(16.))
            .gap(px(12.))
            .child(Self::render_section(None, basic_info, cx))
            // Labels section if not empty
            .when(!volume.labels.is_empty(), |el| {
                el.child(Self::render_labels_section(volume, cx))
            })
            // Additional info
            .child(Self::render_section(
                Some("Details"),
                vec![
                    ("Driver", volume.driver.clone()),
                    ("Mountpoint", volume.mountpoint.clone()),
                    ("Scope", volume.scope.clone()),
                ],
                cx,
            ))
  }

  fn render_section(header: Option<&str>, rows: Vec<(&str, String)>, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut section = v_flex().gap(px(1.));

    if let Some(title) = header {
      section = section.child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child(title.to_string()),
      );
    }

    let rows_container = v_flex()
      .bg(colors.background)
      .rounded(px(8.))
      .overflow_hidden()
      .children(
        rows
          .into_iter()
          .enumerate()
          .map(|(i, (label, value))| Self::render_section_row(label, value, i == 0, cx)),
      );

    section.child(rows_container)
  }

  fn render_section_row(label: &str, value: String, is_first: bool, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut row = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .items_center()
      .justify_between()
      .child(
        div()
          .text_sm()
          .text_color(colors.secondary_foreground)
          .child(label.to_string()),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.foreground)
          .max_w(px(200.))
          .overflow_hidden()
          .text_ellipsis()
          .child(value),
      );

    if !is_first {
      row = row.border_t_1().border_color(colors.border);
    }

    row
  }

  fn render_labels_section(volume: &VolumeInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut labels: Vec<_> = volume.labels.iter().collect();
    labels.sort_by(|a, b| a.0.cmp(b.0));

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Labels"),
      )
      .child(
        v_flex()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .overflow_hidden()
                    // Header row
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Key"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Value"),
                            ),
                    )
                    // Label rows
                    .children(labels.iter().enumerate().map(|(i, (key, value))| {
                        let mut row = h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*key).clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*value).clone()),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  fn render_files_tab(&self, window: &mut Window, cx: &App) -> gpui::AnyElement {
    let state = self.volume_state.as_ref();

    let explorer_state = FileExplorerState {
      current_path: state.map_or_else(|| "/".to_string(), |s| s.current_path.clone()),
      is_loading: state.is_some_and(|s| s.files_loading),
      error: None,
      selected_file: state.and_then(|s| s.selected_file.clone()),
      file_content: state.map(|s| s.file_content.clone()).unwrap_or_default(),
      file_content_loading: state.is_some_and(|s| s.file_content_loading),
      file_content_error: None,
    };

    let files = state.map(|s| s.files.clone()).unwrap_or_default();

    let mut explorer = FileExplorer::new()
      .files(files)
      .state(explorer_state)
      .config(FileExplorerConfig::default().empty_message("Volume is empty"))
      .file_content_editor(self.file_content_editor.clone());

    if let Some(ref cb) = self.on_navigate_path {
      let cb = cb.clone();
      explorer = explorer.on_navigate(move |path, window, cx| {
        cb(path, window, cx);
      });
    }

    if let Some(ref cb) = self.on_file_select {
      let cb = cb.clone();
      explorer = explorer.on_file_select(move |path, window, cx| {
        cb(path, window, cx);
      });
    }

    if let Some(ref cb) = self.on_close_file_viewer {
      let cb = cb.clone();
      explorer = explorer.on_close_viewer(move |(), window, cx| {
        cb(&(), window, cx);
      });
    }

    if let Some(ref cb) = self.on_symlink_click {
      let cb = cb.clone();
      explorer = explorer.on_symlink_click(move |path, window, cx| {
        cb(path, window, cx);
      });
    }

    explorer.render(window, cx)
  }

  pub fn render(self, window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(volume) = &self.volume else {
      return Self::render_empty(cx).into_any_element();
    };

    let volume_name = volume.name.clone();
    let volume_name_for_delete = volume_name.clone();

    let on_delete = self.on_delete.clone();
    let on_tab_change = self.on_tab_change.clone();

    let tabs = ["Info", "Files"];

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(8.))
      .gap(px(12.))
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(
        TabBar::new("volume-tabs")
          .flex_1()
          .children(tabs.iter().enumerate().map(|(i, label)| {
            let on_tab_change = on_tab_change.clone();
            Tab::new()
              .label((*label).to_string())
              .selected(self.active_tab == i)
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_tab_change {
                  cb(&i, window, cx);
                }
              })
          })),
      )
      .child(h_flex().gap(px(8.)).child({
        let on_delete = on_delete.clone();
        let name = volume_name_for_delete.clone();
        Button::new("delete")
          .icon(Icon::new(AppIcon::Trash))
          .ghost()
          .small()
          .on_click(move |_ev, window, cx| {
            if let Some(ref cb) = on_delete {
              cb(&name, window, cx);
            }
          })
      }));

    // Content based on active tab
    let is_files_tab = self.active_tab == 1;

    let mut result = div().size_full().bg(colors.sidebar).flex().flex_col().child(toolbar);

    if is_files_tab {
      // Files tab handles its own scrolling (for file viewer)
      result = result.child(
        div()
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .child(self.render_files_tab(window, cx)),
      );
    } else {
      // Info tab with scroll container
      let content = Self::render_info_tab(volume, cx);
      result = result.child(
        div()
          .id("volume-detail-scroll")
          .flex_1()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      );
    }

    result.into_any_element()
  }
}
