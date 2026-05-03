use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use rust_embed::RustEmbed;
#[cfg(not(target_os = "linux"))]
use tray_icon::TrayIcon;
use tray_icon::TrayIconBuilder;

use crate::platform::Platform;

/// Menu item IDs for handling events
pub mod menu_ids {
  pub const SHOW_APP: &str = "show_app";
  pub const CONTAINERS: &str = "containers";
  pub const COMPOSE: &str = "compose";
  pub const VOLUMES: &str = "volumes";
  pub const IMAGES: &str = "images";
  pub const NETWORKS: &str = "networks";
  pub const ACTIVITY: &str = "activity";
  pub const MACHINES: &str = "machines";
  pub const START_COLIMA: &str = "start_colima";
  pub const STOP_COLIMA: &str = "stop_colima";
  pub const RESTART_COLIMA: &str = "restart_colima";
  pub const QUIT: &str = "quit";
}

/// Create the tray menu with platform-specific items
fn create_tray_menu() -> Menu {
  let menu = Menu::new();
  let platform = Platform::detect();

  // Show App
  let show_app = MenuItem::with_id(menu_ids::SHOW_APP, "Open Dockside", true, None);
  menu.append(&show_app).unwrap();

  menu.append(&PredefinedMenuItem::separator()).unwrap();

  // Docker section
  let docker_submenu = Submenu::new("Docker", true);
  docker_submenu
    .append(&MenuItem::with_id(menu_ids::CONTAINERS, "Containers", true, None))
    .unwrap();
  docker_submenu
    .append(&MenuItem::with_id(menu_ids::COMPOSE, "Compose", true, None))
    .unwrap();
  docker_submenu
    .append(&MenuItem::with_id(menu_ids::VOLUMES, "Volumes", true, None))
    .unwrap();
  docker_submenu
    .append(&MenuItem::with_id(menu_ids::IMAGES, "Images", true, None))
    .unwrap();
  docker_submenu
    .append(&MenuItem::with_id(menu_ids::NETWORKS, "Networks", true, None))
    .unwrap();
  menu.append(&docker_submenu).unwrap();

  // General section
  let general_submenu = Submenu::new("General", true);
  general_submenu
    .append(&MenuItem::with_id(menu_ids::ACTIVITY, "Activity Monitor", true, None))
    .unwrap();

  // Only show Machines menu on platforms that support Colima
  if platform.supports_colima() {
    general_submenu
      .append(&MenuItem::with_id(menu_ids::MACHINES, "Machines", true, None))
      .unwrap();
  }
  menu.append(&general_submenu).unwrap();

  menu.append(&PredefinedMenuItem::separator()).unwrap();

  // Colima controls - only on platforms that support it
  if platform.supports_colima() {
    let colima_submenu = Submenu::new("Colima", true);
    colima_submenu
      .append(&MenuItem::with_id(menu_ids::START_COLIMA, "Start", true, None))
      .unwrap();
    colima_submenu
      .append(&MenuItem::with_id(menu_ids::STOP_COLIMA, "Stop", true, None))
      .unwrap();
    colima_submenu
      .append(&MenuItem::with_id(menu_ids::RESTART_COLIMA, "Restart", true, None))
      .unwrap();
    menu.append(&colima_submenu).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();
  }

  // Quit
  let quit = MenuItem::with_id(menu_ids::QUIT, "Quit Dockside", true, None);
  menu.append(&quit).unwrap();

  menu
}

/// Embedded app icon for tray
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icon.png"]
struct TrayAssets;

/// Get the appropriate icon size for the current platform
fn get_tray_icon_size() -> u32 {
  let platform = Platform::detect();
  match platform {
    // macOS uses 22pt icons, 44px for retina (2x)
    Platform::MacOS => 44,
    // Linux typically uses 24px icons
    Platform::Linux | Platform::WindowsWsl2 => 24,
    // Windows uses 32px icons (or 16px for older systems)
    Platform::Windows => 32,
  }
}

/// Load and resize the app icon for the tray
fn load_tray_icon() -> tray_icon::Icon {
  // Load icon.png from embedded assets
  let icon_data = TrayAssets::get("icon.png").expect("icon.png must be embedded in assets");

  let img = image::load_from_memory(&icon_data.data).expect("icon.png must be a valid image");

  // Use platform-specific icon size
  let size = get_tray_icon_size();
  let resized = image::imageops::resize(&img.into_rgba8(), size, size, image::imageops::FilterType::Lanczos3);
  let rgba = resized.into_raw();

  tray_icon::Icon::from_rgba(rgba, size, size).expect("Failed to create tray icon from RGBA data")
}

/// The tray icon manager. macOS only — gpui 0.2's Linux backend has
/// no `Window::hide()` and stops the run loop on the last window
/// close, so a tray-resident app is not viable there without forking
/// gpui. The whole `tray` module is `#[cfg(not(target_os = "linux"))]`
/// in `main.rs`.
#[cfg(not(target_os = "linux"))]
pub struct AppTray {
  _tray_icon: TrayIcon,
}

#[cfg(not(target_os = "linux"))]
impl AppTray {
  /// Create and initialize the system tray icon
  pub fn new() -> Self {
    tracing::info!("tray: AppTray::new() entered");

    let menu = create_tray_menu();
    tracing::info!("tray: menu built");
    let icon = load_tray_icon();
    tracing::info!("tray: icon loaded");

    // Stable SNI id so KDE Plasma / GNOME-AppIndicator can persist the
    // user's "Shown / Hidden" preference across app restarts. Without
    // this the tray-icon crate autogenerates a fresh UUID each launch,
    // so the panel forgets the chosen visibility every time.
    let build_result = TrayIconBuilder::new()
      .with_id("dev.dockside.app")
      .with_menu(Box::new(menu))
      .with_tooltip("Dockside - Docker Management")
      .with_icon(icon)
      .with_menu_on_left_click(true)
      .build();
    let tray_icon = match build_result {
      Ok(t) => {
        tracing::info!("tray: TrayIconBuilder::build succeeded");
        t
      }
      Err(e) => {
        tracing::error!("tray: TrayIconBuilder::build failed: {e}");
        panic!("Failed to create tray icon: {e}");
      }
    };

    Self { _tray_icon: tray_icon }
  }
}

#[cfg(not(target_os = "linux"))]
impl Default for AppTray {
  fn default() -> Self {
    Self::new()
  }
}
