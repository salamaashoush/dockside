use gpui::{Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Disableable, IconName, IndexPath, Sizable, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  label::Label,
  scroll::ScrollableElement,
  select::{Select, SelectItem, SelectState},
  theme::{ActiveTheme, Theme, ThemeRegistry},
  v_flex,
};

use crate::assets::AppIcon;
use crate::colima::ColimaClient;
use crate::state::{ExternalEditor, SettingsChanged, SettingsState, ThemeName, settings_state};

/// Theme wrapper for Select
#[derive(Debug, Clone)]
struct ThemeOption {
  theme: ThemeName,
  label: SharedString,
}

impl ThemeOption {
  fn new(theme: ThemeName) -> Self {
    let label = SharedString::from(theme.display_name().to_string());
    Self { theme, label }
  }

  fn all() -> Vec<Self> {
    ThemeName::all().into_iter().map(Self::new).collect()
  }
}

impl SelectItem for ThemeOption {
  type Value = ThemeOption;

  fn value(&self) -> &Self::Value {
    self
  }

  fn title(&self) -> SharedString {
    self.label.clone()
  }
}

/// External editor wrapper for Select
#[derive(Debug, Clone)]
struct EditorOption {
  editor: ExternalEditor,
  label: SharedString,
}

impl EditorOption {
  fn new(editor: ExternalEditor) -> Self {
    let label = SharedString::from(editor.display_name().to_string());
    Self { editor, label }
  }

  fn all() -> Vec<Self> {
    ExternalEditor::all().into_iter().map(Self::new).collect()
  }
}

impl SelectItem for EditorOption {
  type Value = EditorOption;

  fn value(&self) -> &Self::Value {
    self
  }

  fn title(&self) -> SharedString {
    self.label.clone()
  }
}

/// Settings view - full page settings
pub struct SettingsView {
  settings_state: Entity<SettingsState>,
  // Form state
  theme_select: Option<Entity<SelectState<Vec<ThemeOption>>>>,
  editor_select: Option<Entity<SelectState<Vec<EditorOption>>>>,
  docker_socket_input: Option<Entity<InputState>>,
  colima_profile_input: Option<Entity<InputState>>,
  container_refresh_input: Option<Entity<InputState>>,
  stats_refresh_input: Option<Entity<InputState>>,
  log_lines_input: Option<Entity<InputState>>,
  font_size_input: Option<Entity<InputState>>,
  initialized: bool,
  last_theme_index: Option<usize>,
  // Colima cache state
  cache_size: String,
  is_pruning: bool,
}

impl SettingsView {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let settings_state = settings_state(cx);

    // Get initial cache size
    let cache_size = ColimaClient::cache_size().unwrap_or_else(|_| "Unknown".to_string());

