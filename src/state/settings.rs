use anyhow::Result;
use gpui::{App, AppContext, Entity, EventEmitter, Global};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::platform::get_config_dir;

/// Available themes (matching themes in themes/ directory JSON files)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeName {
  // Tokyo Night variants
  #[default]
  TokyoNight,
  TokyoStorm,
  TokyoMoon,
  // Catppuccin variants
  CatppuccinLatte,
  CatppuccinFrappe,
  CatppuccinMacchiato,
  CatppuccinMocha,
  // Gruvbox variants
  GruvboxLight,
  GruvboxDark,
  // Ayu variants
  AyuLight,
  AyuDark,
  // Solarized variants
  SolarizedLight,
  SolarizedDark,
  // Everforest variants
  EverforestLight,
  EverforestDark,
  // Flexoki variants
  FlexokiLight,
  FlexokiDark,
  // Hybrid variants
  HybridLight,
  HybridDark,
  // Molokai variants
  MolokaiLight,
  MolokaiDark,
  // Mellifluous variants
  MellifluousLight,
  MellifluousDark,
  // macOS Classic variants
  MacOSClassicLight,
  MacOSClassicDark,
  // Adventure variants
  Adventure,
  AdventureTime,
  // Single-mode themes
  Alduin,
  Fahrenheit,
  Harper,
  Jellybeans,
  Kibble,
  Matrix,
  Spaceduck,
  Twilight,
}

impl ThemeName {
  pub fn all() -> Vec<ThemeName> {
    vec![
      // Dark themes first (most common preference)
      ThemeName::TokyoNight,
      ThemeName::TokyoStorm,
      ThemeName::TokyoMoon,
      ThemeName::CatppuccinMocha,
      ThemeName::CatppuccinMacchiato,
      ThemeName::CatppuccinFrappe,
      ThemeName::GruvboxDark,
      ThemeName::AyuDark,
      ThemeName::SolarizedDark,
      ThemeName::EverforestDark,
      ThemeName::FlexokiDark,
      ThemeName::HybridDark,
      ThemeName::MolokaiDark,
      ThemeName::MellifluousDark,
      ThemeName::MacOSClassicDark,
      ThemeName::Adventure,
      ThemeName::AdventureTime,
      ThemeName::Alduin,
      ThemeName::Fahrenheit,
      ThemeName::Harper,
      ThemeName::Jellybeans,
      ThemeName::Kibble,
      ThemeName::Matrix,
      ThemeName::Spaceduck,
      ThemeName::Twilight,
      // Light themes
      ThemeName::CatppuccinLatte,
      ThemeName::GruvboxLight,
      ThemeName::AyuLight,
      ThemeName::SolarizedLight,
      ThemeName::EverforestLight,
      ThemeName::FlexokiLight,
      ThemeName::HybridLight,
      ThemeName::MolokaiLight,
      ThemeName::MellifluousLight,
      ThemeName::MacOSClassicLight,
    ]
  }

  pub fn display_name(&self) -> &str {
    match self {
      ThemeName::TokyoNight => "Tokyo Night",
      ThemeName::TokyoStorm => "Tokyo Storm",
      ThemeName::TokyoMoon => "Tokyo Moon",
      ThemeName::CatppuccinLatte => "Catppuccin Latte",
      ThemeName::CatppuccinFrappe => "Catppuccin Frappe",
      ThemeName::CatppuccinMacchiato => "Catppuccin Macchiato",
      ThemeName::CatppuccinMocha => "Catppuccin Mocha",
      ThemeName::GruvboxLight => "Gruvbox Light",
      ThemeName::GruvboxDark => "Gruvbox Dark",
      ThemeName::AyuLight => "Ayu Light",
      ThemeName::AyuDark => "Ayu Dark",
      ThemeName::SolarizedLight => "Solarized Light",
      ThemeName::SolarizedDark => "Solarized Dark",
      ThemeName::EverforestLight => "Everforest Light",
      ThemeName::EverforestDark => "Everforest Dark",
      ThemeName::FlexokiLight => "Flexoki Light",
      ThemeName::FlexokiDark => "Flexoki Dark",
      ThemeName::HybridLight => "Hybrid Light",
      ThemeName::HybridDark => "Hybrid Dark",
      ThemeName::MolokaiLight => "Molokai Light",
      ThemeName::MolokaiDark => "Molokai Dark",
      ThemeName::MellifluousLight => "Mellifluous Light",
      ThemeName::MellifluousDark => "Mellifluous Dark",
      ThemeName::MacOSClassicLight => "macOS Classic Light",
      ThemeName::MacOSClassicDark => "macOS Classic Dark",
      ThemeName::Adventure => "Adventure",
      ThemeName::AdventureTime => "Adventure Time",
      ThemeName::Alduin => "Alduin",
      ThemeName::Fahrenheit => "Fahrenheit",
      ThemeName::Harper => "Harper",
      ThemeName::Jellybeans => "Jellybeans",
      ThemeName::Kibble => "Kibble",
      ThemeName::Matrix => "Matrix",
      ThemeName::Spaceduck => "Spaceduck",
      ThemeName::Twilight => "Twilight",
    }
  }

