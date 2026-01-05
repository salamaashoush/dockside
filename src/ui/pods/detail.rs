use gpui::{App, Entity, Styled, Window, div, prelude::*, px};
use gpui_component::{
  Icon, Selectable, Sizable,
  button::{Button, ButtonVariants},
  h_flex,
  input::{Input, InputState},
  scroll::ScrollableElement,
  tab::{Tab, TabBar},
  theme::ActiveTheme,
  v_flex,
};
use std::rc::Rc;

use crate::assets::AppIcon;
use crate::kubernetes::{PodInfo, PodPhase};
use crate::terminal::TerminalView;

// Re-export from state module for backwards compatibility
pub use crate::state::PodDetailTab;

type PodActionCallback = Rc<dyn Fn(&(String, String), &mut Window, &mut App) + 'static>;
type TabChangeCallback = Rc<dyn Fn(&PodDetailTab, &mut Window, &mut App) + 'static>;
type RefreshCallback = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;
type ContainerSelectCallback = Rc<dyn Fn(&String, &mut Window, &mut App) + 'static>;

/// State for pod detail tabs
#[derive(Debug, Clone, Default)]
pub struct PodTabState {
  pub logs: String,
  pub logs_loading: bool,
  pub describe: String,
  pub describe_loading: bool,
  pub yaml: String,
  pub yaml_loading: bool,
  pub selected_container: Option<String>,
}

impl PodTabState {
  pub fn new() -> Self {
    Self::default()
  }
}

pub struct PodDetail {
  pod: Option<PodInfo>,
  active_tab: PodDetailTab,
  pod_state: Option<PodTabState>,
  terminal_view: Option<Entity<TerminalView>>,
  logs_editor: Option<Entity<InputState>>,
  describe_editor: Option<Entity<InputState>>,
  yaml_editor: Option<Entity<InputState>>,
  on_delete: Option<PodActionCallback>,
  on_tab_change: Option<TabChangeCallback>,
  on_refresh_logs: Option<RefreshCallback>,
  on_container_select: Option<ContainerSelectCallback>,
}

impl PodDetail {
  pub fn new() -> Self {
    Self {
      pod: None,
      active_tab: PodDetailTab::Info,
      pod_state: None,
      terminal_view: None,
      logs_editor: None,
      describe_editor: None,
      yaml_editor: None,
      on_delete: None,
      on_tab_change: None,
      on_refresh_logs: None,
      on_container_select: None,
    }
  }

  pub fn pod(mut self, pod: Option<PodInfo>) -> Self {
    self.pod = pod;
    self
  }

  pub fn active_tab(mut self, tab: PodDetailTab) -> Self {
    self.active_tab = tab;
    self
  }

  pub fn pod_state(mut self, state: PodTabState) -> Self {
    self.pod_state = Some(state);
    self
  }

  pub fn terminal_view(mut self, view: Option<Entity<TerminalView>>) -> Self {
    self.terminal_view = view;
    self
  }

