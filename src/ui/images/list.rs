use gpui::{div, prelude::*, px, App, Context, Entity, Render, SharedString, Styled, Window};
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
use crate::docker::ImageInfo;
use crate::services;
use crate::state::{docker_state, DockerState, StateChanged};

/// Image list events emitted to parent
pub enum ImageListEvent {
    Selected(ImageInfo),
    PullImage,
}

/// Delegate for the image list
pub struct ImageListDelegate {
    docker_state: Entity<DockerState>,
    selected_index: Option<IndexPath>,
    /// Cached list: (section_index, is_in_use, images)
    sections: Vec<(bool, Vec<ImageInfo>)>,
}

impl ImageListDelegate {
    fn rebuild_sections(&mut self, cx: &App) {
        let state = self.docker_state.read(cx);
        let containers = &state.containers;

        // Get all image IDs that are in use by containers
        let in_use_ids: std::collections::HashSet<String> = containers
            .iter()
            .map(|c| c.image_id.clone())
            .collect();

        let mut in_use = Vec::new();
        let mut unused = Vec::new();

        for image in &state.images {
            if in_use_ids.contains(&image.id) {
                in_use.push(image.clone());
            } else {
                unused.push(image.clone());
            }
        }

        self.sections.clear();
        if !in_use.is_empty() {
            self.sections.push((true, in_use));
        }
        if !unused.is_empty() {
            self.sections.push((false, unused));
        }
    }

    fn get_image(&self, ix: IndexPath) -> Option<&ImageInfo> {
        self.sections
            .get(ix.section)
            .and_then(|(_, images)| images.get(ix.row))
    }
}

impl ListDelegate for ImageListDelegate {
    type Item = ListItem;

    fn sections_count(&self, _cx: &App) -> usize {
        self.sections.len()
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        self.sections.get(section).map(|(_, imgs)| imgs.len()).unwrap_or(0)
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let colors = &cx.theme().colors;
        let (is_in_use, _) = self.sections.get(section)?;

        let title = if *is_in_use { "In Use" } else { "Unused" };

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
        let image = self.get_image(ix)?.clone();
        let colors = &cx.theme().colors;

        let is_selected = self.selected_index == Some(ix);
        let image_id = image.id.clone();

        // Display name (repo:tag or short id)
        let display_name = image.display_name();
        let size_text = image.display_size();

        // Age display
        let age_text = image.created.map(|created| {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(created);
            if duration.num_days() > 365 {
                format!("{} years ago", duration.num_days() / 365)
            } else if duration.num_days() > 30 {
                format!("{} months ago", duration.num_days() / 30)
            } else if duration.num_days() > 0 {
                format!("{} days ago", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{} hours ago", duration.num_hours())
            } else {
                "just now".to_string()
            }
        }).unwrap_or_else(|| "Unknown".to_string());

        // Platform badge
        let platform = image.architecture.as_ref().map(|arch| {
            if arch.contains("arm") || arch.contains("aarch") {
                "arm64"
            } else {
                "amd64"
            }
        });

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
                    .child(Icon::new(AppIcon::Container).text_color(colors.background)),
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
                            .child(Label::new(display_name)),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(size_text)
                            .child(age_text),
                    ),
            )
            .when_some(platform, |el, plat| {
                el.child(
                    div()
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .bg(colors.primary)
                        .text_xs()
                        .text_color(colors.background)
                        .child(plat)
                )
            });

        let id = image_id.clone();
        let row = ix.row;
        let section = ix.section;

        let item = ListItem::new(ix)
            .py(px(6.))
            .rounded(px(6.))
            .selected(is_selected)
            .child(item_content)
            .suffix(move |_, _| {
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
                                services::delete_image(id.clone(), cx);
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

/// Self-contained image list component
pub struct ImageList {
    docker_state: Entity<DockerState>,
    list_state: Entity<ListState<ImageListDelegate>>,
}

impl ImageList {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let docker_state = docker_state(cx);

        let mut delegate = ImageListDelegate {
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
                    if let Some(image) = delegate.get_image(*ix) {
                        cx.emit(ImageListEvent::Selected(image.clone()));
                    }
                }
                _ => {}
            }
        })
        .detach();

        // Subscribe to docker state changes to refresh list
        cx.subscribe(&docker_state, |this, _state, event: &StateChanged, cx| {
            if matches!(event, StateChanged::ImagesUpdated | StateChanged::ContainersUpdated) {
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
                    .child(Icon::new(AppIcon::Container).text_color(colors.muted_foreground)),
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.secondary_foreground)
                    .child("No Images"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Pull an image to get started"),
            )
    }

    fn calculate_total_size(&self, cx: &App) -> String {
        let state = self.docker_state.read(cx);
        let total: i64 = state.images.iter().map(|i| i.size).sum();
        bytesize::ByteSize(total as u64).to_string()
    }
}

impl gpui::EventEmitter<ImageListEvent> for ImageList {}

impl Render for ImageList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.docker_state.read(cx);
        let images_empty = state.images.is_empty();
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
                    .child(Label::new("Images"))
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
                        Button::new("sort")
                            .icon(IconName::ArrowDown)
                            .ghost()
                            .compact(),
                    )
                    .child(
                        Button::new("search")
                            .icon(Icon::new(AppIcon::Search))
                            .ghost()
                            .compact(),
                    )
                    .child(
                        Button::new("pull")
                            .icon(Icon::new(AppIcon::Plus))
                            .ghost()
                            .compact()
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(ImageListEvent::PullImage);
                            })),
                    ),
            );

        let content: gpui::Div = if images_empty {
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
                    .id("image-list-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
    }
}
