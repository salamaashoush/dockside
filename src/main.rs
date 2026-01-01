mod actions;
mod app;
mod assets;
mod colima;
mod docker;
mod services;
mod state;
mod terminal;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use gpui::{px, size, App, AppContext, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowOptions};
use gpui_component::{theme::{Theme, ThemeRegistry}, Root};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    gpui::Application::new()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            // Initialize gpui-component
            gpui_component::init(cx);

            // Load Tokyo Night theme from themes directory
            let theme_name = SharedString::from("Tokyo Night");
            if let Err(err) = ThemeRegistry::watch_dir(
                PathBuf::from("./themes"),
                cx,
                move |cx| {
                    if let Some(theme) = ThemeRegistry::global(cx)
                        .themes()
                        .get(&theme_name)
                        .cloned()
                    {
                        Theme::global_mut(cx).apply_config(&theme);
                    }
                },
            ) {
                tracing::warn!("Failed to watch themes directory: {}", err);
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
                        title: Some("Docker UI".into()),
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
        });

    Ok(())
}
