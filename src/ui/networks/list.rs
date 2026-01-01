use gpui::{div, prelude::*, px, rgb, App, Context, Entity, Render, SharedString, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    theme::ActiveTheme,
    v_flex,
    Icon, IconName, IndexPath, Sizable,
};

use crate::assets::AppIcon;
use crate::docker::NetworkInfo;
use crate::services;
use crate::state::{docker_state, DockerState, StateChanged};

/// Network list events emitted to parent
pub enum NetworkListEvent {
    Selected(NetworkInfo),
}

/// Delegate for the network list
pub struct NetworkListDelegate {
    docker_state: Entity<DockerState>,
    selected_index: Option<IndexPath>,
    /// Cached list: (section_index, is_system, networks)
    sections: Vec<(bool, Vec<NetworkInfo>)>,
}

impl NetworkListDelegate {
    fn rebuild_sections(&mut self, cx: &App) {
        let state = self.docker_state.read(cx);

        let mut system = Vec::new();
        let mut custom = Vec::new();

        for network in &state.networks {
            if network.is_system_network() {
                system.push(network.clone());
            } else {
                custom.push(network.clone());
            }
        }

        self.sections.clear();
        if !custom.is_empty() {
            self.sections.push((false, custom));
        }
        if !system.is_empty() {
            self.sections.push((true, system));
        }
    }

    fn get_network(&self, ix: IndexPath) -> Option<&NetworkInfo> {
        self.sections
            .get(ix.section)
            .and_then(|(_, networks)| networks.get(ix.row))
    }
}

impl ListDelegate for NetworkListDelegate {
    type Item = ListItem;

    fn sections_count(&self, _cx: &App) -> usize {
        self.sections.len()
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        self.sections.get(section).map(|(_, nets)| nets.len()).unwrap_or(0)
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let colors = &cx.theme().colors;
        let (is_system, _) = self.sections.get(section)?;

        let title = if *is_system { "System" } else { "Custom" };

        Some(
            div()
                .w_full()
                .px(px(12.))
                .py(px(8.))
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.muted_foreground)
                .child(title)
        )
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let network = self.get_network(ix)?.clone();
        let colors = &cx.theme().colors;

        let is_selected = self.selected_index == Some(ix);
        let network_id = network.id.clone();
        let is_system = network.is_system_network();

        // Display info
        let name = network.name.clone();
        let driver = network.driver.clone();
        let container_count = network.container_count();

        let item_content = h_flex()
            .w_full()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .size(px(36.))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .bg(colors.primary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Globe).text_color(gpui::rgb(0xffffff))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .gap(px(2.))
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(Label::new(name)),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(driver)
                            .when(container_count > 0, |el| {
                                el.child(format!("{} containers", container_count))
                            }),
                    ),
            );

        let id = network_id.clone();
        let row = ix.row;
        let section = ix.section;

        let mut item = ListItem::new(ix)
            .py(px(6.))
            .rounded(px(6.))
            .selected(is_selected)
            .child(item_content)
            .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.delegate_mut().selected_index = Some(ix);
                cx.notify();
            }));

        // Only show delete button for non-system networks
        if !is_system {
            item = item.suffix(move |_, _| {
                let id = id.clone();
                div()
                    .size(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new(SharedString::from(format!("delete-{}-{}", section, row)))
                            .icon(Icon::new(AppIcon::Trash))
                            .ghost()
                            .xsmall()
                            .on_click(move |_ev, _window, cx| {
                                services::delete_network(id.clone(), cx);
                            }),
                    )
            });
        }

        Some(item)
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }
}

/// Self-contained network list component
pub struct NetworkList {
    docker_state: Entity<DockerState>,
    list_state: Entity<ListState<NetworkListDelegate>>,
}

impl NetworkList {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        let mut delegate = NetworkListDelegate {
            docker_state: docker_state.clone(),
            selected_index: None,
            sections: Vec::new(),
        };

        // Build initial sections
        delegate.rebuild_sections(cx);

        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        // Subscribe to list events
        cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| {
            match event {
                ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
                    let delegate = state.read(cx).delegate();
                    if let Some(network) = delegate.get_network(*ix) {
                        cx.emit(NetworkListEvent::Selected(network.clone()));
                    }
                }
                _ => {}
            }
        })
        .detach();

        // Subscribe to docker state changes to refresh list
        cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
            if matches!(event, StateChanged::NetworksUpdated) {
                this.list_state.update(cx, |state, cx| {
                    state.delegate_mut().rebuild_sections(cx);
                    cx.notify();
                });
                cx.notify();
            }
        })
        .detach();

        Self {
            docker_state,
            list_state,
        }
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> gpui::Div {
        let colors = &cx.theme().colors;

        v_flex()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .py(px(48.))
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
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.secondary_foreground)
                    .child("No Networks"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Create a network to get started"),
            )
    }

    fn count_networks(&self, cx: &App) -> usize {
        self.docker_state.read(cx).networks.len()
    }
}

impl gpui::EventEmitter<NetworkListEvent> for NetworkList {}

impl Render for NetworkList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let networks_empty = self.count_networks(cx) == 0;
        let count = self.count_networks(cx);

        // Toolbar
        let toolbar = h_flex()
            .h(px(52.))
            .w_full()
            .px(px(16.))
            .border_b_1()
            .border_color(rgb(0x414868))
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .child(
                v_flex()
                    .child(Label::new("Networks"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x565f89))
                            .child(format!("{} networks", count)),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("search")
                            .icon(Icon::new(AppIcon::Search))
                            .ghost()
                            .compact(),
                    ),
            );

        let content: gpui::Div = if networks_empty {
            self.render_empty(cx)
        } else {
            div()
                .size_full()
                .p(px(8.))
                .child(List::new(&self.list_state))
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(toolbar)
            .child(
                div()
                    .id("network-list-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
    }
}