  /// Returns the theme name as used in the JSON file (for `ThemeRegistry` lookup)
  pub fn theme_name(&self) -> &str {
    self.display_name()
  }
}

/// External editor for opening container files
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExternalEditor {
  #[default]
  VSCode,
  Cursor,
  Zed,
}

/// Terminal cursor style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TerminalCursorStyle {
  Block,
  Underline,
  /// Thin I-beam (default).
  #[default]
  Bar,
}

#[allow(dead_code)]
impl TerminalCursorStyle {
  pub fn all() -> Vec<Self> {
    vec![Self::Block, Self::Underline, Self::Bar]
  }

  pub fn display_name(&self) -> &str {
    match self {
      Self::Block => "Block",
      Self::Underline => "Underline",
      Self::Bar => "Bar",
    }
  }
}

impl ExternalEditor {
  pub fn all() -> Vec<ExternalEditor> {
    vec![ExternalEditor::VSCode, ExternalEditor::Cursor, ExternalEditor::Zed]
  }

  pub fn display_name(&self) -> &str {
    match self {
      Self::VSCode => "VS Code",
      Self::Cursor => "Cursor",
      Self::Zed => "Zed",
    }
  }

  /// Returns the CLI command for this editor
  pub fn command(&self) -> &str {
    match self {
      Self::VSCode => "code",
      Self::Cursor => "cursor",
      Self::Zed => "zed",
    }
  }

  /// Whether this editor supports attaching to Docker containers via Dev Containers extension
  /// VS Code and Cursor use the vscode-remote:// URI scheme
  pub fn supports_container_attach(&self) -> bool {
    matches!(self, Self::VSCode | Self::Cursor)
  }

  /// Whether this editor supports SSH remote connections
  #[allow(clippy::unused_self)]
  pub const fn supports_ssh(&self) -> bool {
    true
  }
}

/// A pinned favorite shown on the Dashboard. Click navigates to the
/// corresponding detail view via the existing navigation services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FavoriteRef {
  Container { id: String, name: String },
  Image { id: String, repo_tag: String },
  Volume { name: String },
  Network { name: String, id: String },
  Pod { name: String, namespace: String },
  Deployment { name: String, namespace: String },
  Service { name: String, namespace: String },
  StatefulSet { name: String, namespace: String },
  DaemonSet { name: String, namespace: String },
  Job { name: String, namespace: String },
  CronJob { name: String, namespace: String },
  Ingress { name: String, namespace: String },
  Pvc { name: String, namespace: String },
  Secret { name: String, namespace: String },
  ConfigMap { name: String, namespace: String },
  Machine { id: String, label: String },
}

impl FavoriteRef {
  pub fn label(&self) -> &str {
    match self {
      Self::Image { repo_tag, .. } => repo_tag,
      Self::Container { name, .. }
      | Self::Volume { name }
      | Self::Network { name, .. }
      | Self::Pod { name, .. }
      | Self::Deployment { name, .. }
      | Self::Service { name, .. }
      | Self::StatefulSet { name, .. }
      | Self::DaemonSet { name, .. }
      | Self::Job { name, .. }
      | Self::CronJob { name, .. }
      | Self::Ingress { name, .. }
      | Self::Pvc { name, .. }
      | Self::Secret { name, .. }
      | Self::ConfigMap { name, .. } => name,
      Self::Machine { label, .. } => label,
    }
  }