    Self {
      settings_state,
      theme_select: None,
      editor_select: None,
      docker_socket_input: None,
      colima_profile_input: None,
      container_refresh_input: None,
      stats_refresh_input: None,
      log_lines_input: None,
      font_size_input: None,
      initialized: false,
      last_theme_index: None,
      cache_size,
      is_pruning: false,
    }
  }

  fn ensure_initialized(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    if self.initialized {
      return;
    }

    let settings = self.settings_state.read(cx).settings.clone();

    // Find the current theme index
    let themes = ThemeOption::all();
    let current_theme_idx = themes.iter().position(|t| t.theme == settings.theme).unwrap_or(0);
    self.last_theme_index = Some(current_theme_idx);

    let theme_select = cx.new(|cx| SelectState::new(themes, Some(IndexPath::new(current_theme_idx)), window, cx));

    // Subscribe to theme selection changes to apply theme preview
    cx.subscribe(
      &theme_select,
      |this, select, _event: &gpui_component::select::SelectEvent<Vec<ThemeOption>>, cx| {
        let current_index = select.read(cx).selected_index(cx).map(|idx| idx.row);
        if current_index != this.last_theme_index {
          this.last_theme_index = current_index;
          // Apply the theme immediately for preview
          if let Some(theme_opt) = select.read(cx).selected_value() {
            let theme_name = SharedString::from(theme_opt.theme.theme_name().to_string());
            if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
              Theme::global_mut(cx).apply_config(&theme_config);
            }
          }
        }
      },
    )
    .detach();

    self.theme_select = Some(theme_select);

    // Initialize editor select
    let editors = EditorOption::all();
    let current_editor_idx = editors
      .iter()
      .position(|e| e.editor == settings.external_editor)
      .unwrap_or(0);
    let editor_select = cx.new(|cx| SelectState::new(editors, Some(IndexPath::new(current_editor_idx)), window, cx));
    self.editor_select = Some(editor_select);

    self.docker_socket_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("Default socket")
        .default_value(&settings.docker_socket)
    }));

    self.colima_profile_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("default")
        .default_value(&settings.default_colima_profile)
    }));

    self.container_refresh_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.container_refresh_interval.to_string())));

    self.stats_refresh_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.stats_refresh_interval.to_string())));

    self.log_lines_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.max_log_lines.to_string())));

    self.font_size_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.terminal_font_size.to_string())));

    self.initialized = true;
  }

  fn reset_to_defaults(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    // Get default theme to apply it
    let default_settings = crate::state::AppSettings::default();
    let default_theme = default_settings.theme.clone();

    // Apply the default theme immediately
    let theme_name = SharedString::from(default_theme.theme_name().to_string());
    if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
      Theme::global_mut(cx).apply_config(&theme_config);
    }

    // Reset settings state to defaults
    self.settings_state.update(cx, |state, cx| {
      state.settings = default_settings;
      let _ = state.settings.save();
      cx.emit(SettingsChanged::SettingsUpdated);
    });

    // Reinitialize the form with default values
    self.initialized = false;
    self.ensure_initialized(window, cx);
    cx.notify();
  }

  fn apply_settings(&mut self, cx: &mut Context<'_, Self>) {
    let Some(docker_socket_input) = &self.docker_socket_input else {
      return;
    };
    let Some(colima_profile_input) = &self.colima_profile_input else {
      return;
    };
    let Some(container_refresh_input) = &self.container_refresh_input else {
      return;
    };
    let Some(stats_refresh_input) = &self.stats_refresh_input else {
      return;
    };
    let Some(log_lines_input) = &self.log_lines_input else {
      return;
    };
    let Some(font_size_input) = &self.font_size_input else {
      return;
    };
    let Some(theme_select) = &self.theme_select else {
      return;
    };
    let Some(editor_select) = &self.editor_select else {
      return;
    };

    // Get current values from inputs
    let docker_socket = docker_socket_input.read(cx).text().to_string();
    let colima_profile = colima_profile_input.read(cx).text().to_string();
    let container_refresh = container_refresh_input
      .read(cx)
      .text()
      .to_string()
      .parse::<u64>()
      .unwrap_or(5);
    let stats_refresh = stats_refresh_input
      .read(cx)
      .text()
      .to_string()
      .parse::<u64>()
      .unwrap_or(2);
    let log_lines = log_lines_input
      .read(cx)
      .text()
      .to_string()
      .parse::<usize>()
      .unwrap_or(1000);
    let font_size = font_size_input
      .read(cx)
      .text()
      .to_string()
      .parse::<f32>()
      .unwrap_or(14.0);

    // Get selected theme
    let theme = theme_select
      .read(cx)
      .selected_value()
      .map(|opt| opt.theme.clone())
      .unwrap_or_default();

    // Get selected external editor
    let external_editor = editor_select
      .read(cx)
      .selected_value()
      .map(|opt| opt.editor.clone())
      .unwrap_or_default();

    // Apply the theme immediately using ThemeRegistry
    let theme_name = SharedString::from(theme.theme_name().to_string());
    if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
      Theme::global_mut(cx).apply_config(&theme_config);
    }

    // Update settings state
    self.settings_state.update(cx, |state, cx| {
      let old_theme = state.settings.theme.clone();
      state.settings.docker_socket = docker_socket;
      state.settings.default_colima_profile = colima_profile;
      state.settings.container_refresh_interval = container_refresh;
      state.settings.stats_refresh_interval = stats_refresh;
      state.settings.max_log_lines = log_lines;
      state.settings.terminal_font_size = font_size;
      state.settings.theme = theme.clone();
      state.settings.external_editor = external_editor;
      let _ = state.settings.save();

      if old_theme != theme {
        cx.emit(SettingsChanged::ThemeChanged);
      }
      cx.emit(SettingsChanged::SettingsUpdated);
    });

    cx.notify();
  }

  fn render_section_header(title: &str, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    div()
      .w_full()
      .py(px(12.))
      .mt(px(16.))
      .mb(px(4.))
      .text_xs()
      .font_weight(gpui::FontWeight::SEMIBOLD)
      .text_color(colors.muted_foreground)
      .child(title.to_uppercase())
  }

  fn render_form_row(
    label: &'static str,
    description: &'static str,
    content: impl IntoElement,
    cx: &Context<'_, Self>,
  ) -> impl IntoElement {
    let colors = cx.theme().colors;
    h_flex()
      .w_full()
      .py(px(12.))
      .justify_between()
      .items_center()
      .gap(px(16.))
      .border_b_1()
      .border_color(colors.border.opacity(0.5))
      .child(
        v_flex()
          .flex_1()
          .gap(px(2.))
          .child(Label::new(label).text_color(colors.foreground))
          .child(div().text_xs().text_color(colors.muted_foreground).child(description)),
      )
      .child(div().w(px(250.)).child(content))
  }

  fn render_colima_section(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let cache_size = self.cache_size.clone();
    let is_pruning = self.is_pruning;

    v_flex()
      .w_full()
      .gap(px(4.))
      // Cache row
      .child(
        h_flex()
          .w_full()
          .py(px(12.))
          .justify_between()
          .items_center()
          .gap(px(16.))
          .border_b_1()
          .border_color(colors.border.opacity(0.5))
          .child(
            v_flex()
              .flex_1()
              .gap(px(2.))
              .child(Label::new("Cache Size").text_color(colors.foreground))
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("Downloaded VM images and assets"),
              ),
          )
          .child(
            h_flex().gap(px(8.)).items_center().child(
              div()
                .text_sm()
                .text_color(colors.foreground)
                .child(cache_size),
            )
            .child(
              Button::new("prune-cache")
                .label(if is_pruning { "Pruning..." } else { "Prune" })
                .small()
                .ghost()
                .disabled(is_pruning)
                .on_click(cx.listener(|this, _ev, _window, cx| {
                  this.prune_cache(cx);
                })),
            ),
          ),
      )
      // Template row
      .child(
        h_flex()
          .w_full()
          .py(px(12.))
          .justify_between()
          .items_center()
          .gap(px(16.))
          .border_b_1()
          .border_color(colors.border.opacity(0.5))
          .child(
            v_flex()
              .flex_1()
              .gap(px(2.))
              .child(Label::new("Default Template").text_color(colors.foreground))
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child("Default configuration for new Colima VMs"),
              ),
          )
          .child(
            Button::new("edit-template")
              .label("Edit")
              .icon(AppIcon::Edit)
              .small()
              .ghost()
              .on_click(cx.listener(|this, _ev, window, cx| {
                this.open_template_editor(window, cx);
              })),
          ),
      )
  }

  fn prune_cache(&mut self, cx: &mut Context<'_, Self>) {
    if self.is_pruning {
      return;
    }
    self.is_pruning = true;
    cx.notify();

    cx.spawn(async move |this, cx| {
      let result = cx
        .background_executor()
        .spawn(async { ColimaClient::prune(false, true) })
        .await;

      cx.update(|cx| {
        this.update(cx, |this, cx| {
          this.is_pruning = false;
          match result {
            Ok(_) => {
              // Refresh cache size after prune
              this.cache_size = ColimaClient::cache_size().unwrap_or_else(|_| "Unknown".to_string());
            }
            Err(e) => {
              tracing::error!("Failed to prune cache: {e}");
            }
          }
          cx.notify();
        })
      })
    })
    .detach();
  }

  #[allow(clippy::unused_self)]
  fn open_template_editor(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
    // Read current template
    let template_content = ColimaClient::read_template().unwrap_or_default();
    let template_input = cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("yaml")
        .line_number(true)
        .default_value(template_content)
    });

    window.open_dialog(cx, move |dialog, _window, cx| {
      let colors = cx.theme().colors;
      let template_clone = template_input.clone();

      dialog
        .title("Edit Default Template")
        .width(px(700.))
        .child(
          v_flex()
            .gap(px(8.))
            .child(
              div()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child("This template is used as the default configuration when creating new Colima VMs."),
            )
            .child(div().h(px(400.)).child(Input::new(&template_input).w_full().h_full())),
        )
        .footer(move |_, _, _, _| {
          let template_for_save = template_clone.clone();
          vec![
            Button::new("cancel-template")
              .label("Cancel")
              .ghost()
              .on_click(|_, window, cx| {
                window.close_dialog(cx);
              }),
            Button::new("save-template")
              .label("Save")
              .primary()
              .on_click(move |_, window, cx| {
                let content = template_for_save.read(cx).text().to_string();
                if let Err(e) = ColimaClient::write_template(&content) {
                  tracing::error!("Failed to save template: {e}");
                }
                window.close_dialog(cx);
              }),
          ]
        })
    });
  }
}

