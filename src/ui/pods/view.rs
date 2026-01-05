use gpui::{App, Context, Entity, Render, Styled, Timer, Window, div, prelude::*, px};
use gpui_component::{input::InputState, theme::ActiveTheme};
use std::time::Duration;

use crate::kubernetes::{PodInfo, PodPhase};
use crate::services;
use crate::state::{DockerState, Selection, StateChanged, docker_state, settings_state};
use crate::terminal::{TerminalSessionType, TerminalView};

use super::detail::{PodDetail, PodDetailTab, PodTabState};
use super::list::{PodList, PodListEvent};

/// Self-contained Pods view - handles list, detail, and all state
pub struct PodsView {
  docker_state: Entity<DockerState>,
  pod_list: Entity<PodList>,
  active_tab: PodDetailTab,
  pod_tab_state: PodTabState,
  terminal_view: Option<Entity<TerminalView>>,
  // Editors for logs/describe/yaml
  logs_editor: Option<Entity<InputState>>,
  describe_editor: Option<Entity<InputState>>,
  yaml_editor: Option<Entity<InputState>>,
  // Track synced content
  last_synced_logs: String,
  last_synced_describe: String,
  last_synced_yaml: String,
}

impl PodsView {
  /// Get the currently selected pod from global state
  fn selected_pod(&self, cx: &App) -> Option<PodInfo> {
    let state = self.docker_state.read(cx);
    if let Selection::Pod {
      ref name,
      ref namespace,
    } = state.selection
    {
      state.get_pod(name, namespace).cloned()
    } else {
      None
    }
  }

  pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
    let docker_state = docker_state(cx);

    // Create pod list entity
    let pod_list = cx.new(|cx| PodList::new(window, cx));

    // Subscribe to pod list events
    cx.subscribe_in(&pod_list, window, |this, _list, event: &PodListEvent, window, cx| {
      let PodListEvent::Selected(pod) = event;
      this.on_select_pod(pod, window, cx);
    })
    .detach();

    // Subscribe to state changes
    cx.subscribe_in(
      &docker_state,
      window,
      |this, state, event: &StateChanged, window, cx| {
        match event {
          StateChanged::PodsUpdated => {
            // If selected pod was deleted, clear selection
            let selected_key = {
              if let Selection::Pod {
                ref name,
                ref namespace,
              } = this.docker_state.read(cx).selection
              {
                Some((name.clone(), namespace.clone()))
              } else {
                None
              }
            };

            if let Some((name, namespace)) = selected_key {
              let pod_exists = state.read(cx).get_pod(&name, &namespace).is_some();
              if !pod_exists {
                this.docker_state.update(cx, |s, _| {
                  s.set_selection(Selection::None);
                });
                this.active_tab = PodDetailTab::Info;
                this.terminal_view = None;
              }
            }
            cx.notify();
          }
          StateChanged::PodLogsLoaded {
            pod_name,
            namespace,
            logs,
          } => {
            if let Selection::Pod { name, namespace: ns } = &this.docker_state.read(cx).selection
              && name == pod_name
              && ns == namespace
            {
              logs.clone_into(&mut this.pod_tab_state.logs);
              this.pod_tab_state.logs_loading = false;
              cx.notify();
            }
          }
          StateChanged::PodDescribeLoaded {
            pod_name,
            namespace,
            describe,
          } => {
            if let Selection::Pod { name, namespace: ns } = &this.docker_state.read(cx).selection
              && name == pod_name
              && ns == namespace
            {
              describe.clone_into(&mut this.pod_tab_state.describe);
              this.pod_tab_state.describe_loading = false;
              cx.notify();
            }
          }
          StateChanged::PodYamlLoaded {
            pod_name,
            namespace,
            yaml,
          } => {
            if let Selection::Pod { name, namespace: ns } = &this.docker_state.read(cx).selection
              && name == pod_name
              && ns == namespace
            {
              yaml.clone_into(&mut this.pod_tab_state.yaml);
              this.pod_tab_state.yaml_loading = false;
              cx.notify();
            }
          }
          StateChanged::PodTabRequest {
            pod_name,
            namespace,
            tab,
          } => {
            // Find the pod and select it with the specified tab
            let pod = {
              let state = state.read(cx);
              state.get_pod(pod_name, namespace).cloned()
            };
            if let Some(pod) = pod {
              // Select in the view (updates global selection which list observes)
              this.on_select_pod(&pod, window, cx);
              this.on_tab_change(*tab, window, cx);
            }
          }
          _ => {}
        }
      },
    )
    .detach();

