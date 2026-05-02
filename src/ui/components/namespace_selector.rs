//! Shared namespace dropdown used by k8s parent group views (Workloads,
//! Networking, Config). Reads/writes the global `selected_namespace`
//! on `DockerState`, so a change in any group is reflected everywhere.

use gpui::{App, IntoElement};
use gpui_component::{
  button::{Button, ButtonVariants},
  menu::{DropdownMenu, PopupMenuItem},
};

use crate::services;
use crate::state::docker_state;

/// Render the global namespace dropdown. Always reflects the current
/// `selected_namespace` from `DockerState` and dispatches
/// `services::set_namespace` on click.
pub fn render_namespace_selector(cx: &App) -> impl IntoElement {
  let state = docker_state(cx).read(cx);
  let selected = state.selected_namespace.clone();
  let namespaces = state.namespaces.clone();

  let display = if selected == "all" { "All".to_string() } else { selected };

  Button::new("namespace-selector")
    .label(display)
    .ghost()
    .compact()
    .dropdown_menu(move |menu, _window, _cx| {
      let mut menu = menu.item(PopupMenuItem::new("All Namespaces").on_click(|_, _, cx| {
        services::set_namespace("all".to_string(), cx);
      }));
      if !namespaces.is_empty() {
        menu = menu.separator();
        for ns in &namespaces {
          let ns_clone = ns.clone();
          menu = menu.item(PopupMenuItem::new(ns.clone()).on_click({
            let ns = ns_clone.clone();
            move |_, _, cx| {
              services::set_namespace(ns.clone(), cx);
            }
          }));
        }
      }
      menu
    })
}
