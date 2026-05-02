use gpui::{Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  scroll::ScrollableElement,
  theme::ActiveTheme,
  v_flex,
};
use std::collections::{HashMap, HashSet};

use crate::assets::AppIcon;
use crate::docker::{ComposeProject, ComposeService, extract_compose_projects};
use crate::services;
use crate::state::{DockerState, StateChanged, docker_state};

/// Docker Compose projects view
pub struct ComposeView {
  docker_state: Entity<DockerState>,
  /// Set of expanded project names
  expanded_projects: HashSet<String>,
  /// Set of projects with YAML viewer open
  yaml_visible: HashSet<String>,
  /// Loaded YAML content per project keyed by project name. `None`
  /// means we tried to load and failed (file missing / read error)
  /// — the UI shows the error string instead of waiting forever.
  yaml_cache: HashMap<String, Result<String, String>>,
}

impl ComposeView {
  pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Subscribe to state changes
    cx.subscribe(&docker_state, |_this, _state, event: &StateChanged, cx| {
      if let StateChanged::ContainersUpdated = event {
        cx.notify();
      }
    })
    .detach();

    Self {
      docker_state,
      expanded_projects: HashSet::new(),
      yaml_visible: HashSet::new(),
      yaml_cache: HashMap::new(),
    }
  }

  fn toggle_project(&mut self, project_name: &str, cx: &mut Context<'_, Self>) {
    if self.expanded_projects.contains(project_name) {
      self.expanded_projects.remove(project_name);
    } else {
      self.expanded_projects.insert(project_name.to_string());
    }
    cx.notify();
  }

  /// Toggle the YAML viewer for `project_name` and trigger a
  /// background read of `path` on first open. Subsequent toggles
  /// reuse `yaml_cache`.
  fn toggle_yaml(&mut self, project_name: &str, path: Option<String>, cx: &mut Context<'_, Self>) {
    if self.yaml_visible.contains(project_name) {
      self.yaml_visible.remove(project_name);
      cx.notify();
      return;
    }
    self.yaml_visible.insert(project_name.to_string());
    if !self.yaml_cache.contains_key(project_name)
      && let Some(path) = path
    {
      let project_owned = project_name.to_string();
      cx.spawn(async move |this, cx| {
        let result = cx
          .background_executor()
          .spawn(async move {
            std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))
          })
          .await;
        let _ = cx.update(|cx| {
          if let Some(this) = this.upgrade() {
            this.update(cx, |this, cx| {
              this.yaml_cache.insert(project_owned, result);
              cx.notify();
            });
          }
        });
      })
      .detach();
    }
    cx.notify();
  }

  fn render_empty(cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;

    div().size_full().flex().items_center().justify_center().child(
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
            .child("No Docker Compose projects found"),
        )
        .child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child("Start a compose project to see it here"),
        ),
    )
  }

  fn render_project(&self, project: &ComposeProject, is_expanded: bool, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let project_name = project.name.clone();
    let project_name_for_toggle = project_name.clone();
    let project_name_for_up = project_name.clone();
    let project_name_for_down = project_name.clone();
    let project_name_for_restart = project_name.clone();
    let project_name_for_watch = project_name.clone();
    let working_dir_up = project.working_dir.clone();
    let config_files_up = project.config_files.clone();
    let working_dir_down = project.working_dir.clone();
    let config_files_down = project.config_files.clone();
    let working_dir_restart = project.working_dir.clone();
    let config_files_restart = project.config_files.clone();
    let working_dir_watch = project.working_dir.clone();
    let config_files_watch = project.config_files.clone();
    let project_name_for_yaml = project_name.clone();
    let yaml_path = project.config_files.first().cloned();
    let yaml_visible = self.yaml_visible.contains(&project_name);
    let yaml_content = self.yaml_cache.get(&project_name).cloned();

    let status_color = if project.is_all_running() {
      colors.success
    } else if project.is_all_stopped() {
      colors.muted_foreground
    } else {
      colors.warning
    };

    v_flex()
            .w_full()
            // Project header row
            .child(
                h_flex()
                    .id(SharedString::from(format!("project-{project_name}")))
                    .w_full()
                    .h(px(44.))
                    .px(px(16.))
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|el| el.bg(colors.list_hover))
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        this.toggle_project(&project_name_for_toggle, cx);
                    }))
                    // Expand/collapse chevron
                    .child(
                        Icon::new(if is_expanded {
                            AppIcon::ChevronDown
                        } else {
                            AppIcon::ChevronRight
                        })
                        .size(px(14.))
                        .text_color(colors.muted_foreground),
                    )
                    // Project icon
                    .child(
                        Icon::new(AppIcon::Container)
                            .size(px(18.))
                            .text_color(colors.foreground),
                    )
                    // Project name
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.foreground)
                            .child(project_name.clone()),
                    )
                    // Status badge
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .bg(status_color.opacity(0.15))
                            .text_xs()
                            .text_color(status_color)
                            .child(project.status_display()),
                    )
                    // Action buttons (shown on hover would be nice, but always visible for now)
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .child(
                                Button::new(SharedString::from(format!("up-{}", project_name_for_up.clone())))
                                    .icon(AppIcon::Play)
                                    .xsmall()
                                    .ghost()
                                    .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                        services::compose_up(
                                            project_name_for_up.clone(),
                                            working_dir_up.clone(),
                                            config_files_up.clone(),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("down-{}", project_name_for_down.clone())))
                                    .icon(AppIcon::Stop)
                                    .xsmall()
                                    .ghost()
                                    .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                        services::compose_down(
                                            project_name_for_down.clone(),
                                            working_dir_down.clone(),
                                            config_files_down.clone(),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("restart-{}", project_name_for_restart.clone())))
                                    .icon(AppIcon::Restart)
                                    .xsmall()
                                    .ghost()
                                    .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                        services::compose_restart(
                                            project_name_for_restart.clone(),
                                            working_dir_restart.clone(),
                                            config_files_restart.clone(),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("watch-{}", project_name_for_watch.clone())))
                                    .label("Watch")
                                    .xsmall()
                                    .ghost()
                                    .on_click(cx.listener(move |_this, _ev, window, cx| {
                                        crate::ui::dialogs::open_compose_watch_dialog(
                                            project_name_for_watch.clone(),
                                            working_dir_watch.clone(),
                                            config_files_watch.clone(),
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("yaml-{}", project_name_for_yaml.clone())))
                                    .label("YAML")
                                    .xsmall()
                                    .when(yaml_visible, Button::primary)
                                    .when(!yaml_visible, ButtonVariants::ghost)
                                    .on_click(cx.listener({
                                        let path = yaml_path.clone();
                                        let name = project_name_for_yaml.clone();
                                        move |this, _ev, _window, cx| {
                                            this.toggle_yaml(&name, path.clone(), cx);
                                        }
                                    })),
                            ),
                    ),
            )
            // Services list (when expanded)
            .when_some(
                if is_expanded {
                    Some(
                        project
                            .services
                            .iter()
                            .map(|service| Self::render_service(service, cx).into_any_element())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                },
                ParentElement::children,
            )
            .when(is_expanded && yaml_visible, move |el| {
                el.child(Self::render_yaml_block(yaml_path.as_deref(), yaml_content, cx))
            })
  }

  fn render_yaml_block(
    yaml_path: Option<&str>,
    yaml_content: Option<Result<String, String>>,
    cx: &mut Context<'_, Self>,
  ) -> impl IntoElement {
    let colors = cx.theme().colors;
    let header_text = yaml_path.map_or_else(|| "compose YAML".to_string(), str::to_string);
    let body: gpui::AnyElement = match yaml_content {
      Some(Ok(text)) => div()
        .w_full()
        .max_h(px(360.))
        .overflow_y_scrollbar()
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(text)
        .into_any_element(),
      Some(Err(err)) => div()
        .p(px(12.))
        .text_xs()
        .text_color(colors.danger)
        .child(err)
        .into_any_element(),
      None => div()
        .p(px(12.))
        .text_xs()
        .text_color(colors.muted_foreground)
        .child("Loading...")
        .into_any_element(),
    };
    v_flex()
      .w_full()
      .pl(px(56.))
      .pr(px(16.))
      .py(px(8.))
      .gap(px(4.))
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child(header_text),
      )
      .child(
        div()
          .w_full()
          .bg(colors.background)
          .rounded(px(6.))
          .border_1()
          .border_color(colors.border)
          .child(body),
      )
  }

  fn render_service(service: &ComposeService, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;

    let status_color = if service.state.is_running() {
      colors.success
    } else {
      colors.muted_foreground
    };

    h_flex()
            .id(SharedString::from(format!("service-{}", service.container_id)))
            .w_full()
            .h(px(36.))
            .pl(px(56.)) // Indent for child rows
            .pr(px(16.))
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .hover(|el| el.bg(colors.list_hover))
            .on_click(cx.listener(move |_this, _ev, _window, cx| {
                // Navigate to containers view
                services::set_view(crate::state::CurrentView::Containers, cx);
            }))
            // Service name
            .child(
                div()
                    .w(px(150.))
                    .text_sm()
                    .text_color(colors.foreground)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(service.name.clone()),
            )
            // Image name
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(format!("({})", service.image)),
            )
            // Status indicator
            .child(
                div()
                    .w(px(8.))
                    .h(px(8.))
                    .rounded_full()
                    .bg(status_color),
            )
            // Status text
            .child(
                div()
                    .w(px(80.))
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(service.state.to_string()),
            )
  }
}

impl Render for ComposeView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;

    // Get containers and extract compose projects
    let containers = self.docker_state.read(cx).containers.clone();
    let projects = extract_compose_projects(&containers);

    div()
            .size_full()
            .bg(colors.background)
            .flex()
            .flex_col()
            // Header
            .child(
                h_flex()
                    .w_full()
                    .h(px(52.))
                    .px(px(16.))
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        Label::new("Docker Compose")
                            .text_color(colors.foreground)
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                Button::new("refresh-compose")
                                    .icon(AppIcon::Restart)
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        services::refresh_containers(cx);
                                    })),
                            ),
                    ),
            )
            // Content
            .child({
                // Pre-render content to avoid closure escaping issues
                let content = if projects.is_empty() {
                    Self::render_empty(cx).into_any_element()
                } else {
                    v_flex()
                        .w_full()
                        .children(projects.iter().map(|project| {
                            let is_expanded = self.expanded_projects.contains(&project.name);
                            self.render_project(project, is_expanded, cx).into_any_element()
                        }))
                        .into_any_element()
                };

                div()
                    .id("compose-scroll")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(content)
            })
  }
}
