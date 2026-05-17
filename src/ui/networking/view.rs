//! Tabbed wrapper for networking k8s resources: Services + Ingresses.

use gpui::{Context, Entity, Render, Window, div, prelude::*};
use gpui_component::{
  Selectable,
  tab::{Tab, TabBar},
  v_flex,
};

use crate::ui::components::render_k8s_header;
use crate::ui::ingresses::IngressesView;
use crate::ui::services::ServicesView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkingTab {
  #[default]
  Services,
  Ingresses,
}

pub struct NetworkingView {
  active_tab: NetworkingTab,
  services: Entity<ServicesView>,
  ingresses: Entity<IngressesView>,
}

impl NetworkingView {
  pub fn new(services: Entity<ServicesView>, ingresses: Entity<IngressesView>) -> Self {
    Self {
      active_tab: NetworkingTab::Services,
      services,
      ingresses,
    }
  }
}

impl Render for NetworkingView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let active = self.active_tab;

    let tab_bar = TabBar::new("networking-tabs")
      .child(
        Tab::new()
          .label("Services")
          .selected(active == NetworkingTab::Services)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = NetworkingTab::Services;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Ingresses")
          .selected(active == NetworkingTab::Ingresses)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = NetworkingTab::Ingresses;
            cx.notify();
          })),
      );

    let body = match active {
      NetworkingTab::Services => div().size_full().child(self.services.clone()),
      NetworkingTab::Ingresses => div().size_full().child(self.ingresses.clone()),
    };

    v_flex()
      .size_full()
      .child(render_k8s_header(tab_bar, true, div(), cx))
      .child(div().flex_1().min_h_0().child(body))
  }
}