    // Start periodic pod and machine refresh
    let refresh_interval = settings_state(cx).read(cx).settings.container_refresh_interval;
    cx.spawn(async move |_this, cx| {
      loop {
        Timer::after(Duration::from_secs(refresh_interval)).await;
        let _ = cx.update(|cx| {
          services::refresh_machines(cx);
          services::refresh_pods(cx);
        });
      }
    })
    .detach();

    // Initial data load
    services::refresh_machines(cx);
    services::refresh_pods(cx);
    services::refresh_namespaces(cx);

    Self {
      docker_state,
      pod_list,
      active_tab: PodDetailTab::Info,
      pod_tab_state: PodTabState::new(),
      terminal_view: None,
      logs_editor: None,
      describe_editor: None,
      yaml_editor: None,
      last_synced_logs: String::new(),
      last_synced_describe: String::new(),
      last_synced_yaml: String::new(),
    }
  }

  fn on_select_pod(&mut self, pod: &PodInfo, window: &mut Window, cx: &mut Context<'_, Self>) {
    // Update global selection (single source of truth)
    self.docker_state.update(cx, |state, _cx| {
      state.set_selection(Selection::Pod {
        name: pod.name.clone(),
        namespace: pod.namespace.clone(),
      });
    });

    // Reset view-specific state
    self.active_tab = PodDetailTab::Info;
    self.terminal_view = None;

    // Clear synced tracking for new pod
    self.last_synced_logs.clear();
    self.last_synced_describe.clear();
    self.last_synced_yaml.clear();

    // Create editors for logs/describe/yaml
    self.logs_editor = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("log")
        .line_number(true)
        .searchable(true)
        .soft_wrap(false)
    }));

    self.describe_editor = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("yaml")
        .line_number(true)
        .searchable(true)
        .soft_wrap(false)
    }));

    self.yaml_editor = Some(cx.new(|cx| {
      InputState::new(window, cx)
        .multi_line(true)
        .code_editor("yaml")
        .line_number(true)
        .searchable(true)
        .soft_wrap(false)
    }));

    // Reset state
    self.pod_tab_state = PodTabState::new();
    self.pod_tab_state.selected_container = pod.containers.first().map(|c| c.name.clone());

    // Load logs for the selected pod
    self.load_pod_logs(pod, cx);

    cx.notify();
  }

  fn on_tab_change(&mut self, tab: PodDetailTab, window: &mut Window, cx: &mut Context<'_, Self>) {
    self.active_tab = tab;

    if let Some(pod) = self.selected_pod(cx) {
      match tab {
        PodDetailTab::Terminal => {
          if self.terminal_view.is_none() && matches!(pod.phase, PodPhase::Running) {
            let container = self.pod_tab_state.selected_container.clone();
            self.terminal_view = Some(cx.new(|cx| {
              TerminalView::new(
                TerminalSessionType::kubectl_exec(pod.name.clone(), pod.namespace.clone(), container, None),
                window,
                cx,
              )
            }));
          }
        }
        PodDetailTab::Describe => {
          if self.pod_tab_state.describe.is_empty() && !self.pod_tab_state.describe_loading {
            self.pod_tab_state.describe_loading = true;
            services::get_pod_describe(pod.name.clone(), pod.namespace.clone(), cx);
          }
        }
        PodDetailTab::Yaml => {
          if self.pod_tab_state.yaml.is_empty() && !self.pod_tab_state.yaml_loading {
            self.pod_tab_state.yaml_loading = true;
            services::get_pod_yaml(pod.name.clone(), pod.namespace.clone(), cx);
          }
        }
        _ => {}
      }
    }

    cx.notify();
  }

  fn load_pod_logs(&mut self, pod: &PodInfo, cx: &mut Context<'_, Self>) {
    self.pod_tab_state.logs_loading = true;
    self.pod_tab_state.logs.clear();
    cx.notify();

    // Load logs
    let container = self.pod_tab_state.selected_container.clone();
    services::get_pod_logs(pod.name.clone(), pod.namespace.clone(), container, 500, cx);
  }

  fn on_container_select(&mut self, container: &str, window: &mut Window, cx: &mut Context<'_, Self>) {
    self.pod_tab_state.selected_container = Some(container.to_string());

    // Reset terminal if we're on terminal tab
    if self.active_tab == PodDetailTab::Terminal {
      self.terminal_view = None;
    }

    // Get selected pod once for subsequent operations
    let pod = self.selected_pod(cx);

    // Reload logs if we're on logs tab
    if self.active_tab == PodDetailTab::Logs
      && let Some(ref pod) = pod
    {
      self.load_pod_logs(pod, cx);
    }

    // Recreate terminal if on terminal tab
    if self.active_tab == PodDetailTab::Terminal
      && let Some(ref pod) = pod
      && matches!(pod.phase, PodPhase::Running)
    {
      let container = self.pod_tab_state.selected_container.clone();
      self.terminal_view = Some(cx.new(|cx| {
        TerminalView::new(
          TerminalSessionType::kubectl_exec(pod.name.clone(), pod.namespace.clone(), container, None),
          window,
          cx,
        )
      }));
    }

    cx.notify();
  }

  fn on_refresh_logs(&mut self, cx: &mut Context<'_, Self>) {
    if let Some(pod) = self.selected_pod(cx) {
      self.load_pod_logs(&pod, cx);
    }
  }
}

