use gpui::{div, prelude::*, px, App, Context, Entity, Render, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    theme::ActiveTheme,
    v_flex,
    Icon, IndexPath, Sizable,
};

use crate::assets::AppIcon;
use crate::docker::VolumeInfo;
use crate::services;
use crate::state::{docker_state, DockerState, StateChanged};

/// Volume list events emitted to parent
pub enum VolumeListEvent {
    Selected(VolumeInfo),
    NewVolume,
}

/// Delegate for the volume list
pub struct VolumeListDelegate {
    docker_state: Entity<DockerState>,
    selected_index: Option<IndexPath>,
}

impl VolumeListDelegate {
    fn volumes<'a>(&self, cx: &'a App) -> &'a Vec<VolumeInfo> {
        &self.docker_state.read(cx).volumes
    }
}

impl ListDelegate for VolumeListDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, cx: &App) -> usize {
        self.volumes(cx).len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let volumes = self.volumes(cx);
        let volume = volumes.get(ix.row)?;
        let colors = &cx.theme().colors;

        let is_selected = self.selected_index == Some(ix);
        let is_in_use = volume.is_in_use();
        let volume_name = volume.name.clone();

        let icon_bg = if is_in_use {
            colors.primary
        } else {
            colors.muted_foreground
        };

        let size_text = volume.display_size();

        let item_content = h_flex()
            .w_full()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .size(px(36.))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .bg(icon_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(AppIcon::Volume).text_color(colors.background)),
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
                            .child(Label::new(volume.name.clone())),
                    )
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(size_text),
                    ),
            );

        // Delete button
        let name = volume_name.clone();
        let row = ix.row;

        let item = ListItem::new(ix)
            .py(px(6.))
            .rounded(px(6.))
            .selected(is_selected)
            .child(item_content)
            .suffix(move |_, _| {
                let n = name.clone();
                div()
                    .size(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Button::new(("delete", row))
                            .icon(Icon::new(AppIcon::Trash))
                            .ghost()
                            .xsmall()
                            .on_click(move |_ev, _window, cx| {
                                services::delete_volume(n.clone(), cx);
                            }),
                    )
            })
            .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.delegate_mut().selected_index = Some(ix);
                cx.notify();
            }));

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

/// Self-contained volume list component
pub struct VolumeList {
    docker_state: Entity<DockerState>,
    list_state: Entity<ListState<VolumeListDelegate>>,
}

impl VolumeList {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        let delegate = VolumeListDelegate {
            docker_state: docker_state.clone(),
            selected_index: None,
        };

        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        // Subscribe to list events
        cx.subscribe(&list_state, |_this, state, event: &ListEvent, cx| {
            match event {
                ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
                    let delegate = state.read(cx).delegate();
                    if let Some(volume) = delegate.volumes(cx).get(ix.row) {
                        cx.emit(VolumeListEvent::Selected(volume.clone()));
                    }
                }
                _ => {}
            }
        })
        .detach();

        // Subscribe to docker state changes to refresh list
        cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
            if matches!(event, StateChanged::VolumesUpdated) {
                this.list_state.update(cx, |_state, cx| {
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
                    .child(Icon::new(AppIcon::Volume).text_color(colors.muted_foreground)),
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.secondary_foreground)
                    .child("No Volumes"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Volumes will appear here when created"),
            )
    }

    fn calculate_total_size(&self, cx: &App) -> String {
        let state = self.docker_state.read(cx);
        let total: i64 = state
            .volumes
            .iter()
            .filter_map(|v| v.usage_data.as_ref().map(|u| u.size))
            .sum();
        bytesize::ByteSize(total as u64).to_string()
    }
}

impl gpui::EventEmitter<VolumeListEvent> for VolumeList {}

impl Render for VolumeList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.docker_state.read(cx);
        let volumes_empty = state.volumes.is_empty();
        let total_size = self.calculate_total_size(cx);
        let colors = &cx.theme().colors;

        // Toolbar
        let toolbar = h_flex()
            .h(px(52.))
            .w_full()
            .px(px(16.))
            .border_b_1()
            .border_color(cx.theme().colors.border)
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .child(
                v_flex()
                    .child(Label::new("Volumes"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!("{} total", total_size)),
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
                    )
                    .child(
                        Button::new("add")
                            .icon(Icon::new(AppIcon::Plus))
                            .ghost()
                            .compact()
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(VolumeListEvent::NewVolume);
                            })),
                    ),
            );

        let content: gpui::Div = if volumes_empty {
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
                    .id("volume-list-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
    }
}