  pub fn kind_label(&self) -> &'static str {
    match self {
      Self::Container { .. } => "Container",
      Self::Image { .. } => "Image",
      Self::Volume { .. } => "Volume",
      Self::Network { .. } => "Network",
      Self::Pod { .. } => "Pod",
      Self::Deployment { .. } => "Deployment",
      Self::Service { .. } => "Service",
      Self::StatefulSet { .. } => "StatefulSet",
      Self::DaemonSet { .. } => "DaemonSet",
      Self::Job { .. } => "Job",
      Self::CronJob { .. } => "CronJob",
      Self::Ingress { .. } => "Ingress",
      Self::Pvc { .. } => "PVC",
      Self::Secret { .. } => "Secret",
      Self::ConfigMap { .. } => "ConfigMap",
      Self::Machine { .. } => "Machine",
    }
  }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
  /// Selected theme
  pub theme: ThemeName,
  /// Docker socket path (empty for default)
  pub docker_socket: String,
  /// Default Colima profile name
  pub default_colima_profile: String,
  /// Enable Colima VM support (default: true on macOS, false on Linux/Windows).
  /// When disabled, the Colima sidebar group + Machines view are hidden.
  #[serde(default = "default_colima_enabled")]
  pub colima_enabled: bool,
  /// Enable Kubernetes feature (Pods/Deployments/Services). Default: true.
  /// When disabled, the Kubernetes sidebar group is hidden.
  #[serde(default = "default_kubernetes_enabled")]
  pub kubernetes_enabled: bool,
  /// Refresh interval for containers (in seconds)
  pub container_refresh_interval: u64,
  /// Refresh interval for stats (in seconds)
  pub stats_refresh_interval: u64,
  /// Maximum log lines to display
  pub max_log_lines: usize,
  /// Terminal font size
  pub terminal_font_size: f32,
  /// Terminal line height multiplier
  pub terminal_line_height: f32,
  /// Terminal cursor style
  pub terminal_cursor_style: TerminalCursorStyle,
  /// Terminal cursor blink enabled
  pub terminal_cursor_blink: bool,
  /// Terminal scrollback lines
  pub terminal_scrollback_lines: usize,
  /// Optional terminal font family override (empty = system mono).
  #[serde(default)]
  pub terminal_font_family: String,
  /// External editor for opening files
  pub external_editor: ExternalEditor,
  /// Wait for external editor process to close before reporting done.
  #[serde(default)]
  pub editor_wait_close: bool,
  /// Prompt before destructive actions (delete container, prune, etc.).
  #[serde(default = "default_true")]
  pub confirm_destructive: bool,
  /// Show desktop notifications for completed background tasks.
  #[serde(default = "default_true")]
  pub show_notifications: bool,
  /// Default platform passed to `docker pull` (empty = host arch).
  #[serde(default)]
  pub default_pull_platform: String,
  /// Override path for `kubeconfig` (empty = standard discovery).
  #[serde(default)]
  pub kubeconfig_path: String,
  /// Default Kubernetes namespace selected on first load.
  #[serde(default = "default_namespace")]
  pub default_namespace: String,
  /// Default CPU count for new Colima profiles.
  #[serde(default = "default_colima_cpus")]
  pub colima_default_cpus: u32,
  /// Default memory (GiB) for new Colima profiles.
  #[serde(default = "default_colima_memory")]
  pub colima_default_memory_gb: u32,
  /// Default disk size (GiB) for new Colima profiles.
  #[serde(default = "default_colima_disk")]
  pub colima_default_disk_gb: u32,
  /// Favorites pinned to the Dashboard.
  #[serde(default)]
  pub favorites: Vec<FavoriteRef>,
  /// Whether the local DNS resolver + reverse proxy stack is enabled.
  #[serde(default)]
  pub dns_enabled: bool,
  /// Auto-start the resolver + proxy on app launch once `dns_enabled` is on.
  #[serde(default = "default_true")]
  pub dns_autostart: bool,
  /// Suffix served by the resolver. Defaults to `dockside.test` (RFC 6761).
  #[serde(default = "default_dns_suffix")]
  pub dns_suffix: String,
  /// UDP+TCP port for the local DNS server.
  #[serde(default = "default_dns_port")]
  pub dns_port: u16,
  /// Plain HTTP listen port for the reverse proxy.
  #[serde(default = "default_proxy_http_port")]
  pub proxy_http_port: u16,
  /// HTTPS listen port for the reverse proxy.
  #[serde(default = "default_proxy_https_port")]
  pub proxy_https_port: u16,
}

