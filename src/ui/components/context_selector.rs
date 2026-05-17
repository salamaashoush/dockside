//! The cluster anchor shown on every k8s view. Always visible (even with
//! one or zero contexts) so "which cluster am I on" is never ambiguous —
//! the same role Lens' cluster badge plays. The dropdown switches context
//! and links straight into the Clusters manager, tying the whole k8s
//! surface together as one app.

use gpui::{App, IntoElement};
use gpui_component::{
  Sizable,
  button::Button,
  menu::{DropdownMenu, PopupMenuItem},
};

use crate::services;
use crate::state::{CurrentView, docker_state};

pub fn render_context_selector(cx: &App) -> impl IntoElement {
  let state = docker_state(cx).read(cx);
  let contexts = state.kube_contexts.clone();
  let current = state.current_kube_context_name();

  let display = match &current {
    Some(name) => format!("⎈ {name}"),
    None => "⎈ no cluster".to_string(),
  };

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
        .separator()
        .item(PopupMenuItem::new("Manage clusters…").on_click(|_, _, cx| {
          services::set_view(CurrentView::Clusters, cx);
        }))
    })
}
