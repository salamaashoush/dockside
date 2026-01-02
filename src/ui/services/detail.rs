use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    tab::{Tab, TabBar},
    theme::ActiveTheme,
    v_flex,
    Icon, IconName, Selectable, Sizable,
};

use crate::kubernetes::{PodInfo, ServiceInfo};
use crate::services;
use crate::state::{docker_state, DockerState, StateChanged};

/// Detail view for a service with tabs
pub struct ServiceDetail {
    docker_state: Entity<DockerState>,
    service: Option<ServiceInfo>,
    active_tab: usize,
    yaml_content: String,
}

impl ServiceDetail {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        // Subscribe to state changes
        cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
            match event {
                StateChanged::ServiceYamlLoaded {
                    service_name,
                    namespace,
                    yaml,
                } => {
                    if let Some(ref svc) = this.service {
                        if svc.name == *service_name && svc.namespace == *namespace {
                            this.yaml_content = yaml.clone();
                            cx.notify();
                        }
                    }
                }
                StateChanged::ServiceTabRequest {
                    service_name,
                    namespace,
                    tab,
                } => {
                    // Find and select the service
                    let state = _state.read(cx);
                    if let Some(svc) = state.get_service(service_name, namespace) {
                        this.service = Some(svc.clone());
                        this.active_tab = *tab;
                        this.yaml_content.clear();
                        cx.notify();
                    }
                }
                StateChanged::ServicesUpdated => {
                    // Refresh current service if still exists
                    if let Some(ref current) = this.service {
                        let state = _state.read(cx);
                        if let Some(updated) =
                            state.get_service(&current.name, &current.namespace)
                        {
                            this.service = Some(updated.clone());
                            cx.notify();
                        }
                    }
                }
                _ => {}
            }
        })
        .detach();

        Self {
            docker_state,
            service: None,
            active_tab: 0,
            yaml_content: String::new(),
        }
    }

    pub fn set_service(&mut self, service: ServiceInfo, cx: &mut Context<Self>) {
        self.service = Some(service.clone());
        self.active_tab = 0;
        self.yaml_content.clear();

        // Load YAML
        services::get_service_yaml(service.name, service.namespace, cx);

        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.service = None;
        self.active_tab = 0;
        self.yaml_content.clear();
        cx.notify();
    }

    fn render_info_tab(&self, service: &ServiceInfo, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        let info_row = |label: &str, value: String| {
            h_flex()
                .w_full()
                .py(px(8.))
                .gap(px(16.))
                .child(
                    div()
                        .w(px(120.))
                        .flex_shrink_0()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(colors.muted_foreground)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(colors.foreground)
                        .child(value),
                )
        };

        let mut content = v_flex()
            .w_full()
            .gap(px(4.))
            .child(info_row("Name", service.name.clone()))
            .child(info_row("Namespace", service.namespace.clone()))
            .child(info_row("Type", service.service_type.clone()))
            .child(info_row(
                "Cluster IP",
                service
                    .cluster_ip
                    .clone()
                    .unwrap_or_else(|| "None".to_string()),
            ));

        if !service.external_ips.is_empty() {
            content = content.child(info_row("External IPs", service.external_ips.join(", ")));
        }

        content = content
            .child(info_row("Ports", service.ports_display()))
            .child(info_row("Age", service.age.clone()));

        // Selector labels
        if !service.selector.is_empty() {
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
                            .child("Selector"),
                    )
                    .child(
                        div()
                            .w_full()
                            .p(px(12.))
                            .rounded(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                v_flex().gap(px(4.)).children(
                                    service
                                        .selector
                                        .iter()
                                        .map(|(k, v)| {
                                            div()
                                                .text_xs()
                                                .font_family("monospace")
                                                .text_color(colors.muted_foreground)
                                                .child(format!("{}={}", k, v))
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                            ),
                    ),
            );
        }

        // Labels
        if !service.labels.is_empty() {
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
                        div()
                            .w_full()
                            .p(px(12.))
                            .rounded(px(8.))
                            .bg(colors.sidebar)
                            .child(
                                v_flex().gap(px(4.)).children(
                                    service
                                        .labels
                                        .iter()
                                        .map(|(k, v)| {
                                            div()
                                                .text_xs()
                                                .font_family("monospace")
                                                .text_color(colors.muted_foreground)
                                                .child(format!("{}={}", k, v))
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                            ),
                    ),
            );
        }

        div().size_full().p(px(16.)).child(content)
    }

    fn render_ports_tab(&self, service: &ServiceInfo, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        if service.ports.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Icon::new(IconName::Info).text_color(colors.muted_foreground))
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("No ports configured"),
                        ),
                );
        }

        let header = h_flex()
            .w_full()
            .py(px(8.))
            .px(px(12.))
            .bg(colors.sidebar)
            .rounded_t(px(8.))
            .child(
                div()
                    .w(px(100.))
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Name"),
            )
            .child(
                div()
                    .w(px(80.))
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Protocol"),
            )
            .child(
                div()
                    .w(px(80.))
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Port"),
            )
            .child(
                div()
                    .w(px(100.))
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Target Port"),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Node Port"),
            );

        let rows = service
            .ports
            .iter()
            .enumerate()
            .map(|(i, port)| {
                h_flex()
                    .w_full()
                    .py(px(10.))
                    .px(px(12.))
                    .when(i % 2 == 1, |el| el.bg(colors.sidebar.opacity(0.5)))
                    .child(
                        div()
                            .w(px(100.))
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(port.name.clone().unwrap_or_else(|| "-".to_string())),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(port.protocol.clone()),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(port.port.to_string()),
                    )
                    .child(
                        div()
                            .w(px(100.))
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(port.target_port.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(
                                port.node_port
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                            ),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .size_full()
            .p(px(16.))
            .child(v_flex().w_full().child(header).children(rows))
    }

    fn render_endpoints_tab(&self, service: &ServiceInfo, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        if service.selector.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Icon::new(IconName::Info).text_color(colors.muted_foreground))
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("No selector defined"),
                        ),
                );
        }

        // Get pods that match the service selector
        let state = self.docker_state.read(cx);
        let matching_pods: Vec<&PodInfo> = state
            .pods
            .iter()
            .filter(|pod| {
                // Pod must be in same namespace
                if pod.namespace != service.namespace {
                    return false;
                }
                // All selector labels must match pod labels
                service.selector.iter().all(|(key, value)| {
                    pod.labels.get(key).map(|v| v == value).unwrap_or(false)
                })
            })
            .collect();

        if matching_pods.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Icon::new(IconName::Info).text_color(colors.muted_foreground))
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("No matching pods found"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(format!(
                                    "Selector: {}",
                                    service
                                        .selector
                                        .iter()
                                        .map(|(k, v)| format!("{}={}", k, v))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )),
                        ),
                );
        }

        let header = h_flex()
            .w_full()
            .py(px(8.))
            .px(px(12.))
            .gap(px(8.))
            .bg(colors.sidebar)
            .rounded_t(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Pod Name"),
            )
            .child(
                div()
                    .w(px(80.))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Status"),
            )
            .child(
                div()
                    .w(px(110.))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("IP"),
            )
            .child(
                div()
                    .w(px(50.))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .child("Ready"),
            )
            .child(
                div()
                    .w(px(40.))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.muted_foreground)
                    .text_right(),
            );

        let rows = matching_pods
            .iter()
            .enumerate()
            .map(|(i, pod)| {
                let status_color = if pod.phase.is_running() {
                    colors.success
                } else if pod.phase.is_pending() {
                    colors.warning
                } else {
                    colors.danger
                };

                let pod_name = pod.name.clone();
                let pod_namespace = pod.namespace.clone();

                h_flex()
                    .w_full()
                    .py(px(8.))
                    .px(px(12.))
                    .gap(px(8.))
                    .rounded(px(6.))
                    .when(i % 2 == 1, |el| el.bg(colors.sidebar.opacity(0.3)))
                    .hover(|el| el.bg(colors.sidebar))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(colors.foreground)
                            .font_family("monospace")
                            .text_ellipsis()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(pod.name.clone()),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .bg(status_color.opacity(0.15))
                                    .text_xs()
                                    .text_color(status_color)
                                    .child(pod.phase.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .w(px(110.))
                            .flex_shrink_0()
                            .text_sm()
                            .font_family("monospace")
                            .text_color(colors.muted_foreground)
                            .child(pod.ip.clone().unwrap_or_else(|| "-".to_string())),
                    )
                    .child(
                        div()
                            .w(px(50.))
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(pod.ready.clone()),
                    )
                    .child(
                        div()
                            .w(px(40.))
                            .flex_shrink_0()
                            .flex()
                            .justify_end()
                            .child(
                                Button::new(("view-pod", i))
                                    .icon(IconName::Eye)
                                    .ghost()
                                    .xsmall()
                                    .on_click(move |_ev, _window, cx| {
                                        services::open_pod_info(
                                            pod_name.clone(),
                                            pod_namespace.clone(),
                                            cx,
                                        );
                                    }),
                            ),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .size_full()
            .p(px(16.))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!("{} pod(s) matching selector", matching_pods.len())),
                    )
                    .child(v_flex().w_full().child(header).children(rows)),
            )
    }

    fn render_yaml_tab(&self, _service: &ServiceInfo, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        if self.yaml_content.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("Loading YAML..."),
                );
        }

        div()
            .size_full()
            .p(px(16.))
            .child(
                div()
                    .w_full()
                    .h_full()
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(colors.sidebar)
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(colors.foreground)
                            .whitespace_nowrap()
                            .child(self.yaml_content.clone()),
                    ),
            )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
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
                            .child(Icon::new(IconName::Globe).text_color(colors.muted_foreground)),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.secondary_foreground)
                            .child("Select a Service"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child("Click on a service to view details"),
                    ),
            )
    }
}

