use gpui::{Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{
  WindowExt,
  button::{Button, ButtonVariants},
  input::InputState,
  theme::ActiveTheme,
};
use std::time::Duration;

use crate::docker::ContainerInfo;
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state, settings_state};
use crate::terminal::{TerminalSessionType, TerminalView};
use crate::ui::components::detect_language_from_path;

use super::create_dialog::CreateContainerDialog;
use super::detail::{ContainerDetail, ContainerTabState};
use super::list::{ContainerList, ContainerListEvent};

/// Self-contained Containers view - handles list, detail, and all state
pub struct ContainersView {
  _docker_state: Entity<DockerState>,
  container_list: Entity<ContainerList>,
  selected_container: Option<ContainerInfo>,
  active_tab: usize,
  terminal_view: Option<Entity<TerminalView>>,
  logs_editor: Option<Entity<InputState>>,
  inspect_editor: Option<Entity<InputState>>,
  file_content_editor: Option<Entity<InputState>>,
  container_tab_state: ContainerTabState,
  // Track what we've synced to editors to prevent infinite loops
  last_synced_logs: String,
  last_synced_inspect: String,
  last_synced_file_content: String,
}

impl ContainersView {
  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Create container list entity
    let container_list = cx.new(|cx| ContainerList::new(window, cx));

    // Subscribe to container list events
    cx.subscribe_in(
      &container_list,
      window,
      |this, _list, event: &ContainerListEvent, window, cx| match event {
        ContainerListEvent::Selected(container) => {
          this.on_select_container(container, window, cx);
        }
        ContainerListEvent::NewContainer => {
          this.show_create_dialog(window, cx);
        }
      },
    )
    .detach();

    // Subscribe to state changes
    cx.subscribe_in(
      &docker_state,
      window,
      |this, state, event: &StateChanged, window, cx| {
        match event {
          StateChanged::ContainersUpdated => {
            // If selected container was deleted, clear selection
            if let Some(ref selected) = this.selected_container {
              let state = state.read(cx);
              if !state.containers.iter().any(|c| c.id == selected.id) {
                this.selected_container = None;
                this.active_tab = 0;
                this.terminal_view = None;
              } else {
                // Update the selected container info
                if let Some(updated) = state.containers.iter().find(|c| c.id == selected.id) {
                  this.selected_container = Some(updated.clone());
                }
              }
            }
            cx.notify();
          }
          StateChanged::ContainerTabRequest { container_id, tab } => {
            // Find the container and select it with the specified tab
            let container = {
              let state = state.read(cx);
              state.containers.iter().find(|c| c.id == *container_id).cloned()
            };
            if let Some(container) = container {
              this.on_select_container(&container, window, cx);
              this.on_tab_change(*tab, window, cx);
            }
          }
          StateChanged::RenameContainerRequest {
            container_id,
            current_name,
          } => {
            this.show_rename_dialog(container_id.clone(), current_name.clone(), window, cx);
          }
          StateChanged::CommitContainerRequest {
            container_id,
            container_name,
          } => {
            this.show_commit_dialog(container_id.clone(), container_name.clone(), window, cx);
          }
          StateChanged::ExportContainerRequest {
            container_id,
            container_name,
          } => {
            this.show_export_dialog(container_id.clone(), container_name.clone(), window, cx);
          }
          _ => {}
        }
      },
    )
    .detach();

