use gpui::{App, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::docker::NetworkInfo;

type NetworkActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct NetworkDetail {
  network: Option<NetworkInfo>,
  active_tab: usize,
  on_delete: Option<NetworkActionCallback>,
  on_tab_change: Option<TabChangeCallback>,
}

impl NetworkDetail {
  pub fn new() -> Self {
    Self {
      network: None,
      active_tab: 0,
      on_delete: None,
      on_tab_change: None,
    }
  }

  pub fn network(mut self, network: Option<NetworkInfo>) -> Self {
    self.network = network;
    self
  }

  pub fn active_tab(mut self, tab: usize) -> Self {
    self.active_tab = tab;
    self
  }

  pub fn on_delete<F>(mut self, callback: F) -> Self
  where
    F: Fn(&str, &mut Window, &mut App) + 'static,
  {
    self.on_delete = Some(Rc::new(callback));
    self
  }

  pub fn on_tab_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&usize, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  fn render_empty(cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .items_center()
      .justify_center()
      .child(
        v_flex()
          .items_center()
          .gap(px(16.))
          .child(
            Icon::new(IconName::Globe)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select a network to view details"),
          ),
      )
  }

  fn render_info_tab(network: &NetworkInfo, cx: &App) -> gpui::Div {
    let _colors = &cx.theme().colors;

    // Basic info rows
    let mut basic_info = vec![
      ("ID", network.short_id().to_string()),
      ("Name", network.name.clone()),
      ("Driver", network.driver.clone()),
      ("Scope", network.scope.clone()),
    ];

    if let Some(created) = network.created {
      basic_info.push(("Created", created.format("%Y-%m-%d %H:%M:%S").to_string()));
    }

    if network.internal {
      basic_info.push(("Internal", "Yes".to_string()));
    }

    let mut content = v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .gap(px(12.))
      .child(Self::render_section(None, basic_info, cx));

    // IPAM section
    if let Some(ref ipam) = network.ipam {
      let mut ipam_rows = Vec::new();

      if let Some(ref driver) = ipam.driver {
        ipam_rows.push(("Driver", driver.clone()));
      }

      if let Some(config) = ipam.config.first() {
        if let Some(ref subnet) = config.subnet {
          ipam_rows.push(("Subnet", subnet.clone()));
        }
        if let Some(ref gateway) = config.gateway {
          ipam_rows.push(("Gateway", gateway.clone()));
        }
        if let Some(ref ip_range) = config.ip_range {
          ipam_rows.push(("IP Range", ip_range.clone()));
        }
      }

      if !ipam_rows.is_empty() {
        content = content.child(Self::render_section(Some("IPAM Configuration"), ipam_rows, cx));
      }
    }

    // Connected containers section
    if !network.containers.is_empty() {
      content = content.child(Self::render_containers_section(network, cx));
    }

    // Labels section if not empty
    if !network.labels.is_empty() {
      content = content.child(Self::render_labels_section(network, cx));
    }

    // Options section if not empty
    if !network.options.is_empty() {
      content = content.child(Self::render_options_section(network, cx));
    }

    content
  }

  fn render_section(header: Option<&str>, rows: Vec<(&str, String)>, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut section = v_flex().gap(px(1.));

    if let Some(title) = header {
      section = section.child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child(title.to_string()),
      );
    }

    let rows_container = v_flex()
      .bg(colors.background)
      .rounded(px(8.))
      .overflow_hidden()
      .children(
        rows
          .into_iter()
          .enumerate()
          .map(|(i, (label, value))| Self::render_section_row(label, value, i == 0, cx)),
      );

    section.child(rows_container)
  }

  fn render_section_row(label: &str, value: String, is_first: bool, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut row = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(12.))
      .items_center()
      .justify_between()
      .child(
        div()
          .text_sm()
          .text_color(colors.secondary_foreground)
          .child(label.to_string()),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.foreground)
          .max_w(px(250.))
          .overflow_hidden()
          .text_ellipsis()
          .child(value),
      );

    if !is_first {
      row = row.border_t_1().border_color(colors.border);
    }

    row
  }

  fn render_containers_section(network: &NetworkInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Connected Containers"),
      )
      .child(
        v_flex()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .overflow_hidden()
                    // Header row
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Container"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("IPv4 Address"),
                            ),
                    )
                    // Container rows
                    .children(network.containers.iter().enumerate().map(|(i, (id, container))| {
                        let name = container.name.clone().unwrap_or_else(|| id[..12.min(id.len())].to_string());
                        let ip = container.ipv4_address.clone().unwrap_or_else(|| "-".to_string());

                        let mut row = h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(10.))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .gap(px(8.))
                                    .items_center()
                                    .child(Icon::new(AppIcon::Container).text_color(colors.secondary_foreground))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(colors.foreground)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(name),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .child(ip),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  fn render_labels_section(network: &NetworkInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut labels: Vec<_> = network.labels.iter().collect();
    labels.sort_by(|a, b| a.0.cmp(b.0));

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Labels"),
      )
      .child(
        v_flex()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .overflow_hidden()
                    // Header row
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Key"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Value"),
                            ),
                    )
                    // Label rows
                    .children(labels.iter().enumerate().map(|(i, (key, value))| {
                        let mut row = h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*key).clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*value).clone()),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  fn render_options_section(network: &NetworkInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut options: Vec<_> = network.options.iter().collect();
    options.sort_by(|a, b| a.0.cmp(b.0));

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Options"),
      )
      .child(
        v_flex()
                    .bg(colors.background)
                    .rounded(px(8.))
                    .overflow_hidden()
                    // Header row
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Key"),
                            )
                            .child(
                                div()
                                    .w(px(120.))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("Value"),
                            ),
                    )
                    // Option rows
                    .children(options.iter().enumerate().map(|(i, (key, value))| {
                        let mut row = h_flex()
                            .w_full()
                            .px(px(16.))
                            .py(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*key).clone()),
                            )
                            .child(
                                div()
                                    .w(px(120.))
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child((*value).clone()),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  pub fn render(self, _window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(network) = &self.network else {
      return Self::render_empty(cx).into_any_element();
    };

    let network_id = network.id.clone();
    let is_system = network.is_system_network();

    let on_delete = self.on_delete.clone();
    let on_tab_change = self.on_tab_change.clone();

    let tabs = ["Info"];

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(8.))
      .gap(px(12.))
      .items_center()
      .border_b_1()
      .border_color(colors.border)
      .child(
        TabBar::new("network-tabs")
          .flex_1()
          .children(tabs.iter().enumerate().map(|(i, label)| {
            let on_tab_change = on_tab_change.clone();
            Tab::new()
              .label((*label).to_string())
              .selected(self.active_tab == i)
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_tab_change {
                  cb(&i, window, cx);
                }
              })
          })),
      )
      .when(!is_system, |el| {
        let on_delete = on_delete.clone();
        let id = network_id.clone();
        el.child(
          h_flex().gap(px(8.)).child(
            Button::new("delete")
              .icon(Icon::new(AppIcon::Trash))
              .ghost()
              .small()
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_delete {
                  cb(&id, window, cx);
                }
              }),
          ),
        )
      });

    // Content based on active tab
    let content = Self::render_info_tab(network, cx);

    div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .child(toolbar)
      .child(
        div()
          .id("network-detail-scroll")
          .flex_1()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      )
      .into_any_element()
  }
}