impl Render for ServiceDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();

        let Some(service) = self.service.clone() else {
            return div().size_full().child(self.render_empty(cx));
        };

        let active_tab = self.active_tab;

        // Tab bar
        let tab_bar = TabBar::new("service-tabs")
            .py(px(0.))
            .child(
                Tab::new()
                    .label("Info")
                    .selected(active_tab == 0)
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.active_tab = 0;
                        cx.notify();
                    })),
            )
            .child(
                Tab::new()
                    .label("Ports")
                    .selected(active_tab == 1)
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.active_tab = 1;
                        cx.notify();
                    })),
            )
            .child(
                Tab::new()
                    .label("Endpoints")
                    .selected(active_tab == 2)
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.active_tab = 2;
                        cx.notify();
                    })),
            )
            .child(
                Tab::new()
                    .label("YAML")
                    .selected(active_tab == 3)
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.active_tab = 3;
                        if let Some(ref svc) = this.service {
                            services::get_service_yaml(
                                svc.name.clone(),
                                svc.namespace.clone(),
                                cx,
                            );
                        }
                    })),
            );

        // Tab content
        let content = match active_tab {
            0 => self.render_info_tab(&service, cx),
            1 => self.render_ports_tab(&service, cx),
            2 => self.render_endpoints_tab(&service, cx),
            3 => self.render_yaml_tab(&service, cx),
            _ => div(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .h(px(52.))
                    .px(px(16.))
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .flex_shrink_0()
                    .child(
                        v_flex().child(Label::new(service.name.clone())).child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(format!("{} - {}", service.namespace, service.service_type)),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .px(px(16.))
                    .border_b_1()
                    .border_color(colors.border)
                    .flex_shrink_0()
                    .child(tab_bar),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
    }
}