impl Render for SettingsView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_initialized(window, cx);

    let colors = cx.theme().colors;

    let content = if let (
      Some(theme_select),
      Some(editor_select),
      Some(docker_socket_input),
      Some(colima_profile_input),
      Some(container_refresh_input),
      Some(stats_refresh_input),
      Some(log_lines_input),
      Some(font_size_input),
    ) = (
      &self.theme_select,
      &self.editor_select,
      &self.docker_socket_input,
      &self.colima_profile_input,
      &self.container_refresh_input,
      &self.stats_refresh_input,
      &self.log_lines_input,
      &self.font_size_input,
    ) {
      v_flex()
                .w_full()
                .max_w(px(700.))
                .pb(px(48.)) // Bottom padding for scroll visibility
                // Appearance section
                .child(Self::render_section_header("Appearance", cx))
                .child(Self::render_form_row(
                    "Theme",
                    "Choose the color theme for the application",
                    Select::new(theme_select).w_full().small(),
                    cx,
                ))
                // Editor section
                .child(Self::render_section_header("External Editor", cx))
                .child(Self::render_form_row(
                    "Editor",
                    "Editor for opening container files remotely",
                    Select::new(editor_select).w_full().small(),
                    cx,
                ))
                // Docker section
                .child(Self::render_section_header("Docker", cx))
                .child(Self::render_form_row(
                    "Docker Socket",
                    "Path to Docker socket (leave empty for default)",
                    Input::new(docker_socket_input).small().w_full(),
                    cx,
                ))
                .child(Self::render_form_row(
                    "Default Colima Profile",
                    "Name of the default Colima VM profile",
                    Input::new(colima_profile_input).small().w_full(),
                    cx,
                ))
                // Refresh intervals section
                .child(Self::render_section_header("Refresh Intervals", cx))
                .child(Self::render_form_row(
                    "Container Refresh",
                    "How often to refresh container list (seconds)",
                    Input::new(container_refresh_input).small().w_full(),
                    cx,
                ))
                .child(Self::render_form_row(
                    "Stats Refresh",
                    "How often to refresh stats (seconds)",
                    Input::new(stats_refresh_input).small().w_full(),
                    cx,
                ))
                // Display section
                .child(Self::render_section_header("Display", cx))
                .child(Self::render_form_row(
                    "Max Log Lines",
                    "Maximum number of log lines to display",
                    Input::new(log_lines_input).small().w_full(),
                    cx,
                ))
                .child(Self::render_form_row(
                    "Terminal Font Size",
                    "Font size for terminal views (pixels)",
                    Input::new(font_size_input).small().w_full(),
                    cx,
                ))
                // Colima section
                .child(Self::render_section_header("Colima", cx))
                .child(self.render_colima_section(cx))
    } else {
      v_flex().child("Loading...")
    };

    div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            // Header with actions
            .child(
                h_flex()
                    .w_full()
                    .h(px(52.))
                    .flex_shrink_0()
                    .px(px(16.))
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        Label::new("Settings")
                            .text_color(colors.foreground)
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                Button::new("reset-defaults")
                                    .icon(AppIcon::Restart)
                                    .label("Reset")
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.reset_to_defaults(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-settings")
                                    .icon(IconName::Check)
                                    .label("Save")
                                    .small()
                                    .primary()
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.apply_settings(cx);
                                    })),
                            ),
                    ),
            )
            // Scrollable content
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .px(px(24.))
                    .py(px(8.))
                    .child(content),
            )
  }
}
