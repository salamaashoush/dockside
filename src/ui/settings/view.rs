use gpui::{Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Disableable, Icon, IconName, IndexPath, Sizable, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputEvent, InputState},
  label::Label,
  scroll::ScrollableElement,
  select::{Select, SelectItem, SelectState},
  switch::Switch,
  theme::{ActiveTheme, Theme, ThemeRegistry},
  v_flex,
};

use crate::assets::AppIcon;
use crate::colima::ColimaClient;
use crate::state::{ExternalEditor, SettingsChanged, SettingsState, TerminalCursorStyle, ThemeName, settings_state};

// ============================================================================
// Select item wrappers
// ============================================================================

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
  type Value = Self;
  fn value(&self) -> &Self::Value {
    self
  }
  fn title(&self) -> SharedString {
    self.label.clone()
  }
}

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
  type Value = Self;
  fn value(&self) -> &Self::Value {
    self
  }
  fn title(&self) -> SharedString {
    self.label.clone()
  }
}

#[derive(Debug, Clone)]
struct CursorStyleOption {
  style: TerminalCursorStyle,
  label: SharedString,
}

impl CursorStyleOption {
  fn new(style: TerminalCursorStyle) -> Self {
    Self {
      label: SharedString::from(style.display_name().to_string()),
      style,
    }
  }
  fn all() -> Vec<Self> {
    TerminalCursorStyle::all().into_iter().map(Self::new).collect()
  }
}

impl SelectItem for CursorStyleOption {
  type Value = Self;
  fn value(&self) -> &Self::Value {
    self
  }
  fn title(&self) -> SharedString {
    self.label.clone()
  }
}

// ============================================================================
// Categories
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
  General,
  Appearance,
  Terminal,
  Docker,
  Kubernetes,
  Colima,
  Editor,
}

impl Category {
  const ALL: &'static [Self] = &[
    Self::General,
    Self::Appearance,
    Self::Terminal,
    Self::Docker,
    Self::Kubernetes,
    Self::Colima,
    Self::Editor,
  ];

  fn label(self) -> &'static str {
    match self {
      Self::General => "General",
      Self::Appearance => "Appearance",
      Self::Terminal => "Terminal",
      Self::Docker => "Docker",
      Self::Kubernetes => "Kubernetes",
      Self::Colima => "Colima",
      Self::Editor => "Editor",
    }
  }

  fn icon(self) -> IconName {
    match self {
      Self::General => IconName::Settings,
      Self::Appearance => IconName::Palette,
      Self::Terminal => IconName::SquareTerminal,
      Self::Docker => IconName::Globe,
      Self::Kubernetes => IconName::LayoutDashboard,
      Self::Colima => IconName::Frame,
      Self::Editor => IconName::Inspector,
    }
  }
}

// ============================================================================
// View
// ============================================================================

pub struct SettingsView {
  settings_state: Entity<SettingsState>,
  active: Category,
  // Form state — each input is built lazily on first render of the category.
  theme_select: Option<Entity<SelectState<Vec<ThemeOption>>>>,
  editor_select: Option<Entity<SelectState<Vec<EditorOption>>>>,
  cursor_style_select: Option<Entity<SelectState<Vec<CursorStyleOption>>>>,
  docker_socket_input: Option<Entity<InputState>>,
  default_platform_input: Option<Entity<InputState>>,
  colima_profile_input: Option<Entity<InputState>>,
  container_refresh_input: Option<Entity<InputState>>,
  stats_refresh_input: Option<Entity<InputState>>,
  log_lines_input: Option<Entity<InputState>>,
  font_size_input: Option<Entity<InputState>>,
  line_height_input: Option<Entity<InputState>>,
  font_family_input: Option<Entity<InputState>>,
  scrollback_lines_input: Option<Entity<InputState>>,
  kubeconfig_input: Option<Entity<InputState>>,
  default_namespace_input: Option<Entity<InputState>>,
  colima_cpus_input: Option<Entity<InputState>>,
  colima_memory_input: Option<Entity<InputState>>,
  colima_disk_input: Option<Entity<InputState>>,
  initialized: bool,
  last_theme_index: Option<usize>,
  cache_size: String,
  is_pruning: bool,
}

