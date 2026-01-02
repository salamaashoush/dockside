use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Timer, Window};
use std::time::Duration;
use gpui_component::theme::ActiveTheme;

use crate::kubernetes::{PodInfo, PodPhase};
use crate::services;
use crate::state::{docker_state, settings_state, DockerState, StateChanged};
use crate::terminal::{TerminalSessionType, TerminalView};

use super::detail::{PodDetail, PodTabState};
use super::list::{PodList, PodListEvent};

/// Self-contained Pods view - handles list, detail, and all state
pub struct PodsView {
    docker_state: Entity<DockerState>,
    pod_list: Entity<PodList>,
    selected_pod: Option<PodInfo>,
    active_tab: usize,
    pod_tab_state: PodTabState,
    terminal_view: Option<Entity<TerminalView>>,
}

impl PodsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Create pod list entity
        let pod_list = cx.new(|cx| PodList::new(window, cx));

        // Subscribe to pod list events
        cx.subscribe_in(&pod_list, window, |this, _list, event: &PodListEvent, _window, cx| {
            match event {
                PodListEvent::Selected(pod) => {
                    this.on_select_pod(pod, cx);
                }
                PodListEvent::NamespaceChanged(_ns) => {
                    // Namespace change is handled by the list
                    cx.notify();
                }
            }
        })
        .detach();

        // Subscribe to state changes
        cx.subscribe_in(&docker_state, window, |this, state, event: &StateChanged, window, cx| {
            match event {
                StateChanged::PodsUpdated => {
                    // If selected pod was deleted, clear selection
                    if let Some(ref selected) = this.selected_pod {
                        let state = state.read(cx);
                        if state.get_pod(&selected.name, &selected.namespace).is_none() {
                            this.selected_pod = None;
                            this.active_tab = 0;
                            this.terminal_view = None;
                        } else {
                            // Update the selected pod info
                            if let Some(updated) = state.get_pod(&selected.name, &selected.namespace) {
                                this.selected_pod = Some(updated.clone());
                            }
                        }
                    }
                    cx.notify();
                }
                StateChanged::PodLogsLoaded { pod_name, namespace, logs } => {
                    if let Some(ref pod) = this.selected_pod {
                        if pod.name == *pod_name && pod.namespace == *namespace {
                            this.pod_tab_state.logs = logs.clone();
                            this.pod_tab_state.logs_loading = false;
                            cx.notify();
                        }
                    }
                }
                StateChanged::PodDescribeLoaded { pod_name, namespace, describe } => {
                    if let Some(ref pod) = this.selected_pod {
                        if pod.name == *pod_name && pod.namespace == *namespace {
                            this.pod_tab_state.describe = describe.clone();
                            this.pod_tab_state.describe_loading = false;
                            cx.notify();
                        }
                    }
                }
                StateChanged::PodYamlLoaded { pod_name, namespace, yaml } => {
                    if let Some(ref pod) = this.selected_pod {
                        if pod.name == *pod_name && pod.namespace == *namespace {
                            this.pod_tab_state.yaml = yaml.clone();
                            this.pod_tab_state.yaml_loading = false;
                            cx.notify();
                        }
                    }
                }
                StateChanged::PodTabRequest { pod_name, namespace, tab } => {
                    // Find the pod and select it with the specified tab
                    let pod = {
                        let state = state.read(cx);
                        state.get_pod(pod_name, namespace).cloned()
                    };
                    if let Some(pod) = pod {
                        // Select in the list UI
                        this.pod_list.update(cx, |list, cx| {
                            list.select_pod(pod_name, namespace, cx);
                        });
                        // Select in the view
                        this.on_select_pod(&pod, cx);
                        this.on_tab_change(*tab, window, cx);
                    }
                }
                _ => {}
            }
        })
        .detach();

        // Start periodic pod refresh
        let refresh_interval = settings_state(cx).read(cx).settings.container_refresh_interval;
        cx.spawn(async move |_this, cx| {
            loop {
                Timer::after(Duration::from_secs(refresh_interval)).await;
                let _ = cx.update(|cx| {
                    services::refresh_pods(cx);
                });
            }
        })
        .detach();

        // Initial data load
        services::refresh_pods(cx);
        services::refresh_namespaces(cx);

        Self {
            docker_state,
            pod_list,
            selected_pod: None,
            active_tab: 0,
            pod_tab_state: PodTabState::new(),
            terminal_view: None,
        }
    }

    fn on_select_pod(&mut self, pod: &PodInfo, cx: &mut Context<Self>) {
        self.selected_pod = Some(pod.clone());
        self.active_tab = 0;
        self.terminal_view = None;

        // Reset state
        self.pod_tab_state = PodTabState::new();
        self.pod_tab_state.selected_container = pod.containers.first().map(|c| c.name.clone());

        // Load logs for the selected pod
        self.load_pod_logs(pod, cx);

        cx.notify();
    }

    fn on_tab_change(&mut self, tab: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = tab;

        if let Some(ref pod) = self.selected_pod.clone() {
            match tab {
                // Terminal tab
                2 => {
                    if self.terminal_view.is_none() && matches!(pod.phase, PodPhase::Running) {
                        let container = self.pod_tab_state.selected_container.clone();
                        self.terminal_view = Some(cx.new(|cx| {
                            TerminalView::new(
                                TerminalSessionType::kubectl_exec(
                                    pod.name.clone(),
                                    pod.namespace.clone(),
                                    container,
                                    None,
                                ),
                                window,
                                cx,
                            )
                        }));
                    }
                }
                // Describe tab
                3 => {
                    if self.pod_tab_state.describe.is_empty() && !self.pod_tab_state.describe_loading {
                        self.pod_tab_state.describe_loading = true;
                        services::get_pod_describe(
                            pod.name.clone(),
                            pod.namespace.clone(),
                            cx,
                        );
                    }
                }
                // YAML tab
                4 => {
                    if self.pod_tab_state.yaml.is_empty() && !self.pod_tab_state.yaml_loading {
                        self.pod_tab_state.yaml_loading = true;
                        services::get_pod_yaml(
                            pod.name.clone(),
                            pod.namespace.clone(),
                            cx,
                        );
                    }
                }
                _ => {}
            }
        }

        cx.notify();
    }

    fn load_pod_logs(&mut self, pod: &PodInfo, cx: &mut Context<Self>) {
        self.pod_tab_state.logs_loading = true;
        self.pod_tab_state.logs.clear();
        cx.notify();

        // Load logs
        let container = self.pod_tab_state.selected_container.clone();
        services::get_pod_logs(
            pod.name.clone(),
            pod.namespace.clone(),
            container,
            500,
            cx,
        );
    }

    fn on_container_select(&mut self, container: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.pod_tab_state.selected_container = Some(container.to_string());

        // Reset terminal if we're on terminal tab
        if self.active_tab == 2 {
            self.terminal_view = None;
        }

        // Reload logs if we're on logs tab
        if self.active_tab == 1 {
            if let Some(ref pod) = self.selected_pod.clone() {
                self.load_pod_logs(&pod, cx);
            }
        }

        // Recreate terminal if on terminal tab
        if self.active_tab == 2 {
            if let Some(ref pod) = self.selected_pod.clone() {
                if matches!(pod.phase, PodPhase::Running) {
                    let container = self.pod_tab_state.selected_container.clone();
                    self.terminal_view = Some(cx.new(|cx| {
                        TerminalView::new(
                            TerminalSessionType::kubectl_exec(
                                pod.name.clone(),
                                pod.namespace.clone(),
                                container,
                                None,
                            ),
                            window,
                            cx,
                        )
                    }));
                }
            }
        }

        cx.notify();
    }

    fn on_refresh_logs(&mut self, cx: &mut Context<Self>) {
        if let Some(ref pod) = self.selected_pod.clone() {
            self.load_pod_logs(&pod, cx);
        }
    }
}