impl Render for PodsView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    // Sync editor content with loaded data
    if let Some(ref editor) = self.logs_editor {
      let logs = &self.pod_tab_state.logs;
      if !logs.is_empty() && !self.pod_tab_state.logs_loading && self.last_synced_logs != *logs {
        let logs_clone = logs.clone();
        editor.update(cx, |state, cx| {
          state.replace(&logs_clone, window, cx);
        });
        self.last_synced_logs = logs.clone();
      }
    }

    if let Some(ref editor) = self.describe_editor {
      let describe = &self.pod_tab_state.describe;
      if !describe.is_empty() && !self.pod_tab_state.describe_loading && self.last_synced_describe != *describe {
        let describe_clone = describe.clone();
        editor.update(cx, |state, cx| {
          state.replace(&describe_clone, window, cx);
        });
        self.last_synced_describe = describe.clone();
      }
    }

    if let Some(ref editor) = self.yaml_editor {
      let yaml = &self.pod_tab_state.yaml;
      if !yaml.is_empty() && !self.pod_tab_state.yaml_loading && self.last_synced_yaml != *yaml {
        let yaml_clone = yaml.clone();
        editor.update(cx, |state, cx| {
          state.replace(&yaml_clone, window, cx);
        });
        self.last_synced_yaml = yaml.clone();
      }
    }

    let colors = cx.theme().colors;
    let selected_pod = self.selected_pod(cx);
    let active_tab = self.active_tab;
    let pod_tab_state = self.pod_tab_state.clone();
    let terminal_view = self.terminal_view.clone();
    let logs_editor = self.logs_editor.clone();
    let describe_editor = self.describe_editor.clone();
    let yaml_editor = self.yaml_editor.clone();
    let has_selection = selected_pod.is_some();

    // Build detail panel
    let detail = PodDetail::new()
      .pod(selected_pod)
      .active_tab(active_tab)
      .pod_state(pod_tab_state)
      .terminal_view(terminal_view)
      .logs_editor(logs_editor)
      .describe_editor(describe_editor)
      .yaml_editor(yaml_editor)
      .on_tab_change(cx.listener(|this, tab: &PodDetailTab, window, cx| {
        this.on_tab_change(*tab, window, cx);
      }))
      .on_refresh_logs(cx.listener(|this, (): &(), _window, cx| {
        this.on_refresh_logs(cx);
      }))
      .on_container_select(cx.listener(|this, container: &String, window, cx| {
        this.on_container_select(container, window, cx);
      }))
      .on_delete(cx.listener(|this, (name, ns): &(String, String), _window, cx| {
        services::delete_pod(name.clone(), ns.clone(), cx);
        this.docker_state.update(cx, |s, _| s.set_selection(Selection::None));
        this.active_tab = PodDetailTab::Info;
        this.terminal_view = None;
        cx.notify();
      }));

    div()
      .size_full()
      .flex()
      .overflow_hidden()
      .child(
        // Left: Pod list - fixed width when selected, full width when not
        div()
          .when(has_selection, |el| {
            el.w(px(320.)).border_r_1().border_color(colors.border)
          })
          .when(!has_selection, gpui::Styled::flex_1)
          .h_full()
          .flex_shrink_0()
          .overflow_hidden()
          .child(self.pod_list.clone()),
      )
      .when(has_selection, |el| {
        el.child(
          // Right: Detail panel - only shown when selection exists
          div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .child(detail.render(window, cx)),
        )
      })
  }
}
