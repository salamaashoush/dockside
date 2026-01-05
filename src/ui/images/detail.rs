use gpui::{App, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::docker::ImageInfo;
use crate::state::ImageInspectData;

type ImageActionCallback = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

pub struct ImageDetail {
  image: Option<ImageInfo>,
  inspect_data: Option<ImageInspectData>,
  active_tab: usize,
  on_delete: Option<ImageActionCallback>,
  on_tab_change: Option<TabChangeCallback>,
}

impl ImageDetail {
  pub fn new() -> Self {
    Self {
      image: None,
      inspect_data: None,
      active_tab: 0,
      on_delete: None,
      on_tab_change: None,
    }
  }

  pub fn image(mut self, image: Option<ImageInfo>) -> Self {
    self.image = image;
    self
  }

  pub fn inspect_data(mut self, data: Option<ImageInspectData>) -> Self {
    self.inspect_data = data;
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
            Icon::new(AppIcon::Image)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select an image to view details"),
          ),
      )
  }

  fn render_info_tab(&self, image: &ImageInfo, cx: &App) -> gpui::Div {
    let _colors = &cx.theme().colors;

    // Basic info rows
    let mut basic_info = vec![
      ("ID", image.short_id().to_string()),
      ("Tag", image.display_name()),
      ("Size", image.display_size()),
    ];

    if let Some(created) = image.created {
      basic_info.insert(2, ("Created", created.format("%Y-%m-%d %H:%M:%S").to_string()));
    }

    // Platform
    if let (Some(os), Some(arch)) = (&image.os, &image.architecture) {
      basic_info.push(("Platform", format!("{os}/{arch}")));
    }

    let mut content = v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .gap(px(12.))
      .child(Self::render_section(None, basic_info, cx));

    // Config section if we have inspect data
    if let Some(ref data) = self.inspect_data {
      let mut config_rows = Vec::new();

      if let Some(ref cmd) = data.config_cmd {
        config_rows.push(("Command", cmd.join(" ")));
      }

      if let Some(ref workdir) = data.config_workdir {
        config_rows.push(("Working Directory", workdir.clone()));
      }

      if let Some(ref entrypoint) = data.config_entrypoint {
        config_rows.push(("Entrypoint", entrypoint.join(" ")));
      }

      if !config_rows.is_empty() {
        content = content.child(Self::render_section(Some("Config"), config_rows, cx));
      }

      // Environment section
      if !data.config_env.is_empty() {
        content = content.child(Self::render_env_section(&data.config_env, cx));
      }

      // Exposed ports
      if !data.config_exposed_ports.is_empty() {
        let ports_str = data.config_exposed_ports.join(", ");
        content = content.child(Self::render_section(
          Some("Exposed Ports"),
          vec![("Ports", ports_str)],
          cx,
        ));
      }

      // Used by section
      if !data.used_by.is_empty() {
        content = content.child(Self::render_used_by_section(&data.used_by, cx));
      }
    }

    // Labels section if not empty
    if !image.labels.is_empty() {
      content = content.child(Self::render_labels_section(image, cx));
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

  fn render_env_section(env: &[(String, String)], cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Environment"),
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
                    // Env rows
                    .children(env.iter().enumerate().map(|(i, (key, value))| {
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
                                    .child(key.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(colors.secondary_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(value.clone()),
                            );

                        if i > 0 {
                            row = row.border_t_1().border_color(colors.border);
                        }
                        row
                    })),
      )
  }

  fn render_used_by_section(containers: &[String], cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    v_flex()
      .gap(px(1.))
      .child(
        div()
          .py(px(8.))
          .text_sm()
          .font_weight(gpui::FontWeight::MEDIUM)
          .text_color(colors.foreground)
          .child("Used By"),
      )
      .child(
        v_flex()
          .bg(colors.background)
          .rounded(px(8.))
          .overflow_hidden()
          .children(containers.iter().enumerate().map(|(i, name)| {
            let mut row = h_flex()
              .w_full()
              .px(px(16.))
              .py(px(10.))
              .items_center()
              .gap(px(8.))
              .child(Icon::new(AppIcon::Image).text_color(colors.secondary_foreground))
              .child(div().text_sm().text_color(colors.foreground).child(name.clone()));

            if i > 0 {
              row = row.border_t_1().border_color(colors.border);
            }
            row
          })),
      )
  }

  fn render_labels_section(image: &ImageInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let mut labels: Vec<_> = image.labels.iter().collect();
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

  pub fn render(self, _window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(image) = &self.image else {
      return Self::render_empty(cx).into_any_element();
    };

    let image_id = image.id.clone();
    let image_id_for_delete = image_id.clone();

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
        TabBar::new("image-tabs")
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
      .child(h_flex().gap(px(8.)).child({
        let on_delete = on_delete.clone();
        let id = image_id_for_delete.clone();
        Button::new("delete")
          .icon(Icon::new(AppIcon::Trash))
          .ghost()
          .small()
          .on_click(move |_ev, window, cx| {
            if let Some(ref cb) = on_delete {
              cb(&id, window, cx);
            }
          })
      }));

    // Content based on active tab
    let content = self.render_info_tab(image, cx);

    div()
      .size_full()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .child(toolbar)
      .child(
        div()
          .id("image-detail-scroll")
          .flex_1()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      )
      .into_any_element()
  }
}
