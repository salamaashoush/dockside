//! Tabbed wrapper for k8s storage resources. Currently only PVCs;
//! mirrors the Workloads / Networking / Config layout so the namespace
//! selector lives in the same place across all k8s group views.

use gpui::{Context, Entity, Render, Window, div, prelude::*};
use gpui_component::{
  Selectable,
  tab::{Tab, TabBar},
  v_flex,
};

use crate::ui::components::render_k8s_header;
use crate::ui::pvcs::PvcsView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageTab {
  #[default]
  Pvcs,
}

pub struct StorageView {
  active_tab: StorageTab,
  pvcs: Entity<PvcsView>,
}

impl StorageView {
  pub fn new(pvcs: Entity<PvcsView>) -> Self {
    Self {
      active_tab: StorageTab::Pvcs,
      pvcs,
    }
  }
}

impl Render for StorageView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let active = self.active_tab;

    let tab_bar =
      TabBar::new("storage-tabs").child(Tab::new().label("PVCs").selected(active == StorageTab::Pvcs).on_click(
        cx.listener(|this, _ev, _w, cx| {
          this.active_tab = StorageTab::Pvcs;
          cx.notify();
        }),
      ));

    let body = match active {
      StorageTab::Pvcs => div().size_full().child(self.pvcs.clone()),
    };

    v_flex()
      .size_full()
      .child(render_k8s_header(tab_bar, true, div(), cx))
      .child(div().flex_1().min_h_0().child(body))
  }
}
