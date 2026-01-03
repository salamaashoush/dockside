use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  input::InputState,
  theme::ActiveTheme,
};

use crate::docker::VolumeInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};
use crate::ui::components::detect_language_from_path;

use super::create_dialog::CreateVolumeDialog;
use super::detail::{VolumeDetail, VolumeTabState};
use super::list::{VolumeList, VolumeListEvent};

/// Self-contained Volumes view - handles list, detail, and all state
pub struct VolumesView {
  _docker_state: Entity<DockerState>,
  volume_list: Entity<VolumeList>,
  selected_volume: Option<VolumeInfo>,
  active_tab: usize,
  volume_tab_state: VolumeTabState,
  file_content_editor: Option<Entity<InputState>>,
  last_synced_file_content: String,
}

impl VolumesView {
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
          this.on_select_volume(volume, cx);
        }
        VolumeListEvent::NewVolume => {
          this.show_create_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to state changes
    cx.subscribe(&docker_state, |this, state, event: &StateChanged, cx| {
      match event {
        StateChanged::VolumesUpdated => {
          // If selected volume was deleted, clear selection
          if let Some(ref selected) = this.selected_volume {
            let state = state.read(cx);
            if !state.volumes.iter().any(|v| v.name == selected.name) {
              this.selected_volume = None;
              this.active_tab = 0;
              this.volume_tab_state = VolumeTabState::new();
            } else {
              // Update the selected volume info
              if let Some(updated) = state.volumes.iter().find(|v| v.name == selected.name) {
                this.selected_volume = Some(updated.clone());
              }
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
          if let Some(ref selected) = this.selected_volume
            && &selected.name == volume_name
          {
            this.volume_tab_state.files = files.clone();
            this.volume_tab_state.current_path = path.clone();
            this.volume_tab_state.files_loading = false;
            cx.notify();
          }
        }
        StateChanged::VolumeFilesError { volume_name } => {
          // Handle error for the currently selected volume
          if let Some(ref selected) = this.selected_volume
            && &selected.name == volume_name
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
      _docker_state: docker_state,
      volume_list,
      selected_volume: None,
      active_tab: 0,
      volume_tab_state: VolumeTabState::new(),
      file_content_editor: None,
      last_synced_file_content: String::new(),
    }
  }

  fn show_create_dialog(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let dialog_entity = cx.new(CreateVolumeDialog::new);

    window.open_dialog(cx, move |dialog, _window, _cx| {
      let dialog_clone = dialog_entity.clone();

      dialog
        .title("New Volume")
        .min_w(px(500.))
        .child(dialog_entity.clone())
        .footer(move |_dialog_state, _, _window, _cx| {
          let dialog_for_create = dialog_clone.clone();

          vec![
            Button::new("create")
              .label("Create")
              .primary()
              .on_click({
                let dialog = dialog_for_create.clone();
                move |_ev, window, cx| {
                  let options = dialog.read(cx).get_options(cx);
                  if !options.name.is_empty() {
                    services::create_volume(
                      options.name,
                      options.driver.as_docker_arg().to_string(),
                      options.labels,
                      cx,
                    );
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn on_select_volume(&mut self, volume: &VolumeInfo, cx: &mut Context<'_, Self>) {
    self.selected_volume = Some(volume.clone());
    self.active_tab = 0;
    // Reset file state when selecting new volume
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
    if let Some(ref volume) = self.selected_volume {
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
    if let Some(ref volume) = self.selected_volume {
      self.load_volume_file_content(&volume.name.clone(), path, cx);
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
    if let Some(ref volume) = self.selected_volume.clone() {
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
              this.volume_tab_state.current_path = target.clone();
              if let Some(ref volume) = this.selected_volume {
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
              if let Some(ref volume) = this.selected_volume {
                this.load_volume_file_content(&volume.name.clone(), &target, cx);
              }
            }
          }
          cx.notify();
        });
      })
      .detach();
    }
  }

  fn load_volume_file_content(&mut self, volume_name: &str, path: &str, cx: &mut Context<'_, Self>) {
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
    let selected_volume = self.selected_volume.clone();
    let active_tab = self.active_tab;
    let file_content_editor = self.file_content_editor.clone();

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
      .on_close_file_viewer(cx.listener(|this, _: &(), _window, cx| {
        this.on_close_file_viewer(cx);
      }))
      .on_symlink_click(cx.listener(|this, path: &str, window, cx| {
        this.on_symlink_follow(path, window, cx);
      }))
      .on_delete(cx.listener(|this, name: &str, _window, cx| {
        services::delete_volume(name.to_string(), cx);
        this.selected_volume = None;
        this.active_tab = 0;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Volume list - fixed width with border
        div()
          .w(px(320.))
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .border_r_1()
          .border_color(colors.border)
          .child(self.volume_list.clone()),
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