fn default_true() -> bool {
  true
}
fn default_namespace() -> String {
  "default".to_string()
}
fn default_colima_cpus() -> u32 {
  2
}
fn default_colima_memory() -> u32 {
  4
}
fn default_colima_disk() -> u32 {
  60
}

/// Default value for `colima_enabled` based on platform
fn default_colima_enabled() -> bool {
  // Enable Colima by default on macOS (where it's the primary way to run Docker)
  // Disable by default on Linux (where native Docker is preferred)
  cfg!(target_os = "macos")
}

/// Default value for `kubernetes_enabled`. Off by default — most users only need
/// the Docker side; we don't want to make missing kubeconfigs scream errors at
/// every launch. Users can enable it from Settings.
fn default_kubernetes_enabled() -> bool {
  false
}

fn default_dns_suffix() -> String {
  "dockside.test".to_string()
}
fn default_dns_port() -> u16 {
  15353
}
fn default_proxy_http_port() -> u16 {
  // Unprivileged + uncommon. 8080/8443 collide with dev servers, jenkins,
  // tomcat, etc. The 47000-range is rarely used. Users can switch to 80 in
  // Settings + grant CAP_NET_BIND_SERVICE if they want pretty URLs.
  47080
}
fn default_proxy_https_port() -> u16 {
  47443
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      theme: ThemeName::TokyoNight,
      docker_socket: String::new(),
      default_colima_profile: "default".to_string(),
      colima_enabled: default_colima_enabled(),
      kubernetes_enabled: default_kubernetes_enabled(),
      container_refresh_interval: 5,
      stats_refresh_interval: 2,
      max_log_lines: 1000,
      terminal_font_size: 14.0,
      terminal_line_height: 1.4,
      terminal_cursor_style: TerminalCursorStyle::default(),
      terminal_cursor_blink: true,
      terminal_scrollback_lines: 10000,
      terminal_font_family: String::new(),
      external_editor: ExternalEditor::default(),
      editor_wait_close: false,
      confirm_destructive: true,
      show_notifications: true,
      default_pull_platform: String::new(),
      kubeconfig_path: String::new(),
      default_namespace: "default".to_string(),
      colima_default_cpus: 2,
      colima_default_memory_gb: 4,
      colima_default_disk_gb: 60,
      favorites: Vec::new(),
      dns_enabled: false,
      dns_autostart: true,
      dns_suffix: default_dns_suffix(),
      dns_port: default_dns_port(),
      proxy_http_port: default_proxy_http_port(),
      proxy_https_port: default_proxy_https_port(),
    }
  }
}

impl AppSettings {
  /// Get the platform-specific settings file path
  /// - macOS: `~/Library/Application Support/dockside/settings.json`
  /// - Linux: `~/.config/dockside/settings.json` (`XDG_CONFIG_HOME`)
  /// - Windows: `%APPDATA%/dockside/settings.json`
  fn config_path() -> PathBuf {
    get_config_dir().join("settings.json")
  }

  pub fn load() -> Self {
    let path = Self::config_path();
    if path.exists() {
      match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
          Ok(settings) => return settings,
          Err(e) => tracing::warn!("Failed to parse settings: {}", e),
        },
        Err(e) => tracing::warn!("Failed to read settings file: {}", e),
      }
    }
    Self::default()
  }

  pub fn save(&self) -> Result<()> {
    let path = Self::config_path();
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(self)?;
    fs::write(path, content)?;
    Ok(())
  }
}

/// Events emitted when settings change
#[derive(Debug, Clone)]
pub enum SettingsChanged {
  ThemeChanged,
  SettingsUpdated,
}

/// Global settings state
pub struct SettingsState {
  pub settings: AppSettings,
}

impl SettingsState {
  pub fn new() -> Self {
    Self {
      settings: AppSettings::load(),
    }
  }
}

impl EventEmitter<SettingsChanged> for SettingsState {}

/// Global wrapper for settings state
struct GlobalSettingsState(Entity<SettingsState>);

