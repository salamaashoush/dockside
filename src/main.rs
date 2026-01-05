mod actions;
mod app;
mod assets;
mod colima;
mod docker;
mod keybindings;
mod kubernetes;
mod services;
mod state;
mod terminal;
mod tray;
mod ui;
mod utils;

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
  App, AppContext, Bounds, SharedString, Timer, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions, px, size,
};
use gpui_component::{
  Root,
  theme::{Theme, ThemeConfig, ThemeRegistry, ThemeSet},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use state::{AppSettings, CurrentView};
use tray::{AppTray, menu_ids};

pub use assets::Assets;

/// Open the main application window
fn open_main_window(cx: &mut App) -> WindowHandle<Root> {
  let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);

  let handle = cx
    .open_window(
      WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
          title: Some("Dockside".into()),
          appears_transparent: true,
          traffic_light_position: Some(gpui::point(px(9.), px(9.))),
        }),
        ..Default::default()
      },
      |window, cx| {
        let view = cx.new(|cx| app::DocksideApp::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
      },
    )
    .expect("Failed to open window");

  // Activate the window to ensure it receives keyboard focus
  cx.activate(true);

  handle
}

/// Get the themes directory path, checking multiple locations:
/// 1. Bundle Resources/themes (for .app bundle)
/// 2. ./themes (for development)
fn get_themes_dir() -> Option<PathBuf> {
  // Try to find themes relative to executable (for .app bundle)
  if let Ok(exe_path) = std::env::current_exe() {
    // In a .app bundle: Contents/MacOS/dockside -> Contents/Resources/themes
    if let Some(macos_dir) = exe_path.parent() {
      let resources_themes = macos_dir
                .parent() // Contents
                .map(|p| p.join("Resources").join("themes"));

      if let Some(path) = resources_themes
        && path.exists()
      {
        return Some(path);
      }
    }
  }

  // Fallback to ./themes for development
  let dev_themes = PathBuf::from("./themes");
  if dev_themes.exists() {
    return Some(dev_themes);
  }

  None
}

/// Load a specific theme from JSON files synchronously
fn load_theme_sync(themes_dir: &PathBuf, theme_name: &str) -> Option<Rc<ThemeConfig>> {
  if !themes_dir.exists() {
    return None;
  }

  // Read all JSON files in themes directory
  let entries = std::fs::read_dir(themes_dir).ok()?;

  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
      continue;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
      continue;
    };
    let Ok(theme_set) = serde_json::from_str::<ThemeSet>(&content) else {
      continue;
    };
    for theme in theme_set.themes {
      if theme.name == theme_name {
        return Some(Rc::new(theme));
      }
    }
  }

  None
}

fn main() {
  tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer())
    .init();

  let app = gpui::Application::new().with_assets(Assets);

  // Handle dock icon click when no windows are open
  app.on_reopen(|cx| {
    if cx.windows().is_empty() {
      open_main_window(cx);
    } else {
      cx.activate(true);
    }
  });

  app.run(|cx: &mut App| {
    // Initialize gpui-component
    gpui_component::init(cx);

    // Load saved settings to get the user's preferred theme
    let settings = AppSettings::load();
    let saved_theme_name = settings.theme.theme_name().to_string();

    // Load and apply theme SYNCHRONOUSLY before window opens (prevents flicker)
    if let Some(themes_dir) = get_themes_dir() {
      // Load saved theme immediately
      if let Some(theme_config) = load_theme_sync(&themes_dir, &saved_theme_name) {
        Theme::global_mut(cx).apply_config(&theme_config);
      }

      // Also set up file watcher for hot reload (callback updates if theme files change)
      let theme_name_for_watch = SharedString::from(saved_theme_name.clone());
      if let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name_for_watch).cloned() {
          Theme::global_mut(cx).apply_config(&theme);
          cx.refresh_windows();
        }
      }) {
        tracing::warn!("Failed to watch themes directory: {}", err);
      }
    } else {
      tracing::warn!("Themes directory not found");
    }

    // Initialize global services
    services::init_services(cx);

    // Register keyboard shortcuts
    keybindings::register_keybindings(cx);

    // Register command palette keybindings
    ui::command_palette::init(cx);

    // Register global search keybindings
    ui::global_search::init(cx);

    // Load initial data
    services::load_initial_data(cx);

    // Start real-time resource watchers for automatic UI updates
    services::start_watchers(cx);

    // Open the main window
    open_main_window(cx);

    // Create tray icon after window is opened (deferred to ensure main thread is ready)
    cx.spawn(async move |cx| {
      // Small delay to ensure the app is fully initialized
      Timer::after(Duration::from_millis(500)).await;

      // Create tray on main thread via update
      let _ = cx.update(|_cx| {
        // Create the system tray icon
        // Note: We leak the tray to keep it alive for the app's lifetime
        let tray = Box::new(AppTray::new());
        Box::leak(tray);
      });

      // Then start polling for menu events
      let menu_receiver = muda::MenuEvent::receiver();
      loop {
        // Check for menu events
        if let Ok(event) = menu_receiver.try_recv() {
          let id = event.id.0.as_str();
          let _ = cx.update(|cx| {
            handle_tray_menu_event(id, cx);
          });
        }
        // Small delay to prevent busy-waiting
        Timer::after(Duration::from_millis(100)).await;
      }
    })
    .detach();
  });
}

/// Ensure window exists and activate it
fn ensure_window_and_activate(cx: &mut App) {
  if cx.windows().is_empty() {
    open_main_window(cx);
  }
  cx.activate(true);
}

/// Handle tray menu events
fn handle_tray_menu_event(id: &str, cx: &mut App) {
  match id {
    menu_ids::SHOW_APP => {
      ensure_window_and_activate(cx);
    }
    menu_ids::CONTAINERS => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::Containers, cx);
    }
    menu_ids::COMPOSE => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::Compose, cx);
    }
    menu_ids::VOLUMES => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::Volumes, cx);
    }
    menu_ids::IMAGES => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::Images, cx);
    }
    menu_ids::NETWORKS => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::Networks, cx);
    }
    menu_ids::ACTIVITY => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::ActivityMonitor, cx);
    }
    menu_ids::MACHINES => {
      ensure_window_and_activate(cx);
      services::set_view(CurrentView::Machines, cx);
    }
    menu_ids::START_COLIMA => {
      services::start_colima(None, cx);
    }
    menu_ids::STOP_COLIMA => {
      services::stop_colima(None, cx);
    }
    menu_ids::RESTART_COLIMA => {
      services::restart_colima(None, cx);
    }
    menu_ids::QUIT => {
      cx.quit();
    }
    _ => {}
  }
}
