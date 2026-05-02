use std::collections::HashSet;

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
use crate::kubernetes::{EventInfo, SecretInfo};
use crate::services;
use crate::state::{DockerState, SecretDetailTab, StateChanged, docker_state};

pub struct SecretDetail {
  docker_state: Entity<DockerState>,
  item: Option<SecretInfo>,
  active_tab: SecretDetailTab,
  yaml_content: String,
  yaml_editor: Option<Entity<InputState>>,
  last_synced_yaml: String,
  entries: Vec<(String, String)>,
  revealed: HashSet<String>,
  events_loaded: bool,
}

impl SecretDetail {
  pub fn new(cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    cx.subscribe(&docker_state, |this, ds, event: &StateChanged, cx| match event {
      StateChanged::SecretYamlLoaded { name, namespace, yaml } => {
        if let Some(ref s) = this.item
          && s.name == *name
          && s.namespace == *namespace
        {
          yaml.clone_into(&mut this.yaml_content);
          cx.notify();
        }
      }
      StateChanged::SecretEntriesLoaded {
        name,
        namespace,
        entries,
      } => {
        if let Some(ref s) = this.item
          && s.name == *name
          && s.namespace == *namespace
        {
          this.entries.clone_from(entries);
          cx.notify();
        }
      }
      StateChanged::SecretTabRequest { name, namespace, tab } => {
        let state = ds.read(cx);
        if let Some(s) = state.get_secret(name, namespace) {
          this.item = Some(s.clone());
          this.active_tab = *tab;
          this.yaml_content.clear();
          cx.notify();
        }
      }
      StateChanged::SecretsUpdated => {
        if let Some(ref current) = this.item {
          let state = ds.read(cx);
          if let Some(updated) = state.get_secret(&current.name, &current.namespace) {
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
      active_tab: SecretDetailTab::Info,
      yaml_content: String::new(),
      yaml_editor: None,
      last_synced_yaml: String::new(),
      entries: Vec::new(),
      revealed: HashSet::new(),
      events_loaded: false,
    }
  }

  pub fn set_item(&mut self, item: SecretInfo, cx: &mut Context<'_, Self>) {
    self.item = Some(item.clone());
    self.active_tab = SecretDetailTab::Info;
    self.yaml_content.clear();
    self.yaml_editor = None;
    self.last_synced_yaml.clear();
    self.entries.clear();
    self.revealed.clear();
    self.events_loaded = false;

    services::get_secret_yaml(item.name.clone(), item.namespace.clone(), cx);
    services::load_secret_entries(item.name, item.namespace, cx);
    cx.notify();
  }

  pub fn update_item(&mut self, item: SecretInfo, cx: &mut Context<'_, Self>) {
    self.item = Some(item);
    cx.notify();
  }

  fn toggle_reveal(&mut self, key: &str, cx: &mut Context<'_, Self>) {
    if !self.revealed.remove(key) {
      self.revealed.insert(key.to_string());
    }
    cx.notify();
  }

  fn render_info_tab(item: &SecretInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
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
      .child(info_row("Type", item.secret_type.clone()))
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

  fn render_data_tab(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    if self.entries.is_empty() {
      return div().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading entries…"),
      );
    }

    let mut col = v_flex().w_full().gap(px(2.)).p(px(12.));
    for (k, v) in &self.entries {
      let revealed = self.revealed.contains(k);
      let key_for_btn = k.clone();
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
                h_flex()
                  .gap(px(4.))
                  .child(
                    Button::new(SharedString::from(format!("reveal-{key_for_btn}")))
                      .label(if revealed { "Hide" } else { "Reveal" })
                      .ghost()
                      .xsmall()
                      .on_click(cx.listener(move |this, _, _, cx| this.toggle_reveal(&key_for_btn, cx))),
                  )
                  .child(
                    Button::new(SharedString::from(format!("copy-{k}")))
                      .icon(Icon::new(AppIcon::Copy))
                      .ghost()
                      .xsmall()
                      .on_click({
                        let v = v.clone();
                        move |_, _, cx| {
                          cx.write_to_clipboard(gpui::ClipboardItem::new_string(v.clone()));
                        }
                      }),
                  ),
              ),
          )
          .child(
            div()
              .w_full()
              .p(px(8.))
              .rounded(px(6.))
              .bg(colors.sidebar)
              .text_xs()
              .font_family("monospace")
              .text_color(colors.foreground)
              .whitespace_normal()
              .child(if revealed {
                v.clone()
              } else {
                "•".repeat(v.len().min(64))
              }),
          ),
      );
    }
    div()
      .size_full()
      .child(div().size_full().overflow_y_scrollbar().child(col))
  }

  fn render_yaml_tab(&self, item: &SecretInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
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
                        services::apply_secret_yaml(apply_name.clone(), apply_namespace.clone(), yaml, cx);
                      }
                    }),
                )
                .separator()
                .item(PopupMenuItem::new("Reload from Cluster").on_click(move |_, _, cx| {
                  services::get_secret_yaml(reload_name.clone(), reload_namespace.clone(), cx);
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

  fn render_events_tab(&self, item: &SecretInfo, cx: &mut Context<'_, Self>) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.docker_state.read(cx);
    let events: Vec<&EventInfo> = state
      .events
      .iter()
      .filter(|e| {
        e.namespace == item.namespace && e.object_kind.eq_ignore_ascii_case("Secret") && e.object_name == item.name
      })
      .collect();

    if events.is_empty() {
      return div().size_full().flex().items_center().justify_center().child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("No events for this Secret in selected namespace."),
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
            .child("Select a Secret"),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Click on a secret to view details"),
        ),
    )
  }
}

impl Render for SecretDetail {
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

    let Some(item) = self.item.clone() else {
      return div().size_full().child(Self::render_empty(cx));
    };

    let active_tab = self.active_tab;

    if active_tab == SecretDetailTab::Events && !self.events_loaded {
      services::refresh_events(cx);
      self.events_loaded = true;
    }

    let tab_bar = TabBar::new("secret-tabs")
      .flex_1()
      .py(px(0.))
      .child(
        Tab::new()
          .label("Info")
          .selected(active_tab == SecretDetailTab::Info)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = SecretDetailTab::Info;
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("Data")
          .selected(active_tab == SecretDetailTab::Data)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = SecretDetailTab::Data;
            if this.entries.is_empty()
              && let Some(ref item) = this.item
            {
              services::load_secret_entries(item.name.clone(), item.namespace.clone(), cx);
            }
            cx.notify();
          })),
      )
      .child(
        Tab::new()
          .label("YAML")
          .selected(active_tab == SecretDetailTab::Yaml)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = SecretDetailTab::Yaml;
            if let Some(ref item) = this.item {
              services::get_secret_yaml(item.name.clone(), item.namespace.clone(), cx);
            }
          })),
      )
      .child(
        Tab::new()
          .label("Events")
          .selected(active_tab == SecretDetailTab::Events)
          .on_click(cx.listener(|this, _ev, _w, cx| {
            this.active_tab = SecretDetailTab::Events;
            services::refresh_events(cx);
            this.events_loaded = true;
            cx.notify();
          })),
      );

    let content = match active_tab {
      SecretDetailTab::Info => Self::render_info_tab(&item, cx),
      SecretDetailTab::Data => self.render_data_tab(cx),
      SecretDetailTab::Yaml => self.render_yaml_tab(&item, cx),
      SecretDetailTab::Events => self.render_events_tab(&item, cx),
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