impl SettingsView {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let settings_state = settings_state(cx);
    let cache_size = ColimaClient::cache_size().unwrap_or_else(|_| "Unknown".to_string());

    Self {
      settings_state,
      active: Category::General,
      theme_select: None,
      editor_select: None,
      cursor_style_select: None,
      docker_socket_input: None,
      default_platform_input: None,
      colima_profile_input: None,
      container_refresh_input: None,
      stats_refresh_input: None,
      log_lines_input: None,
      font_size_input: None,
      line_height_input: None,
      font_family_input: None,
      scrollback_lines_input: None,
      kubeconfig_input: None,
      default_namespace_input: None,
      colima_cpus_input: None,
      colima_memory_input: None,
      colima_disk_input: None,
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

    // Theme select with live preview.
    let themes = ThemeOption::all();
    let current_theme_idx = themes.iter().position(|t| t.theme == settings.theme).unwrap_or(0);
    self.last_theme_index = Some(current_theme_idx);
    let theme_select = cx.new(|cx| SelectState::new(themes, Some(IndexPath::new(current_theme_idx)), window, cx));
    cx.subscribe(
      &theme_select,
      |this, select, _event: &gpui_component::select::SelectEvent<Vec<ThemeOption>>, cx| {
        let current_index = select.read(cx).selected_index(cx).map(|idx| idx.row);
        if current_index != this.last_theme_index {
          this.last_theme_index = current_index;
          let chosen = select.read(cx).selected_value().map(|opt| opt.theme.clone());
          if let Some(chosen) = chosen {
            let theme_name = SharedString::from(chosen.theme_name().to_string());
            if let Some(cfg) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
              Theme::global_mut(cx).apply_config(&cfg);
            }
            this.settings_state.update(cx, |state, cx| {
              state.settings.theme = chosen;
              let _ = state.settings.save();
              cx.emit(SettingsChanged::ThemeChanged);
              cx.emit(SettingsChanged::SettingsUpdated);
            });
          }
        }
      },
    )
    .detach();
    self.theme_select = Some(theme_select);

    // Editor select.
    let editors = EditorOption::all();
    let current_editor_idx = editors
      .iter()
      .position(|e| e.editor == settings.external_editor)
      .unwrap_or(0);
    let editor_select = cx.new(|cx| SelectState::new(editors, Some(IndexPath::new(current_editor_idx)), window, cx));
    cx.subscribe(
      &editor_select,
      |this, select, _event: &gpui_component::select::SelectEvent<Vec<EditorOption>>, cx| {
        if let Some(opt) = select.read(cx).selected_value() {
          let chosen = opt.editor.clone();
          this.settings_state.update(cx, |state, cx| {
            state.settings.external_editor = chosen;
            let _ = state.settings.save();
            cx.emit(SettingsChanged::SettingsUpdated);
          });
        }
      },
    )
    .detach();
    self.editor_select = Some(editor_select);

