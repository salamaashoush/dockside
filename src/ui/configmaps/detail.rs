use std::collections::HashMap;

use gpui::{Context, Entity, Render, SharedString, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, IconName, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  menu::{DropdownMenu, PopupMenuItem},
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};

use crate::assets::AppIcon;
use crate::kubernetes::{ConfigMapInfo, EventInfo};
use crate::services;
use crate::state::{ConfigMapDetailTab, DockerState, StateChanged, docker_state};

pub struct ConfigMapDetail {
  docker_state: Entity<DockerState>,
  item: Option<ConfigMapInfo>,
  active_tab: ConfigMapDetailTab,
  yaml_content: String,
  yaml_editor: Option<Entity<InputState>>,
  last_synced_yaml: String,
  /// Original entry contents loaded from cluster.
  entries: Vec<(String, String)>,
  /// Per-key editor states for the Data tab.
  entry_editors: HashMap<String, Entity<InputState>>,
  events_loaded: bool,
}

impl ConfigMapDetail {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |this, ds, event: &StateChanged, cx| match event {
      StateChanged::ConfigMapYamlLoaded { name, namespace, yaml } => {
        if let Some(ref c) = this.item
          && c.name == *name
          && c.namespace == *namespace
        {
          yaml.clone_into(&mut this.yaml_content);
          cx.notify();
        }
      }
      StateChanged::ConfigMapEntriesLoaded {
        name,
        namespace,
        entries,
      } => {
        if let Some(ref c) = this.item
          && c.name == *name
          && c.namespace == *namespace
        {
          this.entries.clone_from(entries);
          this.entry_editors.clear();
          cx.notify();
        }
      }
      StateChanged::ConfigMapTabRequest { name, namespace, tab } => {
        let state = ds.read(cx);
        if let Some(c) = state.get_configmap(name, namespace) {
          this.item = Some(c.clone());
          this.active_tab = *tab;
          this.yaml_content.clear();
          cx.notify();
        }
      }
      StateChanged::ConfigMapsUpdated => {
        if let Some(ref current) = this.item {
          let state = ds.read(cx);
          if let Some(updated) = state.get_configmap(&current.name, &current.namespace) {
            this.item = Some(updated.clone());
          }
          cx.notify();
        }
      }
      StateChanged::EventsUpdated => cx.notify(),
      _ => {}
    })
    .detach();

    Self {
      docker_state,
      item: None,
      active_tab: ConfigMapDetailTab::Info,
      yaml_content: String::new(),
      yaml_editor: None,
      last_synced_yaml: String::new(),
      entries: Vec::new(),
      entry_editors: HashMap::new(),
      events_loaded: false,
    }
  }

  pub fn set_item(&mut self, item: ConfigMapInfo, cx: &mut Context<'_, Self>) {
    self.item = Some(item.clone());
    self.active_tab = ConfigMapDetailTab::Info;
    self.yaml_content.clear();
    self.yaml_editor = None;
    self.last_synced_yaml.clear();
    self.entries.clear();
    self.entry_editors.clear();
    self.events_loaded = false;

    services::get_configmap_yaml(item.name.clone(), item.namespace.clone(), cx);
    services::load_configmap_entries(item.name, item.namespace, cx);
    cx.notify();
  }

  pub fn update_item(&mut self, item: ConfigMapInfo, cx: &mut Context<'_, Self>) {
    self.item = Some(item);
    cx.notify();
  }

  fn collect_entries(&self, cx: &gpui::App) -> Vec<(String, String)> {
    self
      .entries
      .iter()
      .map(|(k, original)| {
        let value = if let Some(editor) = self.entry_editors.get(k) {
          editor.read(cx).text().to_string()
        } else {
          original.clone()
        };
        (k.clone(), value)
      })
      .collect()
  }

  fn render_info_tab(item: &ConfigMapInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let info_row = |label: &str, value: String| {
      h_flex()
        .w_full()
        .py(px(8.))
        .gap(px(16.))
        .child(
          div()
            .w(px(140.))
            .flex_shrink_0()
            .text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.muted_foreground)
            .child(label.to_string()),
        )
        .child(div().flex_1().text_sm().text_color(colors.foreground).child(value))
    };

    let mut content = v_flex()
      .w_full()
      .gap(px(4.))
      .child(info_row("Name", item.name.clone()))
      .child(info_row("Namespace", item.namespace.clone()))
      .child(info_row("Keys", item.keys.len().to_string()))
      .child(info_row("Age", item.age.clone()));

    if !item.keys.is_empty() {
      content = content.child(
        v_flex()
          .w_full()
          .mt(px(16.))
          .gap(px(8.))
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::SEMIBOLD)
              .text_color(colors.foreground)
              .child("Key Names"),
          )
          .child(
            h_flex().gap(px(6.)).flex_wrap().children(
              item
                .keys
                .iter()
                .map(|k| {
                  div()
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(colors.sidebar)
                    .text_xs()
                    .font_family("monospace")
                    .text_color(colors.foreground)
                    .child(k.clone())
                })
                .collect::<Vec<_>>(),
            ),
          ),
      );
    }

    if !item.labels.is_empty() {
      content = content.child(
        v_flex()
          .w_full()
          .mt(px(16.))
          .gap(px(8.))
          .child(
            div()
              .text_sm()
              .font_weight(gpui::FontWeight::SEMIBOLD)
              .text_color(colors.foreground)
              .child("Labels"),
          )
          .child(
            div().w_full().p(px(12.)).rounded(px(8.)).bg(colors.sidebar).child(
              v_flex().gap(px(4.)).children(
                item
                  .labels
                  .iter()
                  .map(|(k, v)| {
                    div()
                      .text_xs()
                      .font_family("monospace")
                      .text_color(colors.muted_foreground)
                      .child(format!("{k}={v}"))
                  })
                  .collect::<Vec<_>>(),
              ),
            ),
          ),
      );
    }

    div()
      .size_full()
      .child(div().w_full().h_full().p(px(16.)).overflow_y_scrollbar().child(content))
  }

  fn ensure_entry_editors(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
    let mut new_editors = HashMap::new();
    for (k, v) in &self.entries {
      if let Some(existing) = self.entry_editors.remove(k) {
        new_editors.insert(k.clone(), existing);
      } else {
        let value = v.clone();
        let editor = cx.new(|cx| {
          let mut state = InputState::new(window, cx).multi_line(true).soft_wrap(true);
          state.set_value(value, window, cx);
          state
        });
        new_editors.insert(k.clone(), editor);
      }
    }
    self.entry_editors = new_editors;
  }

  fn render_data_tab(&self, item: &ConfigMapInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    if self.entries.is_empty() {
      return div().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading entries…"),
      );
    }

    let name = item.name.clone();
    let namespace = item.namespace.clone();

    let toolbar = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(6.))
      .gap(px(6.))
      .items_center()
      .justify_between()
      .border_b_1()
      .border_color(colors.border)
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Edit values inline. Save Changes patches data via JSON merge."),
      )
      .child(
        h_flex()
          .gap(px(6.))
          .child(
            Button::new("cm-data-save")
              .label("Save Changes")
              .primary()
              .compact()
              .on_click(cx.listener(move |this, _ev, _w, cx| {
                let entries = this.collect_entries(cx);
                services::apply_configmap_data(name.clone(), namespace.clone(), entries, cx);
              })),
          )
          .child(
            Button::new("cm-data-revert")
              .label("Revert")
              .ghost()
              .compact()
              .on_click(cx.listener(|this, _ev, _w, cx| {
                this.entry_editors.clear();
                cx.notify();
              })),
          ),
      );

    let mut col = v_flex().w_full().gap(px(2.)).p(px(12.));
    for (k, _v) in &self.entries {
      let editor_opt = self.entry_editors.get(k).cloned();
      col = col.child(
        v_flex()
          .w_full()
          .py(px(8.))
          .gap(px(6.))
          .border_b_1()
          .border_color(colors.border)
          .child(
            h_flex()
              .w_full()
              .items_center()
              .justify_between()
              .child(
                div()
                  .text_sm()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .font_family("monospace")
                  .text_color(colors.foreground)
                  .child(k.clone()),
              )
              .child(
                Button::new(SharedString::from(format!("copy-{k}")))
                  .icon(Icon::new(AppIcon::Copy))
                  .ghost()
                  .xsmall()
                  .on_click({
                    let editor = editor_opt.clone();
                    move |_, _, cx| {
                      if let Some(ref e) = editor {
                        let v = e.read(cx).text().to_string();
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(v));
                      }
                    }
                  }),
              ),
          )
          .child(div().w_full().when_some(editor_opt, |el, editor| {
            el.child(Input::new(&editor).w_full().appearance(true))
          })),
      );
    }
    v_flex()
      .size_full()
      .child(toolbar)
      .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(col))
  }

  fn render_yaml_tab(&self, item: &ConfigMapInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    if self.yaml_content.is_empty() {
      return v_flex().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading YAML..."),
      );
    }

    let name = item.name.clone();
    let namespace = item.namespace.clone();
    let editor_for_apply = self.yaml_editor.clone();

    let toolbar = h_flex()
      .w_full()
      .px(px(12.))
      .py(px(6.))
      .gap(px(6.))
      .items_center()
      .justify_between()
      .border_b_1()
      .border_color(colors.border)
      .child(
        div()
          .text_xs()
          .text_color(colors.muted_foreground)
          .child("Edit YAML and Apply, or use the menu to reload."),
      )
      .child(
        Button::new("yaml-actions")
          .icon(IconName::Ellipsis)
          .ghost()
          .compact()
          .dropdown_menu({
            let name = name.clone();
            let namespace = namespace.clone();
            let editor = editor_for_apply.clone();
            move |menu, _w, _cx| {
              let apply_name = name.clone();
              let apply_namespace = namespace.clone();
              let apply_editor = editor.clone();
              let reload_name = name.clone();
              let reload_namespace = namespace.clone();
              menu
                .item(
                  PopupMenuItem::new("Apply YAML")
                    .icon(Icon::new(AppIcon::Refresh))
                    .on_click(move |_, _, cx| {
                      let Some(ref e) = apply_editor else { return };
                      let yaml: String = e.read(cx).text().to_string();
                      if !yaml.trim().is_empty() {
                        services::apply_configmap_yaml(apply_name.clone(), apply_namespace.clone(), yaml, cx);
                      }
                    }),
                )
                .separator()
                .item(PopupMenuItem::new("Reload from Cluster").on_click(move |_, _, cx| {
                  services::get_configmap_yaml(reload_name.clone(), reload_namespace.clone(), cx);
                }))
            }
          }),
      );

    if let Some(ref editor) = self.yaml_editor {
      return v_flex().size_full().child(toolbar).child(
        div()
          .flex_1()
          .min_h_0()
          .child(Input::new(editor).size_full().appearance(false)),
      );
    }

    v_flex().size_full().child(toolbar).child(
      div().flex_1().min_h_0().child(
        div()
          .size_full()
          .overflow_y_scrollbar()
          .bg(colors.sidebar)
          .p(px(12.))
          .font_family("monospace")
          .text_xs()
          .text_color(colors.foreground)
          .child(self.yaml_content.clone()),
      ),
    )
  }

  fn render_events_tab(&self, item: &ConfigMapInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.docker_state.read(cx);
    let events: Vec<&EventInfo> = state
      .events
      .iter()
      .filter(|e| {
        e.namespace == item.namespace && e.object_kind.eq_ignore_ascii_case("ConfigMap") && e.object_name == item.name
      })
      .collect();

    if events.is_empty() {
      return div().size_full().flex().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No events for this ConfigMap in selected namespace."),
      );
    }

    let mut rows = v_flex().w_full().gap(px(2.)).p(px(12.));
    for ev in events {
      let type_color = if ev.event_type == "Warning" {
        colors.danger
      } else {
        colors.muted_foreground
      };
      rows = rows.child(
        v_flex()
          .w_full()
          .py(px(8.))
          .gap(px(2.))
          .border_b_1()
          .border_color(colors.border)
          .child(
            h_flex()
              .gap(px(8.))
              .items_center()
              .child(
                div()
                  .text_xs()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_color(type_color)
                  .child(ev.event_type.clone()),
              )
              .child(
                div()
                  .text_xs()
                  .font_weight(gpui::FontWeight::MEDIUM)
                  .text_color(colors.foreground)
                  .child(ev.reason.clone()),
              )
              .child(
                div()
                  .flex_1()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .text_ellipsis()
                  .overflow_hidden()
                  .child(format!("×{}", ev.count)),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(colors.muted_foreground)
                  .child(ev.age.clone()),
              ),
          )
          .child(
            div()
              .text_xs()
              .text_color(colors.foreground)
              .whitespace_normal()
              .child(ev.message.clone()),
          ),
      );
    }
    div()
      .size_full()
      .child(div().size_full().overflow_y_scrollbar().child(rows))
  }

  fn render_empty(cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    div().size_full().flex().items_center().justify_center().child(
      v_flex()
        .items_center()
        .gap(px(16.))
        .child(
          div()
            .size(px(64.))
            .rounded(px(12.))
            .bg(colors.sidebar)
            .flex()
            .items_center()
            .justify_center()
            .child(
              Icon::new(AppIcon::Settings)
                .size(px(48.))
                .text_color(colors.muted_foreground),
            ),
        )
        .child(
          div()
            .text_lg()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.secondary_foreground)
            .child("Select a ConfigMap"),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Click on a configmap to view details"),
        ),
    )
  }
}