  pub fn logs_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.logs_editor = editor;
    self
  }

  pub fn describe_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.describe_editor = editor;
    self
  }

  pub fn yaml_editor(mut self, editor: Option<Entity<InputState>>) -> Self {
    self.yaml_editor = editor;
    self
  }

  pub fn on_delete<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(String, String), &mut Window, &mut App) + 'static,
  {
    self.on_delete = Some(Rc::new(callback));
    self
  }

  pub fn on_tab_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&PodDetailTab, &mut Window, &mut App) + 'static,
  {
    self.on_tab_change = Some(Rc::new(callback));
    self
  }

  pub fn on_refresh_logs<F>(mut self, callback: F) -> Self
  where
    F: Fn(&(), &mut Window, &mut App) + 'static,
  {
    self.on_refresh_logs = Some(Rc::new(callback));
    self
  }

  pub fn on_container_select<F>(mut self, callback: F) -> Self
  where
    F: Fn(&String, &mut Window, &mut App) + 'static,
  {
    self.on_container_select = Some(Rc::new(callback));
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
            Icon::new(AppIcon::Container)
              .size(px(48.))
              .text_color(colors.muted_foreground),
          )
          .child(
            div()
              .text_color(colors.muted_foreground)
              .child("Select a pod to view details"),
          ),
      )
  }

  fn render_info_tab(pod: &PodInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;

    let info_row = |label: &str, value: String| {
      h_flex()
        .w_full()
        .py(px(12.))
        .justify_between()
        .border_b_1()
        .border_color(colors.border)
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child(label.to_string()),
        )
        .child(div().text_sm().text_color(colors.foreground).child(value))
    };

    let status_text = pod.phase.to_string();
    let status_color = match pod.phase {
      PodPhase::Running => colors.success,
      PodPhase::Pending => colors.warning,
      PodPhase::Failed => colors.danger,
      _ => colors.muted_foreground,
    };

    v_flex()
            .w_full()
            .p(px(16.))
            .gap(px(8.))
            .child(info_row("Name", pod.name.clone()))
            .child(info_row("Namespace", pod.namespace.clone()))
            .child(
                h_flex()
                    .w_full()
                    .py(px(12.))
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child("Status"),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .w(px(8.))
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(status_color),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .child(status_text),
                            ),
                    ),
            )
            .child(info_row("Ready", pod.ready.clone()))
            .child(info_row("Restarts", pod.restarts.to_string()))
            .child(info_row("Age", pod.age.clone()))
            .child(info_row(
                "Node",
                pod.node.clone().unwrap_or_else(|| "N/A".to_string()),
            ))
            .child(info_row(
                "IP",
                pod.ip.clone().unwrap_or_else(|| "N/A".to_string()),
            ))
            // Containers section
            .child(
                div()
                    .w_full()
                    .pt(px(16.))
                    .pb(px(8.))
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.foreground)
                    .child("Containers"),
            )
            .children(pod.containers.iter().map(|container| {
                let status_color = if container.ready {
                    colors.success
                } else {
                    colors.warning
                };

                h_flex()
                    .w_full()
                    .py(px(8.))
                    .px(px(12.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colors.border)
                    .mb(px(8.))
                    .gap(px(12.))
                    .items_center()
                    .child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .rounded_full()
                            .bg(status_color),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child(container.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child(container.image.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!("Restarts: {}", container.restart_count)),
                    )
            }))
  }

  fn render_logs_tab(&self, _pod: &PodInfo, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.pod_state.as_ref();
    let is_loading = state.is_some_and(|s| s.logs_loading);

    if is_loading {
      return v_flex().size_full().p(px(16.)).child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Loading logs..."),
      );
    }

    if let Some(ref editor) = self.logs_editor {
      return div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true));
    }

    // Fallback to plain text
    let logs_content = state.map_or_else(|| "No logs available".to_string(), |s| s.logs.clone());
    div().size_full().child(
      div()
        .size_full()
        .overflow_y_scrollbar()
        .bg(colors.sidebar)
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(logs_content),
    )
  }

  fn render_terminal_tab(&self, pod: &PodInfo, cx: &App) -> gpui::Div {
    // Check if pod is running
    if !matches!(pod.phase, PodPhase::Running) {
      let colors = &cx.theme().colors;
      return v_flex()
        .flex_1()
        .w_full()
        .p(px(16.))
        .items_center()
        .justify_center()
        .gap(px(16.))
        .child(
          Icon::new(AppIcon::Terminal)
            .size(px(48.))
            .text_color(colors.muted_foreground),
        )
        .child(
          div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child("Pod must be running to open terminal"),
        );
    }

    // If we have a terminal view, render it full size
    if let Some(terminal) = &self.terminal_view {
      return div().size_full().flex_1().min_h_0().p(px(8.)).child(terminal.clone());
    }

    let colors = &cx.theme().colors;

    // Fallback: show message
    v_flex()
      .flex_1()
      .w_full()
      .p(px(16.))
      .items_center()
      .justify_center()
      .gap(px(16.))
      .child(
        Icon::new(AppIcon::Terminal)
          .size(px(48.))
          .text_color(colors.muted_foreground),
      )
      .child(
        div()
          .text_sm()
          .text_color(colors.muted_foreground)
          .child("Click Terminal tab to connect to pod"),
      )
  }

  fn render_describe_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.pod_state.as_ref();
    let is_loading = state.is_some_and(|s| s.describe_loading);

    if is_loading {
      return v_flex()
        .size_full()
        .p(px(16.))
        .child(div().text_sm().text_color(colors.muted_foreground).child("Loading..."));
    }

    if let Some(ref editor) = self.describe_editor {
      return div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true));
    }

    // Fallback to plain text
    let describe_content = state.map_or_else(|| "Loading...".to_string(), |s| s.describe.clone());
    div().size_full().child(
      div()
        .size_full()
        .overflow_y_scrollbar()
        .bg(colors.sidebar)
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(describe_content),
    )
  }

  fn render_yaml_tab(&self, cx: &App) -> gpui::Div {
    let colors = &cx.theme().colors;
    let state = self.pod_state.as_ref();
    let is_loading = state.is_some_and(|s| s.yaml_loading);

    if is_loading {
      return v_flex()
        .size_full()
        .p(px(16.))
        .child(div().text_sm().text_color(colors.muted_foreground).child("Loading..."));
    }

    if let Some(ref editor) = self.yaml_editor {
      return div()
        .size_full()
        .child(Input::new(editor).size_full().appearance(false).disabled(true));
    }

    // Fallback to plain text
    let yaml_content = state.map_or_else(|| "Loading...".to_string(), |s| s.yaml.clone());
    div().size_full().child(
      div()
        .size_full()
        .overflow_y_scrollbar()
        .bg(colors.sidebar)
        .p(px(12.))
        .font_family("monospace")
        .text_xs()
        .text_color(colors.foreground)
        .child(yaml_content),
    )
  }

  pub fn render(&self, _window: &mut Window, cx: &App) -> gpui::AnyElement {
    let colors = &cx.theme().colors;

    let Some(pod) = &self.pod else {
      return Self::render_empty(cx).into_any_element();
    };

    let pod_name = pod.name.clone();
    let pod_namespace = pod.namespace.clone();

    let on_delete = self.on_delete.clone();
    let on_tab_change = self.on_tab_change.clone();

    // Toolbar with tabs and actions
    let toolbar = h_flex()
      .w_full()
      .px(px(16.))
      .py(px(8.))
      .gap(px(12.))
      .items_center()
      .flex_shrink_0()
      .border_b_1()
      .border_color(colors.border)
      .child(
        TabBar::new("pod-tabs")
          .flex_1()
          .children(PodDetailTab::ALL.iter().map(|tab| {
            let on_tab_change = on_tab_change.clone();
            let tab_variant = *tab;
            Tab::new()
              .label(tab.label().to_string())
              .selected(self.active_tab == *tab)
              .on_click(move |_ev, window, cx| {
                if let Some(ref cb) = on_tab_change {
                  cb(&tab_variant, window, cx);
                }
              })
          })),
      )
      .child(h_flex().gap(px(8.)).child({
        let on_delete = on_delete.clone();
        let name = pod_name.clone();
        let ns = pod_namespace.clone();
        Button::new("delete")
          .icon(Icon::new(AppIcon::Trash))
          .ghost()
          .small()
          .on_click(move |_ev, window, cx| {
            if let Some(ref cb) = on_delete {
              cb(&(name.clone(), ns.clone()), window, cx);
            }
          })
      }));

    // Content based on active tab
    let content = match self.active_tab {
      PodDetailTab::Logs => self.render_logs_tab(pod, cx),
      PodDetailTab::Terminal => self.render_terminal_tab(pod, cx),
      PodDetailTab::Describe => self.render_describe_tab(cx),
      PodDetailTab::Yaml => self.render_yaml_tab(cx),
      PodDetailTab::Info => Self::render_info_tab(pod, cx),
    };

    // Terminal tab needs full height without scroll
    let is_terminal_tab = self.active_tab == PodDetailTab::Terminal;

    let mut result = div()
      .size_full()
      .overflow_hidden()
      .bg(colors.sidebar)
      .flex()
      .flex_col()
      .child(toolbar);

    if is_terminal_tab {
      result = result.child(div().flex_1().min_h_0().w_full().child(content));
    } else {
      result = result.child(
        div()
          .id("pod-detail-scroll")
          .flex_1()
          .min_h_0()
          .w_full()
          .overflow_hidden()
          .overflow_y_scrollbar()
          .child(content)
          .child(div().h(px(100.))),
      );
    }

    result.into_any_element()
  }
}
