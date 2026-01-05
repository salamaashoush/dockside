use gpui::{App, Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{input::InputState, theme::ActiveTheme};

use crate::docker::VolumeInfo;
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state};
use crate::ui::components::detect_language_from_path;
use crate::ui::dialogs;

use super::detail::{VolumeDetail, VolumeTabState};
use super::list::{VolumeList, VolumeListEvent};

/// Self-contained Volumes view - handles list, detail, and all state
pub struct VolumesView {
  docker_state: Entity<DockerState>,
  volume_list: Entity<VolumeList>,
  // View-specific state (not selection - that's in global DockerState)
  active_tab: usize,
  volume_tab_state: VolumeTabState,
  file_content_editor: Option<Entity<InputState>>,
  last_synced_file_content: String,
}

impl VolumesView {
  /// Get the currently selected volume from global state
  fn selected_volume(&self, cx: &App) -> Option<VolumeInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Volume(ref name) = state.selection {
      state.volumes.iter().find(|v| v.name == *name).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Create volume list entity
    let volume_list = cx.new(|cx| VolumeList::new(window, cx));

    // Subscribe to volume list events
    cx.subscribe_in(
      &volume_list,
      window,
      |this, _list, event: &VolumeListEvent, window, cx| match event {
        VolumeListEvent::Selected(volume) => {
          this.on_select_volume(volume.as_ref(), cx);
        }
        VolumeListEvent::NewVolume => {
          Self::show_create_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to state changes
    cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
      match event {
        StateChanged::VolumesUpdated => {
          // If selected volume was deleted, clear selection
          let selected_name = {
            if let Selection::Volume(ref name) = this.docker_state.read(cx).selection {
              Some(name.clone())
            } else {
              None
            }
          };

          if let Some(name) = selected_name {
            let ds = state.read(cx);
            if !ds.volumes.iter().any(|v| v.name == name) {
              // Volume was deleted
              this.docker_state.update(cx, |s, _| {
                s.set_selection(Selection::None);
              });
              this.active_tab = 0;
              this.volume_tab_state = VolumeTabState::new();
            }
          }
          cx.notify();
        }
        StateChanged::VolumeFilesLoaded {
          volume_name,
          path,
          files,
        } => {
          // Update file state if this is for the currently selected volume
          if let Some(selected) = this.selected_volume(cx)
            && selected.name == *volume_name
          {
            files.clone_into(&mut this.volume_tab_state.files);
            path.clone_into(&mut this.volume_tab_state.current_path);
            this.volume_tab_state.files_loading = false;
            cx.notify();
          }
        }
        StateChanged::VolumeFilesError { volume_name } => {
          // Handle error for the currently selected volume
          if let Some(selected) = this.selected_volume(cx)
            && selected.name == *volume_name
          {
            this.volume_tab_state.files_loading = false;
            this.volume_tab_state.files = vec![];
            cx.notify();
          }
        }
        _ => {}
      }
    })
    .detach();

    Self {
      docker_state,
      volume_list,
      active_tab: 0,
      volume_tab_state: VolumeTabState::new(),
      file_content_editor: None,
      last_synced_file_content: String::new(),
    }
  }

  fn show_create_dialog(window: &mut Window, cx: &mut Context<'_, Self>) {
    dialogs::open_create_volume_dialog(window, cx);
  }

  fn on_select_volume(&mut self, volume: &VolumeInfo, cx: &mut Context<'_, Self>) {
    // Update global selection (single source of truth)
    self.docker_state.update(cx, |state, _cx| {
      state.set_selection(Selection::Volume(volume.name.clone()));
    });

    // Reset view-specific state
    self.active_tab = 0;
    self.volume_tab_state = VolumeTabState::new();
    self.file_content_editor = None;
    self.last_synced_file_content.clear();

    cx.notify();
  }

  fn on_tab_change(&mut self, tab: usize, cx: &mut Context<'_, Self>) {
    self.active_tab = tab;
    // Load files when switching to Files tab
    if tab == 1 {
      self.load_volume_files("/", cx);
    }
    cx.notify();
  }

  fn load_volume_files(&mut self, path: &str, cx: &mut Context<'_, Self>) {
    if let Some(volume) = self.selected_volume(cx) {
      self.volume_tab_state.files_loading = true;
      self.volume_tab_state.current_path = path.to_string();
      cx.notify();

      let volume_name = volume.name.clone();
      let path = path.to_string();
      services::list_volume_files(volume_name, path, cx);
    }
  }

  fn on_navigate_path(&mut self, path: &str, cx: &mut Context<'_, Self>) {
    self.load_volume_files(path, cx);
  }

  fn on_file_select(&mut self, path: &str, window: &mut Window, cx: &mut Context<'_, Self>) {
    // Detect language from file extension
    let language = detect_language_from_path(path);

    // Create file content editor with detected language
    self.file_content_editor = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor(language)
        .line_number(true)
        .searchable(true)
        .soft_wrap(false)
    }));

    // Clear synced tracking for new file
    self.last_synced_file_content.clear();

    // Set selected file in state
    self.volume_tab_state.selected_file = Some(path.to_string());
    self.volume_tab_state.file_content_loading = true;
    cx.notify();

    // Load file content
    if let Some(volume) = self.selected_volume(cx) {
      Self::load_volume_file_content(&volume.name.clone(), path, cx);
    }
  }

  fn on_close_file_viewer(&mut self, cx: &mut Context<'_, Self>) {
    self.volume_tab_state.selected_file = None;
    self.volume_tab_state.file_content.clear();
    self.file_content_editor = None;
    self.last_synced_file_content.clear();
    cx.notify();
  }

  fn on_symlink_follow(&mut self, path: &str, window: &mut Window, cx: &mut Context<'_, Self>) {
    if let Some(volume) = self.selected_volume(cx) {
      let volume_name = volume.name.clone();
      let path = path.to_string();
      let tokio_handle = services::Tokio::runtime_handle();
      let client = services::docker_client();

      // Create file content editor in case symlink points to a file
      let language = detect_language_from_path(&path);
      let file_editor = cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .code_editor(language)
          .line_number(true)
          .searchable(true)
          .soft_wrap(false)
      });

      cx.spawn(async move |this, cx| {
        let result = cx
          .background_executor()
          .spawn(async move {
            tokio_handle.block_on(async {
              let guard = client.read().await;
              match guard.as_ref() {
                Some(c) => {
                  // Resolve symlink and check if it's a directory
                  if let Ok(target) = c.resolve_volume_symlink(&volume_name, &path).await {
                    let is_dir = c.is_volume_directory(&volume_name, &target).await.unwrap_or(false);
                    Some((target, is_dir))
                  } else {
                    None
                  }
                }
                None => None,
              }
            })
          })
          .await;

        let _ = this.update(cx, |this, cx| {
          if let Some((target, is_dir)) = result {
            if is_dir {
              // Navigate to directory
              this.volume_tab_state.current_path.clone_from(&target);
              if let Some(volume) = this.selected_volume(cx) {
                this.volume_tab_state.files_loading = true;
                cx.notify();
                services::list_volume_files(volume.name.clone(), target, cx);
              }
            } else {
              // View file - set up the editor
              this.file_content_editor = Some(file_editor.clone());
              this.last_synced_file_content.clear();
              this.volume_tab_state.selected_file = Some(target.clone());
              this.volume_tab_state.file_content_loading = true;
              if let Some(volume) = this.selected_volume(cx) {
                Self::load_volume_file_content(&volume.name.clone(), &target, cx);
              }
            }
          }
          cx.notify();
        });
      })
      .detach();
    }
  }

  fn load_volume_file_content(volume_name: &str, path: &str, cx: &mut Context<'_, Self>) {
    let name = volume_name.to_string();
    let path = path.to_string();
    let tokio_handle = services::Tokio::runtime_handle();
    let client = services::docker_client();

    cx.spawn(async move |this, cx| {
      let content = cx
        .background_executor()
        .spawn(async move {
          tokio_handle.block_on(async {
            let guard = client.read().await;
            match guard.as_ref() {
              Some(c) => c.read_volume_file(&name, &path).await.ok(),
              None => None,
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.volume_tab_state.file_content = content.unwrap_or_else(|| "Failed to read file".to_string());
        this.volume_tab_state.file_content_loading = false;
        cx.notify();
      });
    })
    .detach();
  }
}

impl Render for VolumesView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Sync file content editor
    if let Some(ref editor) = self.file_content_editor {
      let content = &self.volume_tab_state.file_content;
      if !content.is_empty() && !self.volume_tab_state.file_content_loading && self.last_synced_file_content != *content
      {
        let content_clone = content.clone();
        editor.update(cx, |state, cx| {
          state.replace(&content_clone, window, cx);
        });
        self.last_synced_file_content = content.clone();
      }
    }

    let colors = cx.theme().colors;
    let selected_volume = self.selected_volume(cx);
    let active_tab = self.active_tab;
    let file_content_editor = self.file_content_editor.clone();
    let has_selection = selected_volume.is_some();

    // Build detail panel
    let detail = VolumeDetail::new()
      .volume(selected_volume)
      .active_tab(active_tab)
      .volume_state(self.volume_tab_state.clone())
      .file_content_editor(file_content_editor)
      .on_tab_change(cx.listener(|this, tab: &usize, _window, cx| {
        this.on_tab_change(*tab, cx);
      }))
      .on_navigate_path(cx.listener(|this, path: &str, _window, cx| {
        this.on_navigate_path(path, cx);
      }))
      .on_file_select(cx.listener(|this, path: &str, window, cx| {
        this.on_file_select(path, window, cx);
      }))
      .on_close_file_viewer(cx.listener(|this, (): &(), _window, cx| {
        this.on_close_file_viewer(cx);
      }))
      .on_symlink_click(cx.listener(|this, path: &str, window, cx| {
        this.on_symlink_follow(path, window, cx);
      }))
      .on_delete(cx.listener(|this, _name: &str, _window, cx| {
        this.docker_state.update(cx, |s, _| s.set_selection(Selection::None));
        this.active_tab = 0;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Volume list - fixed width when selected, full width when not
        div()
          .when(has_selection, |el| {
            el.w(px(320.)).border_r_1().border_color(colors.border)
          })
          .when(!has_selection, gpui::Styled::flex_1)
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .child(self.volume_list.clone()),
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
