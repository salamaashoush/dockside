use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, ParentElement, Render,
    Styled, Window,
};
use gpui_component::{
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    switch::Switch,
    theme::ActiveTheme,
    v_flex,
};

use crate::docker::PruneResult;

/// What to prune
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruneType {
    Containers,
    Images,
    Volumes,
    Networks,
    System,
}

impl PruneType {
    pub fn label(&self) -> &'static str {
        match self {
            PruneType::Containers => "Containers",
            PruneType::Images => "Images",
            PruneType::Volumes => "Volumes",
            PruneType::Networks => "Networks",
            PruneType::System => "System",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PruneType::Containers => "Remove all stopped containers",
            PruneType::Images => "Remove unused images",
            PruneType::Volumes => "Remove unused volumes (data will be lost)",
            PruneType::Networks => "Remove unused networks",
            PruneType::System => "Remove all unused containers, images, and networks",
        }
    }
}

/// Options for prune operation
#[derive(Debug, Clone, Default)]
pub struct PruneOptions {
    pub prune_containers: bool,
    pub prune_images: bool,
    pub prune_volumes: bool,
    pub prune_networks: bool,
    pub images_dangling_only: bool,
}

impl PruneOptions {
    pub fn is_empty(&self) -> bool {
        !self.prune_containers && !self.prune_images && !self.prune_volumes && !self.prune_networks
    }
}

/// Result display for prune operations
#[derive(Debug, Clone, Default)]
pub struct PruneResultDisplay {
    pub result: Option<PruneResult>,
    pub is_loading: bool,
    pub error: Option<String>,
}

/// Dialog for prune operations
pub struct PruneDialog {
    focus_handle: FocusHandle,
    options: PruneOptions,
    result_display: PruneResultDisplay,
}

impl PruneDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            options: PruneOptions::default(),
            result_display: PruneResultDisplay::default(),
        }
    }

    pub fn get_options(&self) -> PruneOptions {
        self.options.clone()
    }

    pub fn set_result(&mut self, result: PruneResult) {
        self.result_display.result = Some(result);
        self.result_display.is_loading = false;
        self.result_display.error = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.result_display.error = Some(error);
        self.result_display.is_loading = false;
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.result_display.is_loading = loading;
        if loading {
            self.result_display.result = None;
            self.result_display.error = None;
        }
    }

    pub fn reset(&mut self) {
        self.options = PruneOptions::default();
        self.result_display = PruneResultDisplay::default();
    }
}

impl Focusable for PruneDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PruneDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        let containers_checked = self.options.prune_containers;
        let images_checked = self.options.prune_images;
        let volumes_checked = self.options.prune_volumes;
        let networks_checked = self.options.prune_networks;
        let dangling_only = self.options.images_dangling_only;

        // Helper to render form row
        let render_form_row = |label: &'static str, description: &'static str, content: gpui::AnyElement, border: Hsla, fg: Hsla, muted: Hsla| {
            h_flex()
                .w_full()
                .py(px(12.))
                .px(px(16.))
                .justify_between()
                .items_center()
                .border_b_1()
                .border_color(border)
                .child(
                    v_flex()
                        .gap(px(2.))
                        .child(Label::new(label).text_color(fg))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(description),
                        ),
                )
                .child(content)
        };

        // Render result section
        let result_section = {
            let display = &self.result_display;

            if display.is_loading {
                div()
                    .w_full()
                    .p(px(16.))
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.link)
                            .child("Pruning..."),
                    )
            } else if let Some(error) = &display.error {
                div()
                    .w_full()
                    .p(px(16.))
                    .bg(colors.danger.opacity(0.1))
                    .rounded(px(4.))
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.danger)
                            .child(format!("Error: {}", error)),
                    )
            } else if let Some(result) = &display.result {
                v_flex()
                    .w_full()
                    .p(px(16.))
                    .gap(px(8.))
                    .bg(colors.sidebar)
                    .rounded(px(4.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.success)
                            .child("Prune completed"),
                    )
                    .when(!result.containers_deleted.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(colors.secondary_foreground)
                                .child(format!(
                                    "Containers removed: {}",
                                    result.containers_deleted.len()
                                )),
                        )
                    })
                    .when(!result.images_deleted.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(colors.secondary_foreground)
                                .child(format!("Images removed: {}", result.images_deleted.len())),
                        )
                    })
                    .when(!result.volumes_deleted.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(colors.secondary_foreground)
                                .child(format!("Volumes removed: {}", result.volumes_deleted.len())),
                        )
                    })
                    .when(!result.networks_deleted.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(colors.secondary_foreground)
                                .child(format!(
                                    "Networks removed: {}",
                                    result.networks_deleted.len()
                                )),
                        )
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.link)
                            .child(format!(
                                "Space reclaimed: {}",
                                result.display_space_reclaimed()
                            )),
                    )
                    .when(result.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("Nothing to prune"),
                        )
                    })
            } else {
                div()
            }
        };

        v_flex()
            .w_full()
            .max_h(px(500.))
            .overflow_y_scrollbar()
            // Header description
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Select what to clean up. This will remove unused Docker resources to free up disk space."),
            )
            // Containers
            .child(render_form_row(
                "Containers",
                "Remove all stopped containers",
                Switch::new("containers")
                    .checked(containers_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_containers = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Images
            .child(render_form_row(
                "Images",
                "Remove unused images",
                Switch::new("images")
                    .checked(images_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_images = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Images dangling only option (sub-option)
            .when(images_checked, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .py(px(8.))
                        .px(px(32.))
                        .justify_between()
                        .items_center()
                        .border_b_1()
                        .border_color(colors.border)
                        .bg(colors.sidebar)
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("Only dangling images (untagged)"),
                        )
                        .child(
                            Switch::new("dangling-only")
                                .checked(dangling_only)
                                .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                    this.options.images_dangling_only = *checked;
                                    cx.notify();
                                })),
                        ),
                )
            })
            // Volumes
            .child(render_form_row(
                "Volumes",
                "Remove unused volumes (warning: data loss)",
                Switch::new("volumes")
                    .checked(volumes_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_volumes = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Warning for volumes
            .when(volumes_checked, |this| {
                this.child(
                    div()
                        .w_full()
                        .px(px(16.))
                        .py(px(8.))
                        .bg(colors.danger.opacity(0.1))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.danger)
                                .child("Warning: Removing volumes will permanently delete data!"),
                        ),
                )
            })
            // Networks
            .child(render_form_row(
                "Networks",
                "Remove unused networks",
                Switch::new("networks")
                    .checked(networks_checked)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.options.prune_networks = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Result display
            .child(result_section)
    }
}
