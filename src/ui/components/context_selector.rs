//! Shared kubeconfig context dropdown, mounted next to the namespace
//! selector in every k8s parent group view. Switching writes
//! `settings.kube_context` and triggers a full per-cluster reload.

use gpui::{App, IntoElement, ParentElement, div, prelude::FluentBuilder};
use gpui_component::{
  Sizable,
  button::Button,
  menu::{DropdownMenu, PopupMenuItem},
};

use crate::services;
use crate::state::docker_state;

/// Render the global context dropdown. Hidden entirely when the kubeconfig
/// holds fewer than two contexts — a single-cluster user never needs it.
pub fn render_context_selector(cx: &App) -> impl IntoElement {
  let state = docker_state(cx).read(cx);
  let contexts = state.kube_contexts.clone();
  let current = state.current_kube_context_name();

  let multi = contexts.len() > 1;
  let display = match &current {
    Some(name) => format!("Context: {name}"),
    None => "Context: default".to_string(),
  };

  div().when(multi, move |el| {
    el.child(
      Button::new("context-selector")
        .label(display)
        .outline()
        .small()
        .dropdown_caret(true)
        .dropdown_menu(move |menu, _window, _cx| {
          let mut menu = menu;
          for ctx in &contexts {
            let name = ctx.name.clone();
            let is_active = current.as_deref() == Some(name.as_str());
            let label = if is_active {
              format!("● {name}")
            } else {
              format!("   {name}")
            };
            menu = menu.item(PopupMenuItem::new(label).on_click({
              let name = name.clone();
              move |_, _, cx| {
                services::set_kube_context(name.clone(), cx);
              }
            }));
          }
          menu
        }),
    )
  })
}
