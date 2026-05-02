//! Tabbed wrapper for k8s storage resources. Currently only PVCs;
//! mirrors the Workloads / Networking / Config layout so the namespace
//! selector lives in the same place across all k8s group views.

use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Selectable, h_flex,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::ui::components::render_namespace_selector;
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
    let colors = cx.theme().colors;
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
      .child(
        h_flex()
          .w_full()
          .items_center()
          .flex_shrink_0()
          .bg(colors.tab_bar)
          .border_b_1()
          .border_color(colors.border)
          .child(div().flex_1().min_w_0().overflow_hidden().child(tab_bar))
          .child(
            h_flex()
              .px(px(12.))
              .flex_shrink_0()
              .child(render_namespace_selector(cx)),
          ),
      )
      .child(div().flex_1().min_h_0().child(body))
  }
}
