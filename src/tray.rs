use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use rust_embed::RustEmbed;
use tray_icon::{TrayIcon, TrayIconBuilder};

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

/// Create the tray menu
fn create_tray_menu() -> Menu {
    let menu = Menu::new();

    // Show App
    let show_app = MenuItem::with_id(menu_ids::SHOW_APP, "Open Deckhand", true, None);
    menu.append(&show_app).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Docker section
    let docker_submenu = Submenu::new("Docker", true);
    docker_submenu
        .append(&MenuItem::with_id(
            menu_ids::CONTAINERS,
            "Containers",
            true,
            None,
        ))
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
        .append(&MenuItem::with_id(
            menu_ids::NETWORKS,
            "Networks",
            true,
            None,
        ))
        .unwrap();
    menu.append(&docker_submenu).unwrap();

    // General section
    let general_submenu = Submenu::new("General", true);
    general_submenu
        .append(&MenuItem::with_id(
            menu_ids::ACTIVITY,
            "Activity Monitor",
            true,
            None,
        ))
        .unwrap();
    general_submenu
        .append(&MenuItem::with_id(
            menu_ids::MACHINES,
            "Machines",
            true,
            None,
        ))
        .unwrap();
    menu.append(&general_submenu).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Colima controls
    let colima_submenu = Submenu::new("Colima", true);
    colima_submenu
        .append(&MenuItem::with_id(
            menu_ids::START_COLIMA,
            "Start",
            true,
            None,
        ))
        .unwrap();
    colima_submenu
        .append(&MenuItem::with_id(
            menu_ids::STOP_COLIMA,
            "Stop",
            true,
            None,
        ))
        .unwrap();
    colima_submenu
        .append(&MenuItem::with_id(
            menu_ids::RESTART_COLIMA,
            "Restart",
            true,
            None,
        ))
        .unwrap();
    menu.append(&colima_submenu).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // Quit
    let quit = MenuItem::with_id(menu_ids::QUIT, "Quit Deckhand", true, None);
    menu.append(&quit).unwrap();

    menu
}

/// Embedded app icon for tray
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icon.png"]
struct TrayAssets;

/// Load and resize the app icon for the tray (22x22 for macOS menu bar)
fn load_tray_icon() -> tray_icon::Icon {
    // Load icon.png from embedded assets and resize for menu bar
    if let Some(icon_data) = TrayAssets::get("icon.png") {
        if let Ok(img) = image::load_from_memory(&icon_data.data) {
            // Resize to 22x22 for macOS menu bar (44x44 for retina would be better but 22 works)
            let resized = image::imageops::resize(
                &img.into_rgba8(),
                22,
                22,
                image::imageops::FilterType::Lanczos3,
            );
            let (width, height) = resized.dimensions();
            let rgba = resized.into_raw();
            if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, width, height) {
                return icon;
            }
        }
    }

    // Fallback: create a simple placeholder if icon loading fails
    let size = 22u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for i in (0..rgba.len()).step_by(4) {
        rgba[i] = 100;     // R
        rgba[i + 1] = 100; // G
        rgba[i + 2] = 100; // B
        rgba[i + 3] = 255; // A
    }
    tray_icon::Icon::from_rgba(rgba, size, size).expect("Failed to create fallback icon")
}

/// The tray icon manager
pub struct AppTray {
    _tray_icon: TrayIcon,
}

impl AppTray {
    /// Create and initialize the system tray icon
    pub fn new() -> Self {
        let menu = create_tray_menu();
        let icon = load_tray_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Deckhand - Docker Management")
            .with_icon(icon)
            .with_menu_on_left_click(true)
            .build()
            .expect("Failed to create tray icon");

        Self {
            _tray_icon: tray_icon,
        }
    }
}

impl Default for AppTray {
    fn default() -> Self {
        Self::new()
    }
}
