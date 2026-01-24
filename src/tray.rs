use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use rust_embed::RustEmbed;
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::platform::Platform;

/// Initialize GTK on Linux (required for tray icon menus)
#[cfg(target_os = "linux")]
fn init_gtk() {
    // Initialize GTK if not already done
    // This is required for tray-icon and muda to work on Linux
    if gtk::is_initialized() {
        return;
    }
    if let Err(e) = gtk::init() {
        tracing::warn!("Failed to initialize GTK for tray icon: {}", e);
    }
}

#[cfg(not(target_os = "linux"))]
fn init_gtk() {
    // No-op on non-Linux platforms
}

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

/// The tray icon manager
pub struct AppTray {
    _tray_icon: TrayIcon,
}

impl AppTray {
    /// Create and initialize the system tray icon
    pub fn new() -> Self {
        // Initialize GTK on Linux (required for menus)
        init_gtk();

        let menu = create_tray_menu();
        let icon = load_tray_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Dockside - Docker Management")
            .with_icon(icon)
            .with_menu_on_left_click(true)
            .build()
            .expect("Failed to create tray icon");

        Self { _tray_icon: tray_icon }
    }
}

impl Default for AppTray {
    fn default() -> Self {
        Self::new()
    }
}
