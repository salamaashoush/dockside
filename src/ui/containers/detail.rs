use gpui::{App, Entity, Styled, Window, div, prelude::*, px};
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
use std::rc::Rc;

// Re-export from state module for backwards compatibility
pub use crate::state::ContainerDetailTab;

use crate::assets::AppIcon;
use crate::docker::{ContainerFileEntry, ContainerInfo};
use crate::terminal::TerminalView;
use crate::ui::components::{FileExplorer, FileExplorerConfig, FileExplorerState, ProcessView};

type ContainerActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&ContainerDetailTab, &mut Window, &mut App) + 'static>;
type RefreshCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type FileNavigateCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type FileSelectCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type CloseViewerCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type SymlinkClickCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type OpenInEditorCallback = Rc<dyn Fn(&(String, bool), &mut Window, &mut App) + 'static>;

/// State for container detail tabs
#[derive(Debug, Clone)]
pub struct ContainerTabState {
  pub logs: String,
  pub logs_loading: bool,
  /// Live-tail (follow) mode toggle. When true the view drives a streaming
  /// task that appends chunks; when false a one-shot snapshot is fetched.
  pub logs_follow: bool,
  /// Include RFC3339 timestamps in log lines.
  pub logs_timestamps: bool,
  pub inspect: String,
  pub inspect_loading: bool,
  pub current_path: String,
  pub files: Vec<ContainerFileEntry>,
  pub files_loading: bool,
  /// Error when listing files failed
  pub files_error: Option<String>,
  /// Selected file path for viewing
  pub selected_file: Option<String>,
  /// Content of selected file
  pub file_content: String,
  /// Whether file content is loading
  pub file_content_loading: bool,
  /// Error when loading file content failed
  pub file_content_error: Option<String>,
}

impl Default for ContainerTabState {
  fn default() -> Self {
    Self {
      logs: String::new(),
      logs_loading: false,
      logs_follow: true,
      logs_timestamps: false,
      inspect: String::new(),
      inspect_loading: false,
      current_path: String::new(),
      files: Vec::new(),
      files_loading: false,
      files_error: None,
      selected_file: None,
      file_content: String::new(),
      file_content_loading: false,
      file_content_error: None,
    }
  }
}

impl ContainerTabState {
  pub fn new() -> Self {
    Self {
      current_path: "/".to_string(),
      ..Default::default()
    }
  }
}

pub struct ContainerDetail {
  container: Option<ContainerInfo>,
  active_tab: ContainerDetailTab,
  container_state: Option<ContainerTabState>,
  terminal_view: Option<Entity<TerminalView>>,
  process_view: Option<Entity<ProcessView>>,
  logs_editor: Option<Entity<InputState>>,
  inspect_editor: Option<Entity<InputState>>,
  file_content_editor: Option<Entity<InputState>>,
  on_start: Option<ContainerActionCallback>,
  on_stop: Option<ContainerActionCallback>,
  on_restart: Option<ContainerActionCallback>,
  on_delete: Option<ContainerActionCallback>,
  on_tab_change: Option<TabChangeCallback>,
  on_refresh_logs: Option<RefreshCallback>,
  on_toggle_logs_follow: Option<RefreshCallback>,
  on_toggle_logs_timestamps: Option<RefreshCallback>,
  on_clear_logs: Option<RefreshCallback>,
  on_navigate_path: Option<FileNavigateCallback>,
  on_file_select: Option<FileSelectCallback>,
  on_close_file_viewer: Option<CloseViewerCallback>,
  on_symlink_click: Option<SymlinkClickCallback>,
  on_open_in_editor: Option<OpenInEditorCallback>,
}

impl ContainerDetail {
  pub fn new() -> Self {
    Self {
      container: None,
      active_tab: ContainerDetailTab::Info,
      container_state: None,
      terminal_view: None,
      process_view: None,
      logs_editor: None,
      inspect_editor: None,
      file_content_editor: None,
      on_start: None,
      on_stop: None,
      on_restart: None,
      on_delete: None,
      on_tab_change: None,
      on_refresh_logs: None,
      on_toggle_logs_follow: None,
      on_toggle_logs_timestamps: None,
      on_clear_logs: None,
      on_navigate_path: None,
      on_file_select: None,
      on_close_file_viewer: None,
      on_symlink_click: None,
      on_open_in_editor: None,
    }
  }

  pub fn container(mut self, container: Option<ContainerInfo>) -> Self {
    self.container = container;
    self
  }