    // Cursor style.
    let cursor_styles = CursorStyleOption::all();
    let current_cursor_idx = cursor_styles
      .iter()
      .position(|c| c.style == settings.terminal_cursor_style)
      .unwrap_or(0);
    let cursor_select =
      cx.new(|cx| SelectState::new(cursor_styles, Some(IndexPath::new(current_cursor_idx)), window, cx));
    cx.subscribe(
      &cursor_select,
      |this, select, _event: &gpui_component::select::SelectEvent<Vec<CursorStyleOption>>, cx| {
        if let Some(opt) = select.read(cx).selected_value() {
          let chosen = opt.style;
          this.settings_state.update(cx, |state, cx| {
            state.settings.terminal_cursor_style = chosen;
            let _ = state.settings.save();
            cx.emit(SettingsChanged::SettingsUpdated);
          });
        }
      },
    )
    .detach();
    self.cursor_style_select = Some(cursor_select);

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
    self.line_height_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.terminal_line_height.to_string())));
    self.scrollback_lines_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.terminal_scrollback_lines.to_string())));
    self.font_family_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("System mono")
        .default_value(&settings.terminal_font_family)
    }));
    self.default_platform_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("linux/amd64")
        .default_value(&settings.default_pull_platform)
    }));
    self.kubeconfig_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("~/.kube/config")
        .default_value(&settings.kubeconfig_path)
    }));
    self.default_namespace_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("default")
        .default_value(&settings.default_namespace)
    }));
    self.colima_cpus_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.colima_default_cpus.to_string())));
    self.colima_memory_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.colima_default_memory_gb.to_string())));
    self.colima_disk_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.colima_default_disk_gb.to_string())));

    // Auto-save every text input on change. No "Apply" button anywhere.
    let inputs = [
      self.docker_socket_input.clone(),
      self.colima_profile_input.clone(),
      self.container_refresh_input.clone(),
      self.stats_refresh_input.clone(),
      self.log_lines_input.clone(),
      self.font_size_input.clone(),
      self.line_height_input.clone(),
      self.scrollback_lines_input.clone(),
      self.font_family_input.clone(),
      self.default_platform_input.clone(),
      self.kubeconfig_input.clone(),
      self.default_namespace_input.clone(),
      self.colima_cpus_input.clone(),
      self.colima_memory_input.clone(),
      self.colima_disk_input.clone(),
    ];
    for input in inputs.into_iter().flatten() {
      cx.subscribe(&input, |this, _state, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
          this.save_text_inputs(cx);
        }
      })
      .detach();
    }

    self.initialized = true;
  }

  fn save_text_inputs(&mut self, cx: &mut Context<'_, Self>) {
    // Apply every text input back into the settings state. Called on
    // every blur / programmatic flush; cheap because the state object
    // is small.
    let docker_socket = self
      .docker_socket_input
      .as_ref()
      .map(|i| i.read(cx).text().to_string())
      .unwrap_or_default();
    let colima_profile = self
      .colima_profile_input
      .as_ref()
      .map(|i| i.read(cx).text().to_string())
      .unwrap_or_default();
    let container_refresh = self
      .container_refresh_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u64>().ok())
      .unwrap_or(5);
    let stats_refresh = self
      .stats_refresh_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u64>().ok())
      .unwrap_or(2);
    let log_lines = self
      .log_lines_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<usize>().ok())
      .unwrap_or(1000);
    let font_size = self
      .font_size_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<f32>().ok())
      .unwrap_or(14.0);
    let line_height = self
      .line_height_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<f32>().ok())
      .unwrap_or(1.4);
    let scrollback_lines = self
      .scrollback_lines_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<usize>().ok())
      .unwrap_or(10_000);

    let font_family = self
      .font_family_input
      .as_ref()
      .map(|i| i.read(cx).text().to_string())
      .unwrap_or_default();
    let default_platform = self
      .default_platform_input
      .as_ref()
      .map(|i| i.read(cx).text().to_string())
      .unwrap_or_default();
    let kubeconfig = self
      .kubeconfig_input
      .as_ref()
      .map(|i| i.read(cx).text().to_string())
      .unwrap_or_default();
    let default_namespace = {
      let v = self
        .default_namespace_input
        .as_ref()
        .map(|i| i.read(cx).text().to_string())
        .unwrap_or_default();
      if v.is_empty() { "default".to_string() } else { v }
    };
    let colima_cpus = self
      .colima_cpus_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u32>().ok())
      .unwrap_or(2);
    let colima_memory = self
      .colima_memory_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u32>().ok())
      .unwrap_or(4);
    let colima_disk = self
      .colima_disk_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u32>().ok())
      .unwrap_or(60);

    self.settings_state.update(cx, |state, cx| {
      state.settings.docker_socket = docker_socket;
      state.settings.default_colima_profile = colima_profile;
      state.settings.container_refresh_interval = container_refresh;
      state.settings.stats_refresh_interval = stats_refresh;
      state.settings.max_log_lines = log_lines;
      state.settings.terminal_font_size = font_size;
      state.settings.terminal_line_height = line_height;
      state.settings.terminal_scrollback_lines = scrollback_lines;
      state.settings.terminal_font_family = font_family;
      state.settings.default_pull_platform = default_platform;
      state.settings.kubeconfig_path = kubeconfig;
      state.settings.default_namespace = default_namespace;
      state.settings.colima_default_cpus = colima_cpus;
      state.settings.colima_default_memory_gb = colima_memory;
      state.settings.colima_default_disk_gb = colima_disk;
      let _ = state.settings.save();
      cx.emit(SettingsChanged::SettingsUpdated);
    });
  }

  fn reset_to_defaults(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let default_settings = crate::state::AppSettings::default();
    let default_theme = default_settings.theme.clone();
    let theme_name = SharedString::from(default_theme.theme_name().to_string());
    if let Some(cfg) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
      Theme::global_mut(cx).apply_config(&cfg);
    }
    self.settings_state.update(cx, |state, cx| {
      state.settings = default_settings;
      let _ = state.settings.save();
      cx.emit(SettingsChanged::SettingsUpdated);
    });
    // Rebuild form controls so values reflect defaults.
    self.initialized = false;
    self.theme_select = None;
    self.editor_select = None;
    self.cursor_style_select = None;
    self.docker_socket_input = None;
    self.colima_profile_input = None;
    self.container_refresh_input = None;
    self.stats_refresh_input = None;
    self.log_lines_input = None;
    self.font_size_input = None;
    self.line_height_input = None;
    self.scrollback_lines_input = None;
    self.font_family_input = None;
    self.default_platform_input = None;
    self.kubeconfig_input = None;
    self.default_namespace_input = None;
    self.colima_cpus_input = None;
    self.colima_memory_input = None;
    self.colima_disk_input = None;
    self.ensure_initialized(window, cx);
    cx.notify();
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
              this.cache_size = ColimaClient::cache_size().unwrap_or_else(|_| "Unknown".to_string());
            }
            Err(e) => tracing::error!("Failed to prune cache: {e}"),
          }
          cx.notify();
        })
      })
    })
    .detach();
  }

  // ==========================================================================
  // Layout primitives
  // ==========================================================================

  /// Section header — small uppercase muted label above a group of rows.
  fn render_section(title: &'static str, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    div()
      .w_full()
      .pt(px(20.))
      .pb(px(8.))
      .text_xs()
      .font_weight(gpui::FontWeight::SEMIBOLD)
      .text_color(colors.muted_foreground)
      .child(title.to_uppercase())
  }

  /// Bare row — label + description on the left, control on the right.
  fn render_row(
    label: &'static str,
    description: &'static str,
    control: impl IntoElement,
    cx: &Context<'_, Self>,
  ) -> impl IntoElement {
    let colors = cx.theme().colors;
    h_flex()
      .w_full()
      .py(px(14.))
      .gap(px(24.))
      .items_center()
      .justify_between()
      .border_b_1()
      .border_color(colors.border.opacity(0.4))
      .child(
        v_flex()
          .flex_1()
          .gap(px(2.))
          .child(Label::new(label).text_color(colors.foreground))
          .child(div().text_xs().text_color(colors.muted_foreground).child(description)),
      )
      .child(div().min_w(px(220.)).max_w(px(320.)).child(control))
  }

  // ==========================================================================
  // Sidebar
  // ==========================================================================

  fn render_sidebar(&self, cx: &Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let active = self.active;
    v_flex()
      .w(px(220.))
      .h_full()
      .flex_shrink_0()
      .py(px(12.))
      .px(px(8.))
      .gap(px(2.))
      .border_r_1()
      .border_color(colors.border)
      .bg(colors.background)
      .children(Category::ALL.iter().map(|cat| {
        let cat = *cat;
        let is_active = cat == active;
        let bg = if is_active {
          colors.accent
        } else {
          gpui::transparent_black()
        };
        let fg = if is_active {
          colors.accent_foreground
        } else {
          colors.foreground
        };
        h_flex()
          .id(SharedString::from(format!("settings-cat-{}", cat.label())))
          .w_full()
          .gap(px(10.))
          .px(px(10.))
          .py(px(8.))
          .items_center()
          .rounded(px(6.))
          .cursor_pointer()
          .bg(bg)
          .hover(|el| el.bg(if is_active { colors.accent } else { colors.list_hover }))
          .on_click(cx.listener(move |this, _ev, _window, cx| {
            this.active = cat;
            cx.notify();
          }))
          .child(Icon::new(cat.icon()).size(px(14.)).text_color(fg))
          .child(div().text_sm().text_color(fg).child(cat.label()))
      }))
      .child(div().flex_1())
      .child(div().h_px().w_full().bg(colors.border.opacity(0.5)))
      .child(
        div().pt(px(8.)).child(
          Button::new("reset-defaults")
            .icon(Icon::new(AppIcon::Restart))
            .label("Reset defaults")
            .small()
            .ghost()
            .on_click(cx.listener(|this, _ev, window, cx| {
              this.reset_to_defaults(window, cx);
            })),
        ),
      )
  }

  // ==========================================================================
  // Category bodies
  // ==========================================================================

  fn render_general(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let container_input = self.container_refresh_input.clone().unwrap();
    let stats_input = self.stats_refresh_input.clone().unwrap();
    let log_input = self.log_lines_input.clone().unwrap();
    let confirm = self.settings_state.read(cx).settings.confirm_destructive;
    let notify = self.settings_state.read(cx).settings.show_notifications;
    v_flex()
      .w_full()
      .child(Self::render_row(
        "Container refresh",
        "How often to refresh the container list (seconds)",
        Input::new(&container_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Stats refresh",
        "How often to refresh resource stats (seconds)",
        Input::new(&stats_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Max log lines",
        "Maximum number of log lines to display",
        Input::new(&log_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Confirm destructive actions",
        "Prompt before delete / prune / kill operations",
        Switch::new("confirm-destructive")
          .checked(confirm)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.settings_state.update(cx, |state, cx| {
              state.settings.confirm_destructive = *checked;
              let _ = state.settings.save();
              cx.emit(SettingsChanged::SettingsUpdated);
            });
            cx.notify();
          })),
        cx,
      ))
      .child(Self::render_row(
        "Show notifications",
        "Surface OS notifications when background tasks finish",
        Switch::new("show-notifications")
          .checked(notify)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.settings_state.update(cx, |state, cx| {
              state.settings.show_notifications = *checked;
              let _ = state.settings.save();
              cx.emit(SettingsChanged::SettingsUpdated);
            });
            cx.notify();
          })),
        cx,
      ))
      .into_any_element()
  }

  fn render_appearance(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let theme_select = self.theme_select.clone().unwrap();
    v_flex()
      .w_full()
      .child(Self::render_row(
        "Theme",
        "Color theme applied across the app",
        Select::new(&theme_select).w_full().small(),
        cx,
      ))
      .into_any_element()
  }

  fn render_terminal(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let font_input = self.font_size_input.clone().unwrap();
    let line_input = self.line_height_input.clone().unwrap();
    let scroll_input = self.scrollback_lines_input.clone().unwrap();
    let cursor_select = self.cursor_style_select.clone().unwrap();
    let font_family_input = self.font_family_input.clone().unwrap();
    let blink = self.settings_state.read(cx).settings.terminal_cursor_blink;

    v_flex()
      .w_full()
      .child(Self::render_row(
        "Font family",
        "Override terminal font family (empty = system mono)",
        Input::new(&font_family_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Font size",
        "Pixel size for terminal glyphs",
        Input::new(&font_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Line height",
        "Multiplier (e.g. 1.4) — applied to font size",
        Input::new(&line_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Scrollback",
        "Lines of history kept by the terminal",
        Input::new(&scroll_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Cursor style",
        "Block, underline, or thin bar",
        Select::new(&cursor_select).w_full().small(),
        cx,
      ))
      .child(Self::render_row(
        "Cursor blink",
        "Blink the terminal cursor",
        Switch::new("cursor-blink")
          .checked(blink)
          .on_click(cx.listener(move |this, checked: &bool, _window, cx| {
            this.settings_state.update(cx, |state, cx| {
              state.settings.terminal_cursor_blink = *checked;
              let _ = state.settings.save();
              cx.emit(SettingsChanged::SettingsUpdated);
            });
            cx.notify();
          })),
        cx,
      ))
      .into_any_element()
  }

  fn render_docker(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let socket_input = self.docker_socket_input.clone().unwrap();
    let platform_input = self.default_platform_input.clone().unwrap();
    v_flex()
      .w_full()
      .child(Self::render_row(
        "Docker socket",
        "Path to docker.sock (leave empty for the platform default)",
        Input::new(&socket_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Default pull platform",
        "Default --platform argument for image pulls (empty = host arch)",
        Input::new(&platform_input).small().w_full(),
        cx,
      ))
      .into_any_element()
  }

  fn render_kubernetes(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let enabled = self.settings_state.read(cx).settings.kubernetes_enabled;
    let kubeconfig_input = self.kubeconfig_input.clone().unwrap();
    let namespace_input = self.default_namespace_input.clone().unwrap();
    v_flex()
      .w_full()
      .child(Self::render_row(
        "Enable Kubernetes",
        "Show Pods, Deployments and Services in the sidebar (requires kubectl + kubeconfig)",
        Switch::new("kubernetes-enabled").checked(enabled).on_click(cx.listener(
          |this, checked: &bool, _window, cx| {
            this.settings_state.update(cx, |state, cx| {
              state.settings.kubernetes_enabled = *checked;
              let _ = state.settings.save();
              cx.emit(SettingsChanged::SettingsUpdated);
            });
            cx.notify();
          },
        )),
        cx,
      ))
      .child(Self::render_row(
        "kubeconfig path",
        "Override path to the kubeconfig file (empty = standard discovery)",
        Input::new(&kubeconfig_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Default namespace",
        "Namespace selected on first load",
        Input::new(&namespace_input).small().w_full(),
        cx,
      ))
      .into_any_element()
  }

  fn render_colima(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let enabled = self.settings_state.read(cx).settings.colima_enabled;
    let cache_size = self.cache_size.clone();
    let is_pruning = self.is_pruning;
    let profile_input = self.colima_profile_input.clone().unwrap();

    let mut col = v_flex().w_full().child(Self::render_row(
      "Enable Colima",
      "Show the Machines view and Colima actions",
      Switch::new("colima-enabled")
        .checked(enabled)
        .on_click(cx.listener(|this, checked: &bool, window, cx| {
          if *checked && !crate::utils::is_colima_installed() {
            this.show_colima_install_dialog(window, cx);
            return;
          }
          this.settings_state.update(cx, |state, cx| {
            state.settings.colima_enabled = *checked;
            let _ = state.settings.save();
            cx.emit(SettingsChanged::SettingsUpdated);
          });
          cx.notify();
        })),
      cx,
    ));

    if !enabled {
      return col.into_any_element();
    }

    let cpus_input = self.colima_cpus_input.clone().unwrap();
    let memory_input = self.colima_memory_input.clone().unwrap();
    let disk_input = self.colima_disk_input.clone().unwrap();

    col = col
      .child(Self::render_section("VM defaults", cx))
      .child(Self::render_row(
        "Default profile",
        "Profile name used when no other is specified",
        Input::new(&profile_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Default CPUs",
        "CPU count for new Colima profiles",
        Input::new(&cpus_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Default memory (GiB)",
        "RAM allocated to new Colima profiles",
        Input::new(&memory_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Default disk (GiB)",
        "Disk allocated to new Colima profiles",
        Input::new(&disk_input).small().w_full(),
        cx,
      ))
      .child(Self::render_row(
        "Default template",
        "YAML template seeded into new profiles",
        Button::new("edit-template")
          .icon(Icon::new(AppIcon::Edit))
          .label("Edit")
          .small()
          .ghost()
          .on_click(cx.listener(|this, _ev, window, cx| {
            this.open_template_editor(window, cx);
          })),
        cx,
      ))
      .child(Self::render_section("Cache", cx))
      .child(Self::render_row(
        "Disk usage",
        "Total cache size on disk",
        h_flex()
          .gap(px(8.))
          .items_center()
          .justify_end()
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().colors.foreground)
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
        cx,
      ));

    col.into_any_element()
  }

  fn render_editor(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let editor_select = self.editor_select.clone().unwrap();
    let wait_close = self.settings_state.read(cx).settings.editor_wait_close;
    v_flex()
      .w_full()
      .child(Self::render_row(
        "External editor",
        "Editor used when opening container or volume files remotely",
        Select::new(&editor_select).w_full().small(),
        cx,
      ))
      .child(Self::render_row(
        "Wait for editor to close",
        "Block the calling task until the editor process exits",
        Switch::new("editor-wait")
          .checked(wait_close)
          .on_click(cx.listener(|this, checked: &bool, _window, cx| {
            this.settings_state.update(cx, |state, cx| {
              state.settings.editor_wait_close = *checked;
              let _ = state.settings.save();
              cx.emit(SettingsChanged::SettingsUpdated);
            });
            cx.notify();
          })),
        cx,
      ))
      .into_any_element()
  }

  // ==========================================================================
  // Dialogs
  // ==========================================================================

  #[allow(clippy::unused_self)]
  fn open_template_editor(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
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
                .child("Used as the default config when creating new Colima VMs."),
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

  #[allow(clippy::unused_self)]
  fn show_colima_install_dialog(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
    use crate::platform::Platform;
    let platform = Platform::detect();
    let (install_cmd, install_description) = match platform {
      Platform::MacOS => (
        "brew install colima docker",
        "Install Colima and Docker CLI using Homebrew. If you don't have Homebrew, visit https://brew.sh first.",
      ),
      Platform::Linux | Platform::WindowsWsl2 => (
        "curl -fsSL https://github.com/abiosoft/colima/releases/latest/download/colima-Linux-x86_64 -o /usr/local/bin/colima && chmod +x /usr/local/bin/colima",
        "Download Colima from GitHub releases. You may also install via apt, dnf, or pacman depending on your distribution.",
      ),
      Platform::Windows => (
        "wsl --install",
        "Colima is not supported on native Windows. Install WSL2, then run Dockside from inside your WSL2 distribution.",
      ),
    };
    let install_cmd_for_copy = install_cmd.to_string();

    window.open_dialog(cx, move |dialog, _window, cx| {
      let colors = cx.theme().colors;
      let cmd_for_button = install_cmd_for_copy.clone();
      let has_command = !install_cmd_for_copy.is_empty();
      dialog
        .title("Colima Not Installed")
        .width(px(550.))
        .child(
          v_flex()
            .gap(px(16.))
            .child(
              div()
                .text_sm()
                .text_color(colors.foreground)
                .child("Colima is not installed. It is required to manage Docker VMs from this app."),
            )
            .child(
              div()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(install_description),
            )
            .when(has_command, |el| {
              el.child(
                div()
                  .w_full()
                  .p(px(12.))
                  .rounded(px(6.))
                  .bg(colors.secondary)
                  .border_1()
                  .border_color(colors.border)
                  .child(
                    div()
                      .text_sm()
                      .font_family("monospace")
                      .text_color(colors.foreground)
                      .child(install_cmd_for_copy.clone()),
                  ),
              )
            }),
        )
        .footer(move |_, _, _, _| {
          let cmd = cmd_for_button.clone();
          let mut buttons = vec![
            Button::new("close-dialog")
              .label("Close")
              .ghost()
              .on_click(|_, window, cx| {
                window.close_dialog(cx);
              }),
          ];
          if !cmd.is_empty() {
            buttons.push(
              Button::new("copy-command")
                .label("Copy command")
                .primary()
                .on_click(move |_, window, cx| {
                  cx.write_to_clipboard(gpui::ClipboardItem::new_string(cmd.clone()));
                  window.close_dialog(cx);
                }),
            );
          }
          buttons
        })
    });
  }
}

impl Render for SettingsView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    self.ensure_initialized(window, cx);
    let colors = cx.theme().colors;

    let body = match self.active {
      Category::General => self.render_general(cx),
      Category::Appearance => self.render_appearance(cx),
      Category::Terminal => self.render_terminal(cx),
      Category::Docker => self.render_docker(cx),
      Category::Kubernetes => self.render_kubernetes(cx),
      Category::Colima => self.render_colima(cx),
      Category::Editor => self.render_editor(cx),
    };

    let pane = v_flex().flex_1().h_full().min_w(px(0.)).bg(colors.background).child(
      div()
        .id("settings-scroll")
        .flex_1()
        .overflow_y_scrollbar()
        .px(px(32.))
        .py(px(24.))
        .child(div().w_full().max_w(px(720.)).child(body)),
    );

    h_flex()
      .size_full()
      .bg(colors.background)
      .child(self.render_sidebar(cx))
      .child(pane)
  }
}
