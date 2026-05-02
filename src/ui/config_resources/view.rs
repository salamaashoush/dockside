//! Tabbed wrapper for k8s config resources: `ConfigMaps`, `Secrets`.

use gpui::{Context, Entity, Render, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Selectable, h_flex,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::ui::components::render_namespace_selector;
use crate::ui::configmaps::ConfigMapsView;
use crate::ui::secrets::SecretsView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigTab {
  #[default]
  ConfigMaps,
  Secrets,
}

pub struct ConfigResourcesView {
  active_tab: ConfigTab,
  configmaps: Entity<ConfigMapsView>,
  secrets: Entity<SecretsView>,
}

impl ConfigResourcesView {
  pub fn new(configmaps: Entity<ConfigMapsView>, secrets: Entity<SecretsView>) -> Self {
    Self {
      active_tab: ConfigTab::ConfigMaps,
      configmaps,
      secrets,
    }
  }
}

impl Render for ConfigResourcesView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let colors = cx.theme().colors;
    let active = self.active_tab;

    let tab_bar = TabBar::new("config-tabs")
      .child(
        Tab::new()
          .label("ConfigMaps")
          .selected(active == ConfigTab::ConfigMaps)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ConfigTab::ConfigMaps;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Secrets")
          .selected(active == ConfigTab::Secrets)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ConfigTab::Secrets;
            cx.notify();
          })),
      );

    let body = match active {
      ConfigTab::ConfigMaps => div().size_full().child(self.configmaps.clone()),
      ConfigTab::Secrets => div().size_full().child(self.secrets.clone()),
    };

    v_flex()
      .size_full()
      .child(
        h_flex()
          .w_full()
          .items_center()
          .flex_shrink_0()
          .border_b_1()
          .border_color(colors.border)
          .child(tab_bar)
          .child(h_flex().pr(px(12.)).child(render_namespace_selector(cx))),
      )
      .child(div().flex_1().min_h_0().child(body))
  }
}