  pub fn active_tab(mut self, tab: ContainerDetailTab) -> Self {
    self.active_tab = tab;
    self
  }

  pub fn container_state(mut self, state: ContainerTabState) -> Self {
    self.container_state = Some(state);
    self
  }

  pub fn terminal_view(mut self, view: Option<Entity<TerminalView>>) -> Self {
    self.terminal_view = view;
    self
  }

  pub fn process_view(mut self, view: Option<Entity<ProcessView>>) -> Self {
    self.process_view = view;
    self
  }

  pub fn logs_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.logs_editor = editor;
    self
  }

  pub fn inspect_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.inspect_editor = editor;
    self
  }

  pub fn file_content_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.file_content_editor = editor;
    self
  }

  pub fn on_start<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_start = Some(Rc::new(callback));
    self
  }

  pub fn on_stop<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_stop = Some(Rc::new(callback));
    self
  }

  pub fn on_restart<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_restart = Some(Rc::new(callback));
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
    F: Fn(&ContainerDetailTab, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  pub fn on_toggle_logs_follow<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_toggle_logs_follow = Some(Rc::new(callback));
    self
  }

  pub fn on_toggle_logs_timestamps<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_toggle_logs_timestamps = Some(Rc::new(callback));
    self
  }

  pub fn on_clear_logs<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_clear_logs = Some(Rc::new(callback));
    self
  }

  pub fn on_refresh_logs<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_refresh_logs = Some(Rc::new(callback));
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

  pub fn on_open_in_editor<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(String, bool), &mut Window, &mut App) + 'static,
  {
    self.on_open_in_editor = Some(Rc::new(callback));
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
            Icon::new(AppIcon::Container)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select a container to view details"),
          ),
      )
  }

  fn render_info_tab(container: &ContainerInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let info_row = |label: &str, value: String| {
      h_flex()
        .w_full()
        .py(px(12.))
        .justify_between()
        .border_b_1()
        .border_color(colors.border)
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child(label.to_string()),
        )
        .child(div().text_sm().text_color(colors.foreground).child(value))
    };

    let status_text = container.status.clone();
    let is_running = container.state.is_running();
    let status_color = if is_running { colors.success } else { colors.danger };

    v_flex()
      .w_full()
      .p(px(16.))
      .gap(px(8.))
      .child(info_row("Name", container.name.clone()))
      .child(info_row("ID", container.short_id().to_string()))
      .child(info_row("Image", container.image.clone()))
      .child(
        h_flex()
          .w_full()
          .py(px(12.))
          .justify_between()
          .border_b_1()
          .border_color(colors.border)
          .child(div().text_sm().text_color(colors.muted_foreground).child("Status"))
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(status_color))
              .child(div().text_sm().text_color(colors.foreground).child(status_text)),
          ),
      )
      .child(info_row("Ports", container.display_ports()))
      .when(container.command.is_some(), |el| {
        el.child(info_row("Command", container.command.clone().unwrap_or_default()))
      })
      .when(container.created.is_some(), |el| {
        el.child(info_row(
          "Created",
          container
            .created
            .map(|c| c.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default(),
        ))
      })
  }

  fn render_logs_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.container_state.as_ref();
    let is_loading = state.is_some_and(|s| s.logs_loading);
    let follow_on = state.is_some_and(|s| s.logs_follow);
    let ts_on = state.is_some_and(|s| s.logs_timestamps);

    let toggle_follow = self.on_toggle_logs_follow.clone();
    let toggle_ts = self.on_toggle_logs_timestamps.clone();
    let refresh = self.on_refresh_logs.clone();
    let clear = self.on_clear_logs.clone();

    let toolbar = h_flex()
      .gap(px(8.))
      .px(px(8.))
      .py(px(6.))
      .border_b_1()
      .border_color(colors.border)
      .child(
        Button::new("logs-follow")
          .label(if follow_on { "Following" } else { "Paused" })
          .icon(if follow_on {
            Icon::new(AppIcon::Pause)
          } else {
            Icon::new(AppIcon::Play)
          })
          .small()
          .when_some(toggle_follow, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      )
      .child(
        Button::new("logs-timestamps")
          .label(if ts_on { "Timestamps: on" } else { "Timestamps: off" })
          .small()
          .ghost()
          .when_some(toggle_ts, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      )
      .child(
        Button::new("logs-refresh")
          .icon(Icon::new(AppIcon::Refresh))
          .small()
          .ghost()
          .when_some(refresh, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      )
      .child(
        Button::new("logs-clear")
          .icon(Icon::new(AppIcon::Trash))
          .small()
          .ghost()
          .when_some(clear, |b, cb| {
            b.on_click(move |_ev, window, cx| {
              cb(&(), window, cx);
            })
          }),
      );

    let body: gpui::AnyElement = if is_loading && state.is_none_or(|s| s.logs.is_empty()) {
      v_flex()
        .size_full()
        .p(px(16.))
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Loading logs..."),
        )
        .into_any_element()
    } else if let Some(ref editor) = self.logs_editor {
      div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true))
        .into_any_element()
    } else {
      let logs_content = state.map_or_else(|| "No logs available".to_string(), |s| s.logs.clone());
      div()
        .size_full()
        .child(
          div()
            .size_full()
            .overflow_y_scrollbar()
            .bg(colors.sidebar)
            .p(px(12.))
            .font_family("monospace")
            .text_xs()
            .text_color(colors.foreground)
            .child(logs_content),
        )
        .into_any_element()
    };

    v_flex()
      .size_full()
      .child(toolbar)
      .child(div().flex_1().min_h_0().w_full().child(body))
  }

  fn render_processes_tab(&self, is_running: bool, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    // Container must be running to show processes
    if !is_running {
      return v_flex()
        .flex_1()
        .w_full()
        .p(px(16.))
        .items_center()
        .justify_center()
        .gap(px(16.))
        .child(
          Icon::new(IconName::Info)
            .size(px(48.))
            .text_color(colors.muted_foreground),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Container must be running to view processes"),
        )
        .into_any_element();
    }

    // If we have a process view, render it full size
    if let Some(process_view) = &self.process_view {
      return div()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(process_view.clone())
        .into_any_element();
    }

    // Fallback: show loading message
    v_flex()
      .flex_1()
      .w_full()
      .items_center()
      .justify_center()
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading processes..."),
      )
      .into_any_element()
  }

  fn render_terminal_tab(&self, is_running: bool, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    // Container must be running to connect terminal
    if !is_running {
      return v_flex()
        .flex_1()
        .w_full()
        .p(px(16.))
        .items_center()
        .justify_center()
        .gap(px(16.))
        .child(
          Icon::new(AppIcon::Terminal)
            .size(px(48.))
            .text_color(colors.muted_foreground),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Container must be running to connect terminal"),
        )
        .into_any_element();
    }

    // If we have a terminal view, render it full size
    if let Some(terminal) = &self.terminal_view {
      return div()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(terminal.clone())
        .into_any_element();
    }

    // Fallback: show message (shouldn't normally happen if tab change creates terminal)
    v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .items_center()
      .justify_center()
      .gap(px(16.))
      .child(
        Icon::new(AppIcon::Terminal)
          .size(px(48.))
          .text_color(colors.muted_foreground),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Connecting to container..."),
      )
      .into_any_element()
  }

  fn render_inspect_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.container_state.as_ref();
    let is_loading = state.is_some_and(|s| s.inspect_loading);

    if is_loading {
      return v_flex()
        .size_full()
        .p(px(16.))
        .child(div().text_sm().text_color(colors.muted_foreground).child("Loading..."));
    }

    if let Some(ref editor) = self.inspect_editor {
      return div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true));
    }

    // Fallback to plain text
    let inspect_content = state.map_or_else(|| "{}".to_string(), |s| s.inspect.clone());
    div().size_full().child(
      div()
        .size_full()
        .overflow_y_scrollbar()
        .bg(colors.sidebar)
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(inspect_content),
    )
  }

  fn render_files_tab(&self, is_running: bool, window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    // Container must be running to browse files
    if !is_running {
      return v_flex()
        .flex_1()
        .w_full()
        .p(px(16.))
        .items_center()
        .justify_center()
        .gap(px(16.))
        .child(
          Icon::new(AppIcon::Files)
            .size(px(48.))
            .text_color(colors.muted_foreground),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Container must be running to browse files"),
        )
        .into_any_element();
    }

    let state = self.container_state.as_ref();

    let explorer_state = FileExplorerState {
      current_path: state.map_or_else(|| "/".to_string(), |s| s.current_path.clone()),
      is_loading: state.is_some_and(|s| s.files_loading),
      error: state.and_then(|s| s.files_error.clone()),
      selected_file: state.and_then(|s| s.selected_file.clone()),
      file_content: state.map(|s| s.file_content.clone()).unwrap_or_default(),
      file_content_loading: state.is_some_and(|s| s.file_content_loading),
      file_content_error: state.and_then(|s| s.file_content_error.clone()),
    };

    let files = state.map(|s| s.files.clone()).unwrap_or_default();

    let mut explorer = FileExplorer::new()
      .files(files)
      .state(explorer_state)
      .config(FileExplorerConfig::default().empty_message("Directory is empty"))
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

    if let Some(ref cb) = self.on_open_in_editor {
      let cb = cb.clone();
      explorer = explorer.on_open_in_editor(move |data: &(String, bool), window, cx| {
        cb(data, window, cx);
      });
    }

    explorer.render(window, cx)
  }

  pub fn render(&self, window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(container) = &self.container else {
      return Self::render_empty(cx).into_any_element();
    };

    let is_running = container.state.is_running();
    let container_id = container.id.clone();
    let container_id_for_stop = container_id.clone();
    let container_id_for_restart = container_id.clone();
    let container_id_for_delete = container_id.clone();

    let on_start = self.on_start.clone();
    let on_stop = self.on_stop.clone();
    let on_restart = self.on_restart.clone();
    let on_delete = self.on_delete.clone();
    let on_tab_change = self.on_tab_change.clone();

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(8.))
      .gap(px(12.))
      .items_center()
      .flex_shrink_0()
      .border_b_1()
      .border_color(colors.border)
      .child(
        TabBar::new("container-tabs")
          .flex_1()
          .children(ContainerDetailTab::ALL.iter().map(|tab| {
            let on_tab_change = on_tab_change.clone();
            let tab_variant = *tab;
            Tab::new()
              .label(tab.label().to_string())
              .selected(self.active_tab == *tab)
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_tab_change {
                  cb(&tab_variant, window, cx);
                }
              })
          })),
      )
      .child(
        h_flex()
          .gap(px(8.))
          .when(!is_running, |el| {
            let on_start = on_start.clone();
            let id = container_id.clone();
            el.child(
              Button::new("start")
                .icon(Icon::new(AppIcon::Play))
                .ghost()
                .small()
                .on_click(move |_ev, window, cx| {
                  if let Some(ref cb) = on_start {
                    cb(&id, window, cx);
                  }
                }),
            )
          })
          .when(is_running, |el| {
            let on_stop = on_stop.clone();
            let id = container_id_for_stop.clone();
            el.child(
              Button::new("stop")
                .icon(Icon::new(AppIcon::Stop))
                .ghost()
                .small()
                .on_click(move |_ev, window, cx| {
                  if let Some(ref cb) = on_stop {
                    cb(&id, window, cx);
                  }
                }),
            )
          })
          .child({
            let on_restart = on_restart.clone();
            let id = container_id_for_restart.clone();
            Button::new("restart")
              .icon(Icon::new(AppIcon::Restart))
              .ghost()
              .small()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_restart {
                  cb(&id, window, cx);
                }
              })
          })
          .child({
            let on_delete = on_delete.clone();
            let id = container_id_for_delete.clone();
            Button::new("delete")
              .icon(Icon::new(AppIcon::Trash))
              .ghost()
              .small()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_delete {
                  cb(&id, window, cx);
                }
              })
          }),
      );

    // Terminal, Logs, Processes, and Files tabs need full height without scroll
    let is_full_height_tab = matches!(
      self.active_tab,
      ContainerDetailTab::Logs
        | ContainerDetailTab::Processes
        | ContainerDetailTab::Terminal
        | ContainerDetailTab::Files
    );

    // Content based on active tab
    let mut result = div()
      .size_full()
      .overflow_hidden()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .child(toolbar);

    if is_full_height_tab {
      let content = match self.active_tab {
        ContainerDetailTab::Logs => self.render_logs_tab(cx).into_any_element(),
        ContainerDetailTab::Processes => self.render_processes_tab(is_running, cx),
        ContainerDetailTab::Terminal => self.render_terminal_tab(is_running, cx),
        ContainerDetailTab::Files => self.render_files_tab(is_running, window, cx),
        _ => Self::render_info_tab(container, cx).into_any_element(),
      };
      result = result.child(
        div()
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .flex()
          .flex_col()
          .child(content),
      );
    } else {
      let content = match self.active_tab {
        ContainerDetailTab::Inspect => self.render_inspect_tab(cx),
        _ => Self::render_info_tab(container, cx),
      };
      result = result.child(
        div()
          .id("container-detail-scroll")
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      );
    }

    result.into_any_element()
  }
}