    // Start periodic container refresh using interval from settings
    let refresh_interval = settings_state(cx).read(cx).settings.container_refresh_interval;
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh_interval)).await;
        let _ = cx.update(|cx| {
          services::refresh_containers(cx);
        });
      }
    })
    .detach();

    Self {
      _docker_state: docker_state,
      container_list,
      selected_container: None,
      active_tab: 0,
      terminal_view: None,
      logs_editor: None,
      inspect_editor: None,
      file_content_editor: None,
      container_tab_state: ContainerTabState::new(),
      last_synced_logs: String::new(),
      last_synced_inspect: String::new(),
      last_synced_file_content: String::new(),
    }
  }

  fn show_create_dialog(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let dialog_entity = cx.new(CreateContainerDialog::new);

    window.open_dialog(cx, move |dialog, _window, cx| {
      let _colors = cx.theme().colors;
      let dialog_clone = dialog_entity.clone();
      let dialog_clone2 = dialog_entity.clone();

      dialog
        .title("New Container")
        .min_w(px(550.))
        .child(dialog_entity.clone())
        .footer(move |_dialog_state, _, _window, _cx| {
          let dialog_for_create = dialog_clone.clone();
          let dialog_for_start = dialog_clone2.clone();

          vec![
            Button::new("create")
              .label("Create")
              .ghost()
              .on_click({
                let dialog = dialog_for_create.clone();
                move |_ev, window, cx| {
                  let options = dialog.read(cx).get_options(cx, false);
                  if !options.image.is_empty() {
                    services::create_container(options, cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
            Button::new("create-start")
              .label("Create & Start")
              .primary()
              .on_click({
                let dialog = dialog_for_start.clone();
                move |_ev, window, cx| {
                  let options = dialog.read(cx).get_options(cx, true);
                  if !options.image.is_empty() {
                    services::create_container(options, cx);
                    window.close_dialog(cx);
                  }
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn show_rename_dialog(
    &mut self,
    container_id: String,
    current_name: String,
    window: &mut Window,
    cx: &mut Context<'_, Self>,
  ) {
    use gpui_component::input::{Input, InputState};

    let name_input = cx.new(|cx| InputState::new(window, cx).default_value(current_name));

    window.open_dialog(cx, move |dialog, _window, _cx| {
      let name_input_clone = name_input.clone();
      let container_id = container_id.clone();

      dialog
        .title("Rename Container")
        .min_w(px(400.))
        .child(Input::new(&name_input).w_full())
        .footer(move |_dialog_state, _, _window, _cx| {
          let name_input = name_input_clone.clone();
          let id = container_id.clone();

          vec![
            Button::new("rename")
              .label("Rename")
              .primary()
              .on_click(move |_ev, window, cx| {
                let new_name = name_input.read(cx).text().to_string();
                if !new_name.is_empty() {
                  services::rename_container(id.clone(), new_name, cx);
                  window.close_dialog(cx);
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn show_commit_dialog(
    &mut self,
    container_id: String,
    container_name: String,
    window: &mut Window,
    cx: &mut Context<'_, Self>,
  ) {
    use gpui_component::{
      input::{Input, InputState},
      v_flex,
    };

    let repo_input = cx.new(|cx| InputState::new(window, cx).placeholder("Repository (e.g., myrepo/myimage)"));
    let tag_input = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("Tag")
        .default_value("latest".to_string())
    });
    let comment_input = cx.new(|cx| {
      InputState::new(window, cx).placeholder(format!("Comment (optional, from container: {})", container_name))
    });

    window.open_dialog(cx, move |dialog, _window, cx| {
      let colors = cx.theme().colors;
      let repo_clone = repo_input.clone();
      let tag_clone = tag_input.clone();
      let comment_clone = comment_input.clone();
      let container_id = container_id.clone();

      dialog
        .title("Commit Container to Image")
        .min_w(px(450.))
        .child(
          v_flex()
            .gap(px(12.))
            .child(
              div()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("Save the container's current state as a new image."),
            )
            .child(Input::new(&repo_input).w_full())
            .child(Input::new(&tag_input).w_full())
            .child(Input::new(&comment_input).w_full()),
        )
        .footer(move |_dialog_state, _, _window, _cx| {
          let repo = repo_clone.clone();
          let tag = tag_clone.clone();
          let comment = comment_clone.clone();
          let id = container_id.clone();

          vec![
            Button::new("commit")
              .label("Commit")
              .primary()
              .on_click(move |_ev, window, cx| {
                let repo_text = repo.read(cx).text().to_string();
                let tag_text = tag.read(cx).text().to_string();
                let comment_text = comment.read(cx).text().to_string();

                if !repo_text.is_empty() {
                  let comment_opt = if comment_text.is_empty() {
                    None
                  } else {
                    Some(comment_text)
                  };
                  services::commit_container(id.clone(), repo_text, tag_text, comment_opt, None, cx);
                  window.close_dialog(cx);
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn show_export_dialog(
    &mut self,
    container_id: String,
    container_name: String,
    window: &mut Window,
    cx: &mut Context<'_, Self>,
  ) {
    use gpui_component::{
      input::{Input, InputState},
      v_flex,
    };

    // Default path: ~/container_<name>.tar
    let default_path = format!(
      "{}/{}.tar",
      std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
      container_name
    );

    let path_input = cx.new(|cx| InputState::new(window, cx).default_value(default_path));

    window.open_dialog(cx, move |dialog, _window, cx| {
      let colors = cx.theme().colors;
      let path_clone = path_input.clone();
      let container_id = container_id.clone();

      dialog
        .title("Export Container")
        .min_w(px(500.))
        .child(
          v_flex()
            .gap(px(12.))
            .child(
              div()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("Export the container's filesystem as a tar archive."),
            )
            .child(Input::new(&path_input).w_full()),
        )
        .footer(move |_dialog_state, _, _window, _cx| {
          let path = path_clone.clone();
          let id = container_id.clone();

          vec![
            Button::new("export")
              .label("Export")
              .primary()
              .on_click(move |_ev, window, cx| {
                let path_text = path.read(cx).text().to_string();
                if !path_text.is_empty() {
                  services::export_container(id.clone(), path_text, cx);
                  window.close_dialog(cx);
                }
              })
              .into_any_element(),
          ]
        })
    });
  }

  fn on_select_container(&mut self, container: &ContainerInfo, window: &mut Window, cx: &mut Context<'_, Self>) {
    self.selected_container = Some(container.clone());
    self.active_tab = 0;
    self.terminal_view = None;
    // Clear synced tracking for new container
    self.last_synced_logs.clear();
    self.last_synced_inspect.clear();

    // Create editors for logs and inspect with syntax highlighting
    // Note: code_editor() is required for replace() method to work
    self.logs_editor = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("log")
        .line_number(true)
        .searchable(true)
        .soft_wrap(false)
    }));

    self.inspect_editor = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("json")
        .line_number(true)
        .searchable(true)
        .soft_wrap(false)
    }));

    // Load logs for the selected container
    self.load_container_data(&container.id, window, cx);

    cx.notify();
  }

  fn on_tab_change(&mut self, tab: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
    self.active_tab = tab;

    // If switching to terminal tab, create terminal view
    if tab == 2
      && self.terminal_view.is_none()
      && let Some(ref container) = self.selected_container
    {
      let container_id = container.id.clone();
      self.terminal_view =
        Some(cx.new(|cx| TerminalView::new(TerminalSessionType::docker_exec(container_id, None), window, cx)));
    }

    // If switching to files tab, load files
    if tab == 3
      && let Some(ref container) = self.selected_container
      && container.state.is_running()
    {
      let container_id = container.id.clone();
      let path = self.container_tab_state.current_path.clone();
      self.load_container_files(&container_id, &path, cx);
    }

    cx.notify();
  }

  fn on_navigate_path(&mut self, path: &str, cx: &mut Context<'_, Self>) {
    self.container_tab_state.current_path = path.to_string();
    if let Some(ref container) = self.selected_container
      && container.state.is_running()
    {
      let container_id = container.id.clone();
      self.load_container_files(&container_id, path, cx);
    }
  }

  fn load_container_files(&mut self, container_id: &str, path: &str, cx: &mut Context<'_, Self>) {
    self.container_tab_state.files_loading = true;
    self.container_tab_state.files.clear();
    cx.notify();

    let id = container_id.to_string();
    let path = path.to_string();
    let tokio_handle = services::Tokio::runtime_handle();
    let client = services::docker_client();

    cx.spawn(async move |this, cx| {
      let files = cx
        .background_executor()
        .spawn(async move {
          tokio_handle.block_on(async {
            let guard = client.read().await;
            match guard.as_ref() {
              Some(c) => c.list_container_files(&id, &path).await.ok(),
              None => None,
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.container_tab_state.files = files.unwrap_or_default();
        this.container_tab_state.files_loading = false;
        cx.notify();
      });
    })
    .detach();
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
    self.container_tab_state.selected_file = Some(path.to_string());
    self.container_tab_state.file_content_loading = true;
    cx.notify();

    // Load file content
    if let Some(ref container) = self.selected_container {
      self.load_container_file_content(&container.id.clone(), path, cx);
    }
  }

  fn on_close_file_viewer(&mut self, cx: &mut Context<'_, Self>) {
    self.container_tab_state.selected_file = None;
    self.container_tab_state.file_content.clear();
    self.file_content_editor = None;
    self.last_synced_file_content.clear();
    cx.notify();
  }

  fn load_container_file_content(&mut self, container_id: &str, path: &str, cx: &mut Context<'_, Self>) {
    let id = container_id.to_string();
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
              Some(c) => c.read_container_file(&id, &path).await.ok(),
              None => None,
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.container_tab_state.file_content = content.unwrap_or_else(|| "Failed to read file".to_string());
        this.container_tab_state.file_content_loading = false;
        cx.notify();
      });
    })
    .detach();
  }

  fn on_symlink_follow(&mut self, path: &str, window: &mut Window, cx: &mut Context<'_, Self>) {
    if let Some(ref container) = self.selected_container.clone() {
      if !container.state.is_running() {
        return;
      }

      let id = container.id.clone();
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
                  if let Ok(target) = c.resolve_symlink(&id, &path).await {
                    let is_dir = c.is_directory(&id, &target).await.unwrap_or(false);
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
              this.container_tab_state.current_path = target;
              if let Some(ref container) = this.selected_container {
                this.load_container_files(
                  &container.id.clone(),
                  &this.container_tab_state.current_path.clone(),
                  cx,
                );
              }
            } else {
              // View file - set up the editor
              this.file_content_editor = Some(file_editor.clone());
              this.last_synced_file_content.clear();
              this.container_tab_state.selected_file = Some(target.clone());
              this.container_tab_state.file_content_loading = true;
              if let Some(ref container) = this.selected_container {
                this.load_container_file_content(&container.id.clone(), &target, cx);
              }
            }
          }
          cx.notify();
        });
      })
      .detach();
    }
  }

  fn load_container_data(&mut self, container_id: &str, _window: &mut Window, cx: &mut Context<'_, Self>) {
    self.container_tab_state.logs_loading = true;
    self.container_tab_state.inspect_loading = true;

    let id = container_id.to_string();
    let id_for_inspect = id.clone();

    // Get max log lines from settings
    let max_log_lines = settings_state(cx).read(cx).settings.max_log_lines;

    // Get tokio handle and docker client before spawning
    let tokio_handle = services::Tokio::runtime_handle();
    let client = services::docker_client();
    let client_for_inspect = client.clone();
    let tokio_handle_for_inspect = tokio_handle.clone();

    // Load logs in background
    cx.spawn(async move |this, cx| {
      let logs = cx
        .background_executor()
        .spawn(async move {
          tokio_handle.block_on(async {
            let guard = client.read().await;
            match guard.as_ref() {
              Some(c) => c
                .container_logs(&id, Some(max_log_lines))
                .await
                .unwrap_or_else(|e| format!("Failed to get logs: {}", e)),
              None => "Docker client not connected".to_string(),
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.container_tab_state.logs = logs;
        this.container_tab_state.logs_loading = false;
        cx.notify();
      });
    })
    .detach();

    // Load inspect data in background
    cx.spawn(async move |this, cx| {
      let inspect = cx
        .background_executor()
        .spawn(async move {
          tokio_handle_for_inspect.block_on(async {
            let guard = client_for_inspect.read().await;
            match guard.as_ref() {
              Some(c) => c
                .inspect_container(&id_for_inspect)
                .await
                .unwrap_or_else(|e| format!("Failed to inspect: {}", e)),
              None => "Docker client not connected".to_string(),
            }
          })
        })
        .await;

      let _ = this.update(cx, |this, cx| {
        this.container_tab_state.inspect = inspect;
        this.container_tab_state.inspect_loading = false;
        cx.notify();
      });
    })
    .detach();
  }

  fn on_refresh_logs(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if let Some(ref container) = self.selected_container.clone() {
      self.load_container_data(&container.id, window, cx);
    }
  }
}

impl Render for ContainersView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Sync editor content with loaded data (only when source data changes, not editor content)
    if let Some(ref editor) = self.logs_editor {
      let logs = &self.container_tab_state.logs;
      if !logs.is_empty() && !self.container_tab_state.logs_loading && self.last_synced_logs != *logs {
        let logs_clone = logs.clone();
        editor.update(cx, |state, cx| {
          state.replace(&logs_clone, window, cx);
        });
        self.last_synced_logs = logs.clone();
      }
    }

    if let Some(ref editor) = self.inspect_editor {
      let inspect = &self.container_tab_state.inspect;
      if !inspect.is_empty() && !self.container_tab_state.inspect_loading && self.last_synced_inspect != *inspect {
        let inspect_clone = inspect.clone();
        editor.update(cx, |state, cx| {
          state.replace(&inspect_clone, window, cx);
        });
        self.last_synced_inspect = inspect.clone();
      }
    }

    // Sync file content editor
    if let Some(ref editor) = self.file_content_editor {
      let content = &self.container_tab_state.file_content;
      if !content.is_empty()
        && !self.container_tab_state.file_content_loading
        && self.last_synced_file_content != *content
      {
        let content_clone = content.clone();
        editor.update(cx, |state, cx| {
          state.replace(&content_clone, window, cx);
        });
        self.last_synced_file_content = content.clone();
      }
    }

    let colors = cx.theme().colors;
    let selected_container = self.selected_container.clone();
    let active_tab = self.active_tab;
    let container_tab_state = self.container_tab_state.clone();
    let terminal_view = self.terminal_view.clone();
    let logs_editor = self.logs_editor.clone();
    let inspect_editor = self.inspect_editor.clone();
    let file_content_editor = self.file_content_editor.clone();

    // Build detail panel
    let detail = ContainerDetail::new()
      .container(selected_container)
      .active_tab(active_tab)
      .container_state(container_tab_state)
      .terminal_view(terminal_view)
      .logs_editor(logs_editor)
      .inspect_editor(inspect_editor)
      .file_content_editor(file_content_editor)
      .on_tab_change(cx.listener(|this, tab: &usize, window, cx| {
        this.on_tab_change(*tab, window, cx);
      }))
      .on_refresh_logs(cx.listener(|this, _: &(), window, cx| {
        this.on_refresh_logs(window, cx);
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
      .on_start(cx.listener(|_this, id: &str, _window, cx| {
        services::start_container(id.to_string(), cx);
      }))
      .on_stop(cx.listener(|_this, id: &str, _window, cx| {
        services::stop_container(id.to_string(), cx);
      }))
      .on_restart(cx.listener(|_this, id: &str, _window, cx| {
        services::restart_container(id.to_string(), cx);
      }))
      .on_delete(cx.listener(|this, id: &str, _window, cx| {
        services::delete_container(id.to_string(), cx);
        this.selected_container = None;
        this.active_tab = 0;
        this.terminal_view = None;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Container list - fixed width with border
        div()
          .w(px(320.))
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .border_r_1()
          .border_color(colors.border)
          .child(self.container_list.clone()),
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
