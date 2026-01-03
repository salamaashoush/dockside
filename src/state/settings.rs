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

  /// Returns the theme name as used in the JSON file (for ThemeRegistry lookup)
  pub fn theme_name(&self) -> &str {
    self.display_name()
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
