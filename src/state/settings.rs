use anyhow::Result;
use gpui::{App, AppContext, Entity, EventEmitter, Global};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
  /// Selected theme
  pub theme: ThemeName,
  /// Docker socket path (empty for default)
  pub docker_socket: String,
  /// Default Colima profile name
  pub default_colima_profile: String,
  /// Refresh interval for containers (in seconds)
  pub container_refresh_interval: u64,
  /// Refresh interval for stats (in seconds)
  pub stats_refresh_interval: u64,
  /// Maximum log lines to display
  pub max_log_lines: usize,
  /// Terminal font size
  pub terminal_font_size: f32,
  /// External editor for opening files
  pub external_editor: ExternalEditor,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      theme: ThemeName::TokyoNight,
      docker_socket: String::new(),
      default_colima_profile: "default".to_string(),
      container_refresh_interval: 5,
      stats_refresh_interval: 2,
      max_log_lines: 1000,
      terminal_font_size: 14.0,
      external_editor: ExternalEditor::default(),
    }
  }
}

impl AppSettings {
  fn config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("docker-ui").join("settings.json")
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
      container_refresh_interval: 10,
      stats_refresh_interval: 5,
      max_log_lines: 5000,
      terminal_font_size: 16.0,
      external_editor: ExternalEditor::Cursor,
    };

    assert_eq!(settings.theme, ThemeName::GruvboxDark);
    assert_eq!(settings.docker_socket, "/custom/docker.sock");
    assert_eq!(settings.default_colima_profile, "dev");
    assert_eq!(settings.container_refresh_interval, 10);
    assert_eq!(settings.external_editor, ExternalEditor::Cursor);
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
