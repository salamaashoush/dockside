use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, Hsla, Render,
    SharedString, Styled, Window,
};
use gpui_component::{
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectItem, SelectState},
    theme::ActiveTheme,
    v_flex, IndexPath, Sizable,
};

/// Platform options for pulling images
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PullPlatform {
    #[default]
    Default,
    LinuxAmd64,
    LinuxArm64,
}

impl PullPlatform {
    pub fn label(&self) -> &'static str {
        match self {
            PullPlatform::Default => "Default",
            PullPlatform::LinuxAmd64 => "linux/amd64",
            PullPlatform::LinuxArm64 => "linux/arm64",
        }
    }

    pub fn as_docker_arg(&self) -> Option<&'static str> {
        match self {
            PullPlatform::Default => None,
            PullPlatform::LinuxAmd64 => Some("linux/amd64"),
            PullPlatform::LinuxArm64 => Some("linux/arm64"),
        }
    }

    pub fn all() -> Vec<PullPlatform> {
        vec![
            PullPlatform::Default,
            PullPlatform::LinuxAmd64,
            PullPlatform::LinuxArm64,
        ]
    }
}

impl SelectItem for PullPlatform {
    type Value = PullPlatform;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

/// Options for pulling an image
#[derive(Debug, Clone, Default)]
pub struct PullImageOptions {
    pub image: String,
    pub platform: PullPlatform,
}

/// Dialog for pulling a new image
pub struct PullImageDialog {
    focus_handle: FocusHandle,
    image_input: Option<Entity<InputState>>,
    platform_select: Option<Entity<SelectState<Vec<PullPlatform>>>>,
}

impl PullImageDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            image_input: None,
            platform_select: None,
        }
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.image_input.is_none() {
            self.image_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("e.g., nginx:latest, ubuntu:22.04")
            }));
        }

        if self.platform_select.is_none() {
            self.platform_select = Some(cx.new(|cx| {
                SelectState::new(PullPlatform::all(), Some(IndexPath::new(0)), window, cx)
            }));
        }
    }

    pub fn get_options(&self, cx: &App) -> PullImageOptions {
        let image = self
            .image_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .unwrap_or_default();

        let platform = self
            .platform_select
            .as_ref()
            .and_then(|s| s.read(cx).selected_value().cloned())
            .unwrap_or_default();

        PullImageOptions { image, platform }
    }
}

impl Focusable for PullImageDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PullImageDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);
        let colors = cx.theme().colors.clone();

        let image_input = self.image_input.clone().unwrap();
        let platform_select = self.platform_select.clone().unwrap();

        // Helper to render form row
        let render_form_row = |label: &'static str, content: gpui::AnyElement, border: Hsla, fg: Hsla| {
            h_flex()
                .w_full()
                .py(px(12.))
                .px(px(16.))
                .justify_between()
                .items_center()
                .border_b_1()
                .border_color(border)
                .child(Label::new(label).text_color(fg))
                .child(content)
        };

        // Helper to render form row with description
        let render_form_row_with_desc = |label: &'static str, description: &'static str, content: gpui::AnyElement, border: Hsla, fg: Hsla, muted: Hsla| {
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

        v_flex()
            .w_full()
            .max_h(px(400.))
            .overflow_y_scrollbar()
            // Description
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Pull an image from Docker Hub or another registry. Use the format: repository:tag or registry/repository:tag"),
            )
            // Image name row (required)
            .child(render_form_row(
                "Image",
                div().w(px(300.)).child(Input::new(&image_input).small()).into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Platform
            .child(render_form_row_with_desc(
                "Platform",
                "Target platform for the image",
                div().w(px(150.)).child(Select::new(&platform_select).small()).into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
    }
}
