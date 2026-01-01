mod dispatcher;
mod gpui_tokio;
mod task_manager;

pub use dispatcher::*;
pub use gpui_tokio::Tokio;
pub use task_manager::*;

use gpui::App;

use crate::state::init_docker_state;

/// Initialize all global services
pub fn init_services(cx: &mut App) {
    // Initialize tokio runtime first (required for Docker client)
    gpui_tokio::init(cx);

    // Initialize state
    init_docker_state(cx);

    // Initialize services
    init_task_manager(cx);
    init_dispatcher(cx);
}