impl Render for ConfigMapDetail {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    if self.yaml_editor.is_none() && self.item.is_some() {
      self.yaml_editor = Some(cx.new(|cx| {
        InputState::new(window, cx)
          .multi_line(true)
          .code_editor("yaml")
          .line_number(true)
          .searchable(true)
          .soft_wrap(false)
      }));
    }

    if let Some(ref editor) = self.yaml_editor
      && !self.yaml_content.is_empty()
      && self.last_synced_yaml != self.yaml_content
    {
      let yaml_clone = self.yaml_content.clone();
      editor.update(cx, |state, cx| {
        state.set_value(yaml_clone.clone(), window, cx);
      });
      self.last_synced_yaml = self.yaml_content.clone();
    }

    if !self.entries.is_empty() {
      self.ensure_entry_editors(window, cx);
    }

    let Some(item) = self.item.clone() else {
      return div().size_full().child(Self::render_empty(cx));
    };

    let active_tab = self.active_tab;

    if active_tab == ConfigMapDetailTab::Events && !self.events_loaded {
      services::refresh_events(cx);
      self.events_loaded = true;
    }

    let tab_bar = TabBar::new("cm-tabs")
      .flex_1()
      .py(px(0.))
      .child(
        Tab::new()
          .label("Info")
          .selected(active_tab == ConfigMapDetailTab::Info)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ConfigMapDetailTab::Info;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Data")
          .selected(active_tab == ConfigMapDetailTab::Data)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ConfigMapDetailTab::Data;
            if this.entries.is_empty()
              && let Some(ref item) = this.item
            {
              services::load_configmap_entries(item.name.clone(), item.namespace.clone(), cx);
            }
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("YAML")
          .selected(active_tab == ConfigMapDetailTab::Yaml)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ConfigMapDetailTab::Yaml;
            if let Some(ref item) = this.item {
              services::get_configmap_yaml(item.name.clone(), item.namespace.clone(), cx);
            }
          })),
      )
      .child(
        Tab::new()
          .label("Events")
          .selected(active_tab == ConfigMapDetailTab::Events)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = ConfigMapDetailTab::Events;
            services::refresh_events(cx);
            this.events_loaded = true;
            cx.notify();
          })),
      );

    let content = match active_tab {
      ConfigMapDetailTab::Info => Self::render_info_tab(&item, cx),
      ConfigMapDetailTab::Data => self.render_data_tab(&item, cx),
      ConfigMapDetailTab::Yaml => self.render_yaml_tab(&item, cx),
      ConfigMapDetailTab::Events => self.render_events_tab(&item, cx),
    };

    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      .child(div().w_full().flex_shrink_0().child(tab_bar))
      .child(div().flex_1().min_h_0().overflow_hidden().child(content))
  }
}
