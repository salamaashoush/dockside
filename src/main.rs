mod actions;
mod app;
mod assets;
mod colima;
mod docker;
mod kubernetes;
mod services;
mod state;
mod terminal;
mod tray;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use gpui::{px, size, App, AppContext, Bounds, SharedString, Timer, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::{theme::{Theme, ThemeRegistry}, Root};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use state::{AppSettings, CurrentView};
use tray::{menu_ids, AppTray};

pub use assets::Assets;

/// Get the themes directory path, checking multiple locations:
/// 1. Bundle Resources/themes (for .app bundle)
/// 2. ./themes (for development)
fn get_themes_dir() -> Option<PathBuf> {
    // Try to find themes relative to executable (for .app bundle)
    if let Ok(exe_path) = std::env::current_exe() {
        // In a .app bundle: Contents/MacOS/deckhand -> Contents/Resources/themes
        if let Some(macos_dir) = exe_path.parent() {
            let resources_themes = macos_dir
                .parent() // Contents
                .map(|p| p.join("Resources").join("themes"));

            if let Some(path) = resources_themes {
                if path.exists() {
                    return Some(path);
                }
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

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    gpui::Application::new()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            // Initialize gpui-component
            gpui_component::init(cx);

            // Load saved settings to get the user's preferred theme
            let settings = AppSettings::load();
            let saved_theme_name = SharedString::from(settings.theme.theme_name().to_string());

            // Watch themes directory and apply the saved theme
            if let Some(themes_dir) = get_themes_dir() {
                if let Err(err) = ThemeRegistry::watch_dir(
                    themes_dir,
                    cx,
                    move |cx| {
                        if let Some(theme) = ThemeRegistry::global(cx)
                            .themes()
                            .get(&saved_theme_name)
                            .cloned()
                        {
                            Theme::global_mut(cx).apply_config(&theme);
                        }
                    },
                ) {
                    tracing::warn!("Failed to watch themes directory: {}", err);
                }
            } else {
                tracing::warn!("Themes directory not found");
            }

            // Initialize global services
            services::init_services(cx);

            // Load initial data
            services::load_initial_data(cx);

            let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Deckhand".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(gpui::point(px(9.), px(9.))),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| app::DockerApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("Failed to open window");

            // Create tray icon after window is opened (deferred to ensure main thread is ready)
            cx.spawn(async move |mut cx| {
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

    Ok(())
}

/// Handle tray menu events
fn handle_tray_menu_event(id: &str, cx: &mut App) {
    match id {
        menu_ids::SHOW_APP => {
            // Activate the app and bring window to front
            cx.activate(true);
        }
        menu_ids::CONTAINERS => {
            cx.activate(true);
            services::set_view(CurrentView::Containers, cx);
        }
        menu_ids::COMPOSE => {
            cx.activate(true);
            services::set_view(CurrentView::Compose, cx);
        }
        menu_ids::VOLUMES => {
            cx.activate(true);
            services::set_view(CurrentView::Volumes, cx);
        }
        menu_ids::IMAGES => {
            cx.activate(true);
            services::set_view(CurrentView::Images, cx);
        }
        menu_ids::NETWORKS => {
            cx.activate(true);
            services::set_view(CurrentView::Networks, cx);
        }
        menu_ids::ACTIVITY => {
            cx.activate(true);
            services::set_view(CurrentView::ActivityMonitor, cx);
        }
        menu_ids::MACHINES => {
            cx.activate(true);
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
