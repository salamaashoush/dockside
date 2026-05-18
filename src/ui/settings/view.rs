use gpui::{Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Disableable, Icon, IconName, IndexPath, Sizable, WindowExt,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputEvent, InputState},
  scroll::ScrollableElement,
  select::{Select, SelectItem, SelectState},
  switch::Switch,
  theme::{ActiveTheme, Theme, ThemeRegistry},
  v_flex,
};

use crate::assets::AppIcon;
use crate::colima::ColimaClient;
use crate::state::{
  ExternalEditor, SettingsChanged, SettingsState, StateChanged, TerminalCursorStyle, ThemeName, docker_state,
  settings_state,
};
use crate::ui::components::{form_field, form_section};

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
  Dns,
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
    Self::Dns,
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
      Self::Dns => "Local DNS",
    }
  }

  fn icon(self) -> IconName {
    match self {
      Self::General => IconName::Settings,
      Self::Appearance => IconName::Palette,
      Self::Terminal => IconName::SquareTerminal,
      Self::Docker | Self::Dns => IconName::Globe,
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
  dns_suffix_input: Option<Entity<InputState>>,
  dns_port_input: Option<Entity<InputState>>,
  proxy_http_port_input: Option<Entity<InputState>>,
  proxy_https_port_input: Option<Entity<InputState>>,
  initialized: bool,
  last_theme_index: Option<usize>,
  cache_size: String,
  is_pruning: bool,
}

impl SettingsView {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let settings_state = settings_state(cx);
    let cache_size = ColimaClient::cache_size().unwrap_or_else(|_| "Unknown".to_string());

    // Re-render when containers change so the DNS Routes / Status panels
    // reflect the current `RouteMap` snapshot live.
    cx.subscribe(&docker_state(cx), |_this, _state, event: &StateChanged, cx| {
      if matches!(event, StateChanged::ContainersUpdated) {
        cx.notify();
      }
    })
    .detach();

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
      dns_suffix_input: None,
      dns_port_input: None,
      proxy_http_port_input: None,
      proxy_https_port_input: None,
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
    self.dns_suffix_input = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("dockside.test")
        .default_value(&settings.dns_suffix)
    }));
    self.dns_port_input = Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.dns_port.to_string())));
    self.proxy_http_port_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.proxy_http_port.to_string())));
    self.proxy_https_port_input =
      Some(cx.new(|cx| InputState::new(window, cx).default_value(settings.proxy_https_port.to_string())));

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
      self.dns_suffix_input.clone(),
      self.dns_port_input.clone(),
      self.proxy_http_port_input.clone(),
      self.proxy_https_port_input.clone(),
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
    let dns_suffix = self.dns_suffix_input.as_ref().map_or_else(
      || "dockside.test".to_string(),
      |i| i.read(cx).text().to_string().trim().to_string(),
    );
    let dns_suffix = if dns_suffix.is_empty() {
      "dockside.test".to_string()
    } else {
      dns_suffix
    };
    let dns_port = self
      .dns_port_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u16>().ok())
      .unwrap_or(15353);
    let plain_proxy_port = self
      .proxy_http_port_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u16>().ok())
      .unwrap_or(47080);
    let secure_proxy_port = self
      .proxy_https_port_input
      .as_ref()
      .and_then(|i| i.read(cx).text().to_string().parse::<u16>().ok())
      .unwrap_or(47443);

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
      state.settings.dns_suffix = dns_suffix;
      state.settings.dns_port = dns_port;
      state.settings.proxy_http_port = plain_proxy_port;
      state.settings.proxy_https_port = secure_proxy_port;
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

  /// One settings column. Every field inside uses the shared
  /// `form_field` / `form_section`; this only sets the shared spacing.
  fn body() -> gpui::Div {
    v_flex().w_full().gap(px(16.))
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
    Self::body()
      .child(form_section("Refresh", cx))
      .child(form_field(
        "Container refresh",
        Input::new(&container_input).small().w_full(),
        Some("How often to refresh the container list, in seconds."),
        cx,
      ))
      .child(form_field(
        "Stats refresh",
        Input::new(&stats_input).small().w_full(),
        Some("How often to refresh resource stats, in seconds."),
        cx,
      ))
      .child(form_field(
        "Max log lines",
        Input::new(&log_input).small().w_full(),
        Some("Maximum number of log lines to display."),
        cx,
      ))
      .child(form_section("Behavior", cx))
      .child(form_field(
        "Confirm destructive actions",
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
        Some("Prompt before delete / prune / kill operations."),
        cx,
      ))
      .child(form_field(
        "Show notifications",
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
        Some("Surface OS notifications when background tasks finish."),
        cx,
      ))
      .into_any_element()
  }

  fn render_appearance(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let theme_select = self.theme_select.clone().unwrap();
    Self::body()
      .child(form_section("Theme", cx))
      .child(form_field(
        "Theme",
        Select::new(&theme_select).w_full().small(),
        Some("Color theme applied across the whole app."),
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

    Self::body()
      .child(form_section("Font", cx))
      .child(form_field(
        "Font family",
        Input::new(&font_family_input).small().w_full(),
        Some("Override the terminal font family. Empty uses the system monospace font."),
        cx,
      ))
      .child(form_field(
        "Font size",
        Input::new(&font_input).small().w_full(),
        Some("Pixel size for terminal glyphs."),
        cx,
      ))
      .child(form_field(
        "Line height",
        Input::new(&line_input).small().w_full(),
        Some("Multiplier applied to the font size (e.g. 1.4)."),
        cx,
      ))
      .child(form_section("Behavior", cx))
      .child(form_field(
        "Scrollback",
        Input::new(&scroll_input).small().w_full(),
        Some("Lines of history kept by the terminal."),
        cx,
      ))
      .child(form_field(
        "Cursor style",
        Select::new(&cursor_select).w_full().small(),
        Some("Block, underline, or thin bar."),
        cx,
      ))
      .child(form_field(
        "Cursor blink",
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
        Some("Blink the terminal cursor."),
        cx,
      ))
      .into_any_element()
  }

  fn render_docker(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let socket_input = self.docker_socket_input.clone().unwrap();
    let platform_input = self.default_platform_input.clone().unwrap();
    Self::body()
      .child(form_section("Connection", cx))
      .child(form_field(
        "Docker socket",
        Input::new(&socket_input).small().w_full(),
        Some("Path to docker.sock. Leave empty for the platform default."),
        cx,
      ))
      .child(form_field(
        "Default pull platform",
        Input::new(&platform_input).small().w_full(),
        Some("Default --platform argument for image pulls. Empty uses the host architecture."),
        cx,
      ))
      .into_any_element()
  }

  fn render_kubernetes(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let enabled = self.settings_state.read(cx).settings.kubernetes_enabled;
    let kubeconfig_input = self.kubeconfig_input.clone().unwrap();
    let namespace_input = self.default_namespace_input.clone().unwrap();
    Self::body()
      .child(form_section("Cluster", cx))
      .child(form_field(
        "Enable Kubernetes",
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
        Some("Show Pods, Deployments and Services in the sidebar. Requires kubectl and a kubeconfig."),
        cx,
      ))
      .child(form_field(
        "kubeconfig path",
        Input::new(&kubeconfig_input).small().w_full(),
        Some("Override the path to the kubeconfig file. Empty uses standard discovery."),
        cx,
      ))
      .child(form_field(
        "Default namespace",
        Input::new(&namespace_input).small().w_full(),
        Some("Namespace selected on first load."),
        cx,
      ))
      .into_any_element()
  }

  fn render_dns(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    // The DNS screen is a feature control panel, not a flat form:
    // resolver + reverse proxy + host integration with a one-time
    // privileged setup gate. Lay it out as status hero → setup gate →
    // configuration → host integration → live routes so related
    // controls (and their state) stay together.
    let colors = cx.theme().colors;
    let settings = self.settings_state.read(cx).settings.clone();
    let dns_mgr = crate::services::dns::manager(cx);
    let dns_running = dns_mgr.is_running();
    let dns_port = dns_mgr.bound_port();
    let proxy_mgr = crate::services::proxy::manager(cx);
    let proxy_running = proxy_mgr.is_running();
    let proxy_ports = proxy_mgr.bound_ports();
    let route_count = dns_mgr.route_map().map_or(0, |m| m.read().distinct_container_count());
    let suffix = settings.dns_suffix.clone();
    let bootstrapped = crate::services::is_bootstrapped();
    let port = settings.dns_port;

    // A status is just a coloured dot now — green = on/installed,
    // muted = off/missing. No redundant "Running" / "Installed" tag.
    let dot_el = move |ok: bool| {
      div()
        .size(px(7.))
        .rounded_full()
        .flex_shrink_0()
        .bg(if ok { colors.success } else { colors.muted_foreground })
    };

    // Label + hint on the left, control on the right. The conventional
    // toggle/action layout — keeps a switch next to what it controls
    // instead of stranding it full-width above its own caption.
    let feature_row = move |label: &'static str, hint: &'static str, control: gpui::AnyElement| {
      h_flex()
        .w_full()
        .py(px(10.))
        .gap(px(16.))
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(colors.border.opacity(0.4))
        .child(
          v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(div().text_sm().text_color(colors.foreground).child(label))
            .child(
              div()
                .text_xs()
                .text_color(colors.muted_foreground)
                .whitespace_normal()
                .child(hint),
            ),
        )
        .child(div().flex_shrink_0().child(control))
    };

    // ---- Status hero: whole-feature state + ONE master switch ------------
    // The switch is the only thing the user normally touches. Turning it
    // on runs the entire privileged setup (helper + polkit + system
    // resolver + root CA + low-port redirect) in one prompt, then starts
    // resolving. Off just stops resolving; setup artifacts stay so the
    // next on is silent. Everything below is read-only status or
    // power-user knobs.
    let low_ports_granted = crate::services::has_low_ports_grant();
    let ca_installed = crate::services::is_local_ca_installed();
    let restart_required = proxy_mgr.restart_required();
    let active = dns_running && proxy_running;
    let partial = !active && (dns_running || proxy_running);
    let state_color = if active {
      colors.success
    } else if partial {
      colors.warning
    } else {
      colors.muted_foreground
    };

    // A status line: leading dot, then label, value on the right. The
    // dot column is fixed so every label starts at the same x.
    let stat = move |ok: bool, label: &'static str, value: gpui::AnyElement| {
      h_flex()
        .w_full()
        .py(px(7.))
        .gap(px(12.))
        .items_center()
        .child(
          h_flex()
            .flex_1()
            .min_w_0()
            .gap(px(10.))
            .items_center()
            .child(dot_el(ok))
            .child(div().text_sm().text_color(colors.muted_foreground).child(label)),
        )
        .child(div().flex_shrink_0().child(value))
    };
    // An info line: no dot (it has no on/off state) but a same-width
    // spacer so its label aligns with the status labels above.
    let info = move |label: &'static str, value: gpui::AnyElement| {
      h_flex()
        .w_full()
        .py(px(7.))
        .gap(px(12.))
        .items_center()
        .child(
          h_flex()
            .flex_1()
            .min_w_0()
            .gap(px(10.))
            .items_center()
            .child(div().w(px(7.)).flex_shrink_0())
            .child(div().text_sm().text_color(colors.muted_foreground).child(label)),
        )
        .child(div().flex_shrink_0().child(value))
    };
    let mono = move |s: SharedString| {
      div()
        .text_sm()
        .font_family("monospace")
        .text_color(colors.foreground)
        .child(s)
    };
    // Resolver / proxy carry their bound ports as the right-hand value;
    // their on/off state is the leading dot on the row.
    let resolver_val = if dns_running {
      mono(dns_port.map_or_else(String::new, |p| format!(":{p}")).into()).into_any_element()
    } else {
      div().into_any_element()
    };
    let proxy_val = if proxy_running {
      let ports = match proxy_ports {
        Some((h, Some(s))) => format!(":{h}/:{s}"),
        Some((h, None)) => format!(":{h}"),
        _ => String::new(),
      };
      mono(ports.into()).into_any_element()
    } else {
      div().into_any_element()
    };

    // ---- The one card: header + master switch + status detail -----------
    let suffix_for_toggle = suffix.clone();
    let card = v_flex()
      .w_full()
      .p(px(16.))
      .rounded(px(8.))
      .bg(colors.sidebar)
      .border_1()
      .border_color(colors.border.opacity(0.6))
      .child(
        h_flex()
          .w_full()
          .pb(px(12.))
          .gap(px(12.))
          .items_center()
          .justify_between()
          .child(
            h_flex()
              .flex_1()
              .min_w_0()
              .gap(px(10.))
              .items_center()
              .child(div().size(px(10.)).rounded_full().flex_shrink_0().bg(state_color))
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::SEMIBOLD)
                  .text_color(colors.foreground)
                  .child("Local DNS"),
              ),
          )
          .child(
            Switch::new("dns-enabled")
              .checked(settings.dns_enabled)
              .on_click(cx.listener(move |this, checked: &bool, _window, cx| {
                let new_state = *checked;
                // Turning on: make sure the whole stack is installed.
                // Every step is idempotent and skipped when already
                // applied, so a warm setup stays prompt-free.
                if new_state {
                  if !crate::services::is_bootstrapped()
                    && let Err(e) = crate::services::bootstrap(&suffix_for_toggle, port)
                  {
                    tracing::warn!("dns: bootstrap failed: {e}");
                  }
                  if !crate::services::is_local_ca_installed()
                    && let Err(e) = crate::services::install_local_ca()
                  {
                    tracing::warn!("dns: install_local_ca failed: {e}");
                  }
                  if !crate::services::has_low_ports_grant()
                    && let Err(e) = crate::services::grant_low_ports()
                  {
                    tracing::warn!("dns: grant_low_ports failed: {e}");
                  }
                }
                this.settings_state.update(cx, |state, cx| {
                  state.settings.dns_enabled = new_state;
                  let _ = state.settings.save();
                  cx.emit(SettingsChanged::SettingsUpdated);
                });
                crate::services::apply_dns_settings(cx);
                if new_state {
                  if let Err(e) = crate::services::install_dns_resolver(&suffix_for_toggle, port) {
                    tracing::warn!("dns: install_dns_resolver failed: {e}");
                  }
                } else if let Err(e) = crate::services::uninstall_dns_resolver(&suffix_for_toggle) {
                  tracing::warn!("dns: uninstall_dns_resolver failed: {e}");
                }
                cx.notify();
              })),
          ),
      )
      .child(div().w_full().h(px(1.)).bg(colors.border.opacity(0.5)).mb(px(4.)))
      .child(stat(dns_running, "Resolver", resolver_val))
      .child(stat(proxy_running, "Reverse proxy", proxy_val))
      .child(stat(bootstrapped, "Privileged helper", div().into_any_element()))
      .child(stat(ca_installed, "HTTPS certificate", div().into_any_element()))
      .child(stat(
        low_ports_granted,
        "Pretty URLs (no port)",
        div().into_any_element(),
      ))
      .child(info("Suffix", mono(format!("*.{suffix}").into()).into_any_element()))
      .child(info(
        "Routed containers",
        mono(route_count.to_string().into()).into_any_element(),
      ));

    // ---- Configuration ---------------------------------------------------
    let suffix_input = self.dns_suffix_input.clone().unwrap();
    let dns_port_box = self.dns_port_input.clone().unwrap();
    let plain_port_box = self.proxy_http_port_input.clone().unwrap();
    let secure_port_box = self.proxy_https_port_input.clone().unwrap();
    let grid = || h_flex().w_full().gap(px(16.)).items_start();

    let autostart_row = feature_row(
      "Auto-start on launch",
      "Start the resolver and proxy when Dockside launches (only while Local DNS is enabled).",
      Switch::new("dns-autostart")
        .checked(settings.dns_autostart)
        .on_click(cx.listener(|this, checked: &bool, _window, cx| {
          this.settings_state.update(cx, |state, cx| {
            state.settings.dns_autostart = *checked;
            let _ = state.settings.save();
            cx.emit(SettingsChanged::SettingsUpdated);
          });
          cx.notify();
        }))
        .into_any_element(),
    );

    // Power-user escape hatch: one row, not a button per component.
    let maintenance_row = feature_row(
      "Setup & repair",
      "Re-run the privileged setup if something broke, or remove Local DNS entirely.",
      h_flex()
        .gap(px(8.))
        .items_center()
        .child(
          Button::new("dns-bootstrap")
            .label("Re-run setup")
            .ghost()
            .small()
            .on_click(cx.listener(|this, _ev, _w, cx| {
              let settings = this.settings_state.read(cx).settings.clone();
              if let Err(e) = crate::services::bootstrap(&settings.dns_suffix, settings.dns_port) {
                tracing::warn!("dns: bootstrap failed: {e}");
              }
              cx.notify();
            })),
        )
        .child(
          Button::new("dns-uninstall-bootstrap")
            .label("Uninstall")
            .ghost()
            .small()
            .on_click(cx.listener(|_this, _ev, _w, cx| {
              if let Err(e) = crate::services::uninstall_bootstrap() {
                tracing::warn!("dns: uninstall_bootstrap failed: {e}");
              }
              if let Err(e) = crate::services::uninstall_local_ca() {
                tracing::warn!("dns: uninstall_local_ca failed: {e}");
              }
              if crate::services::has_low_ports_grant()
                && let Err(e) = crate::services::revoke_low_ports()
              {
                tracing::warn!("dns: revoke_low_ports failed: {e}");
              }
              cx.notify();
            })),
        )
        .into_any_element(),
    );

    let restart_banner = v_flex()
      .w_full()
      .gap(px(4.))
      .p(px(12.))
      .rounded(px(8.))
      .bg(colors.danger.opacity(0.1))
      .border_1()
      .border_color(colors.danger.opacity(0.4))
      .child(
        div()
          .text_sm()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .text_color(colors.danger)
          .child("Restart Dockside to bind 80/443"),
      )
      .child(div().text_xs().text_color(colors.muted_foreground).child(
        "The capability grant only applies to processes started after it. Until you quit and \
         reopen the app the proxy stays on its fallback ports (47080 / 47443); container links \
         use those automatically.",
      ));

    // ---- Live routes: read-only diagnostics, kept compact ----------------
    let routes_snapshot: Vec<(String, String)> = dns_mgr
      .route_map()
      .map_or_else(Vec::new, |map| map.read().route_summaries());
    let routes_panel = {
      let mut list = v_flex().w_full().gap(px(2.));
      if routes_snapshot.is_empty() {
        list = list.child(
          div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(if dns_running {
              "No running containers yet."
            } else {
              "Enable Local DNS to start routing containers."
            }),
        );
      } else {
        for (name, target) in routes_snapshot {
          list = list.child(
            h_flex()
              .w_full()
              .py(px(4.))
              .gap(px(12.))
              .items_center()
              .child(
                div()
                  .flex_1()
                  .min_w_0()
                  .text_sm()
                  .text_color(colors.foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .whitespace_nowrap()
                  .child(name),
              )
              .child(
                div()
                  .flex_shrink_0()
                  .text_sm()
                  .font_family("monospace")
                  .text_color(colors.muted_foreground)
                  .child(target),
              ),
          );
        }
      }
      // Own scroll area. Use a fixed height (not max_h) so the box
      // always reserves its space and clips — max_h collapsed to zero
      // inside the page scroll and let the list overlap the sections
      // below.
      v_flex()
        .w_full()
        .p(px(16.))
        .rounded(px(8.))
        .bg(colors.sidebar)
        .border_1()
        .border_color(colors.border.opacity(0.6))
        .child(
          div()
            .id("dns-routes-scroll")
            .w_full()
            .h(px(240.))
            .overflow_y_scrollbar()
            .child(list),
        )
    };

    Self::body()
      .child(card)
      .when(restart_required, |el| el.child(restart_banner))
      .child(form_section("Configuration", cx))
      .child(autostart_row)
      .child(
        grid()
          .child(div().flex_1().min_w_0().child(form_field(
            "Suffix",
            Input::new(&suffix_input).small().w_full(),
            Some("Wildcard host suffix. .test is RFC 6761; avoid .local (mDNS)."),
            cx,
          )))
          .child(div().flex_1().min_w_0().child(form_field(
            "DNS port",
            Input::new(&dns_port_box).small().w_full(),
            Some("UDP+TCP resolver port. 15353 is the convention; 53 needs root."),
            cx,
          ))),
      )
      .child(
        grid()
          .child(div().flex_1().min_w_0().child(form_field(
            "HTTP proxy port",
            Input::new(&plain_port_box).small().w_full(),
            Some("Loopback HTTP port. 80 needs root; 47080 avoids dev-tool collisions."),
            cx,
          )))
          .child(div().flex_1().min_w_0().child(form_field(
            "HTTPS proxy port",
            Input::new(&secure_port_box).small().w_full(),
            Some("Loopback HTTPS port. 443 needs root; 47443 avoids dev-tool collisions."),
            cx,
          ))),
      )
      .child(form_section("Live routes", cx))
      .child(routes_panel)
      .child(form_section("Maintenance", cx))
      .child(maintenance_row)
      .into_any_element()
  }

  fn render_colima(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let enabled = self.settings_state.read(cx).settings.colima_enabled;
    let cache_size = self.cache_size.clone();
    let is_pruning = self.is_pruning;
    let profile_input = self.colima_profile_input.clone().unwrap();

    let mut col = Self::body().child(form_section("Colima", cx)).child(form_field(
      "Enable Colima",
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
      Some("Show the Machines view and Colima actions."),
      cx,
    ));

    if !enabled {
      return col.into_any_element();
    }

    let cpus_input = self.colima_cpus_input.clone().unwrap();
    let memory_input = self.colima_memory_input.clone().unwrap();
    let disk_input = self.colima_disk_input.clone().unwrap();

    col = col
      .child(form_section("VM defaults", cx))
      .child(form_field(
        "Default profile",
        Input::new(&profile_input).small().w_full(),
        Some("Profile name used when no other is specified."),
        cx,
      ))
      .child(form_field(
        "Default CPUs",
        Input::new(&cpus_input).small().w_full(),
        Some("CPU count for new Colima profiles."),
        cx,
      ))
      .child(form_field(
        "Default memory (GiB)",
        Input::new(&memory_input).small().w_full(),
        Some("RAM allocated to new Colima profiles."),
        cx,
      ))
      .child(form_field(
        "Default disk (GiB)",
        Input::new(&disk_input).small().w_full(),
        Some("Disk allocated to new Colima profiles."),
        cx,
      ))
      .child(form_field(
        "Default template",
        Button::new("edit-template")
          .icon(Icon::new(AppIcon::Edit))
          .label("Edit")
          .small()
          .ghost()
          .on_click(cx.listener(|this, _ev, window, cx| {
            this.open_template_editor(window, cx);
          })),
        Some("YAML template seeded into new profiles."),
        cx,
      ))
      .child(form_section("Cache", cx))
      .child(form_field(
        "Disk usage",
        h_flex()
          .gap(px(8.))
          .items_center()
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
        Some("Total Colima cache size on disk."),
        cx,
      ));

    col.into_any_element()
  }

  fn render_editor(&self, cx: &Context<'_, Self>) -> gpui::AnyElement {
    let editor_select = self.editor_select.clone().unwrap();
    let wait_close = self.settings_state.read(cx).settings.editor_wait_close;
    Self::body()
      .child(form_section("External editor", cx))
      .child(form_field(
        "External editor",
        Select::new(&editor_select).w_full().small(),
        Some("Editor used when opening container or volume files remotely."),
        cx,
      ))
      .child(form_field(
        "Wait for editor to close",
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
        Some("Block the calling task until the editor process exits."),
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
      Category::Dns => self.render_dns(cx),
    };

    let pane = v_flex().flex_1().h_full().min_w(px(0.)).bg(colors.background).child(
      div()
        .id("settings-scroll")
        .flex_1()
        .overflow_y_scrollbar()
        .px(px(32.))
        .py(px(24.))
        .child(div().w_full().child(body)),
    );

    h_flex()
      .size_full()
      .bg(colors.background)
      .child(self.render_sidebar(cx))
      .child(pane)
  }
}