impl Render for PodsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        let selected_pod = self.selected_pod.clone();
        let active_tab = self.active_tab;
        let pod_tab_state = self.pod_tab_state.clone();
        let terminal_view = self.terminal_view.clone();

        // Build detail panel
        let detail = PodDetail::new()
            .pod(selected_pod)
            .active_tab(active_tab)
            .pod_state(pod_tab_state)
            .terminal_view(terminal_view)
            .on_tab_change(cx.listener(|this, tab: &usize, window, cx| {
                this.on_tab_change(*tab, window, cx);
            }))
            .on_refresh_logs(cx.listener(|this, _: &(), _window, cx| {
                this.on_refresh_logs(cx);
            }))
            .on_container_select(cx.listener(|this, container: &String, window, cx| {
                this.on_container_select(container, window, cx);
            }))
            .on_delete(cx.listener(|this, (name, ns): &(String, String), _window, cx| {
                crate::services::delete_pod(name.clone(), ns.clone(), cx);
                this.selected_pod = None;
                this.active_tab = 0;
                this.terminal_view = None;
                cx.notify();
            }));

        div()
            .size_full()
            .flex()
            .overflow_hidden()
            .child(
                // Left: Pod list - fixed width with border
                div()
                    .w(px(320.))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .border_r_1()
                    .border_color(colors.border)
                    .child(self.pod_list.clone()),
            )
            .child(
                // Right: Detail panel - flexible width
                div().flex_1().h_full().overflow_hidden().child(detail.render(window, cx)),
            )
    }
}