impl Global for GlobalSettingsState {}

pub fn init_settings(cx: &mut App) {
  let state = cx.new(|_cx| SettingsState::new());
  cx.set_global(GlobalSettingsState(state));
}

pub fn settings_state(cx: &App) -> Entity<SettingsState> {
  cx.global::<GlobalSettingsState>().0.clone()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_theme_name_default() {
    assert_eq!(ThemeName::default(), ThemeName::TokyoNight);
  }

  #[test]
  fn test_theme_name_all_returns_all_variants() {
    let themes = ThemeName::all();
    // Should have a reasonable number of themes
    assert!(themes.len() >= 30);

    // Should include default theme
    assert!(themes.contains(&ThemeName::TokyoNight));

    // Should include dark themes
    assert!(themes.contains(&ThemeName::GruvboxDark));
    assert!(themes.contains(&ThemeName::CatppuccinMocha));

    // Should include light themes
    assert!(themes.contains(&ThemeName::GruvboxLight));
    assert!(themes.contains(&ThemeName::CatppuccinLatte));
  }

  #[test]
  fn test_theme_name_all_no_duplicates() {
    let themes = ThemeName::all();
    let mut seen: Vec<ThemeName> = themes.clone();
    seen.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    seen.dedup();
    assert_eq!(themes.len(), seen.len(), "all() should not contain duplicates");
  }

  #[test]
  fn test_theme_name_display_name() {
    assert_eq!(ThemeName::TokyoNight.display_name(), "Tokyo Night");
    assert_eq!(ThemeName::TokyoStorm.display_name(), "Tokyo Storm");
    assert_eq!(ThemeName::CatppuccinMocha.display_name(), "Catppuccin Mocha");
    assert_eq!(ThemeName::GruvboxDark.display_name(), "Gruvbox Dark");
    assert_eq!(ThemeName::GruvboxLight.display_name(), "Gruvbox Light");
    assert_eq!(ThemeName::Matrix.display_name(), "Matrix");
  }

  #[test]
  fn test_theme_name_theme_name_equals_display_name() {
    // theme_name() should return the same as display_name()
    for theme in ThemeName::all() {
      assert_eq!(theme.theme_name(), theme.display_name());
    }
  }

  #[test]
  fn test_app_settings_default() {
    let settings = AppSettings::default();
    assert_eq!(settings.theme, ThemeName::TokyoNight);
    assert!(settings.docker_socket.is_empty());
    assert_eq!(settings.default_colima_profile, "default");
    assert_eq!(settings.container_refresh_interval, 5);
    assert_eq!(settings.stats_refresh_interval, 2);
    assert_eq!(settings.max_log_lines, 1000);
    assert!((settings.terminal_font_size - 14.0).abs() < 0.01);
    assert!((settings.terminal_line_height - 1.4).abs() < 0.01);
    assert_eq!(settings.terminal_cursor_style, TerminalCursorStyle::Bar);
    assert!(settings.terminal_cursor_blink);
    assert_eq!(settings.terminal_scrollback_lines, 10000);
  }

  #[test]
  fn test_app_settings_serialization() {
    let settings = AppSettings::default();
    let json = serde_json::to_string(&settings).expect("Failed to serialize");

    // Should contain expected fields
    assert!(json.contains("theme"));
    assert!(json.contains("docker_socket"));
    assert!(json.contains("container_refresh_interval"));

    // Should be able to deserialize back
    let deserialized: AppSettings = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.theme, settings.theme);
    assert_eq!(deserialized.max_log_lines, settings.max_log_lines);
  }

  #[test]
  fn test_app_settings_custom_values() {
    let settings = AppSettings {
      theme: ThemeName::GruvboxDark,
      docker_socket: "/custom/docker.sock".to_string(),
      default_colima_profile: "dev".to_string(),
      colima_enabled: true,
      kubernetes_enabled: true,
      container_refresh_interval: 10,
      stats_refresh_interval: 5,
      max_log_lines: 5000,
      terminal_font_size: 16.0,
      terminal_line_height: 1.5,
      terminal_cursor_style: TerminalCursorStyle::Underline,
      terminal_cursor_blink: false,
      terminal_scrollback_lines: 5000,
      terminal_font_family: String::new(),
      external_editor: ExternalEditor::Cursor,
      editor_wait_close: false,
      confirm_destructive: true,
      show_notifications: true,
      default_pull_platform: String::new(),
      kubeconfig_path: String::new(),
      default_namespace: "default".to_string(),
      colima_default_cpus: 2,
      colima_default_memory_gb: 4,
      colima_default_disk_gb: 60,
      favorites: Vec::new(),
      dns_enabled: false,
      dns_autostart: true,
      dns_suffix: "dockside.test".to_string(),
      dns_port: 15353,
      proxy_http_port: 47080,
      proxy_https_port: 47443,
    };

    assert_eq!(settings.theme, ThemeName::GruvboxDark);
    assert_eq!(settings.docker_socket, "/custom/docker.sock");
    assert_eq!(settings.default_colima_profile, "dev");
    assert_eq!(settings.container_refresh_interval, 10);
    assert_eq!(settings.external_editor, ExternalEditor::Cursor);
  }

  #[test]
  fn test_terminal_cursor_style_default() {
    assert_eq!(TerminalCursorStyle::default(), TerminalCursorStyle::Bar);
  }

  #[test]
  fn test_terminal_cursor_style_all() {
    let styles = TerminalCursorStyle::all();
    assert_eq!(styles.len(), 3);
    assert!(styles.contains(&TerminalCursorStyle::Block));
    assert!(styles.contains(&TerminalCursorStyle::Underline));
    assert!(styles.contains(&TerminalCursorStyle::Bar));
  }

  #[test]
  fn test_terminal_cursor_style_display_name() {
    assert_eq!(TerminalCursorStyle::Block.display_name(), "Block");
    assert_eq!(TerminalCursorStyle::Underline.display_name(), "Underline");
    assert_eq!(TerminalCursorStyle::Bar.display_name(), "Bar");
  }

  #[test]
  fn test_settings_changed_variants() {
    // Just verify all variants can be created
    let _ = SettingsChanged::ThemeChanged;
    let _ = SettingsChanged::SettingsUpdated;
  }

  #[test]
  fn test_theme_name_serialization() {
    // Test that themes serialize/deserialize correctly
    let theme = ThemeName::CatppuccinMocha;
    let json = serde_json::to_string(&theme).expect("Failed to serialize");
    let deserialized: ThemeName = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized, theme);
  }

  #[test]
  fn test_all_themes_have_display_names() {
    for theme in ThemeName::all() {
      let name = theme.display_name();
      assert!(!name.is_empty(), "Theme {theme:?} should have a display name");
    }
  }

  #[test]
  fn test_colima_enabled_default() {
    let settings = AppSettings::default();
    // On macOS, colima should be enabled by default
    // On Linux, it should be disabled by default
    #[cfg(target_os = "macos")]
    assert!(settings.colima_enabled);
    #[cfg(target_os = "linux")]
    assert!(!settings.colima_enabled);
  }

  #[test]
  fn test_colima_enabled_deserialization() {
    // Test that old settings files without colima_enabled still work
    let json = r#"{
      "theme": "TokyoNight",
      "docker_socket": "",
      "default_colima_profile": "default",
      "container_refresh_interval": 5,
      "stats_refresh_interval": 2,
      "max_log_lines": 1000,
      "terminal_font_size": 14.0,
      "terminal_line_height": 1.4,
      "terminal_cursor_style": "Block",
      "terminal_cursor_blink": true,
      "terminal_scrollback_lines": 10000,
      "external_editor": "VSCode"
    }"#;
    let settings: AppSettings = serde_json::from_str(json).expect("Failed to deserialize");
    // Should use default value when field is missing
    assert_eq!(settings.colima_enabled, default_colima_enabled());
  }

  #[test]
  fn test_dark_themes_listed_first() {
    let themes = ThemeName::all();
    // First theme should be Tokyo Night (dark)
    assert_eq!(themes[0], ThemeName::TokyoNight);

    // Light themes should be near the end
    let latte_pos = themes.iter().position(|t| *t == ThemeName::CatppuccinLatte);
    let mocha_pos = themes.iter().position(|t| *t == ThemeName::CatppuccinMocha);
    assert!(
      mocha_pos < latte_pos,
      "Dark themes (Mocha) should come before light themes (Latte)"
    );
  }
}
