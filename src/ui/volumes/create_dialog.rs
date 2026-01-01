use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, Hsla, ParentElement, Render,
    SharedString, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectItem, SelectState},
    theme::ActiveTheme,
    v_flex, IconName, IndexPath, Sizable,
};

/// Driver options for volume
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VolumeDriver {
    #[default]
    Local,
}

impl VolumeDriver {
    pub fn label(&self) -> &'static str {
        match self {
            VolumeDriver::Local => "local",
        }
    }

    pub fn as_docker_arg(&self) -> &'static str {
        match self {
            VolumeDriver::Local => "local",
        }
    }

    pub fn all() -> Vec<VolumeDriver> {
        vec![VolumeDriver::Local]
    }
}

impl SelectItem for VolumeDriver {
    type Value = VolumeDriver;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

/// Options for creating a new volume
#[derive(Debug, Clone, Default)]
pub struct CreateVolumeOptions {
    pub name: String,
    pub driver: VolumeDriver,
    pub labels: Vec<(String, String)>,
    pub driver_opts: Vec<(String, String)>,
}

/// Dialog for creating a new volume
pub struct CreateVolumeDialog {
    focus_handle: FocusHandle,

    // Input states
    name_input: Option<Entity<InputState>>,
    label_key_input: Option<Entity<InputState>>,
    label_value_input: Option<Entity<InputState>>,

    // Select states
    driver_select: Option<Entity<SelectState<Vec<VolumeDriver>>>>,

    // Labels list
    labels: Vec<(String, String)>,
}

impl CreateVolumeDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            name_input: None,
            label_key_input: None,
            label_value_input: None,
            driver_select: None,
            labels: Vec::new(),
        }
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.name_input.is_none() {
            self.name_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("Volume name (required)")
            }));
        }

        if self.label_key_input.is_none() {
            self.label_key_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Key")));
        }

        if self.label_value_input.is_none() {
            self.label_value_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Value")));
        }

        if self.driver_select.is_none() {
            self.driver_select = Some(cx.new(|cx| {
                SelectState::new(VolumeDriver::all(), Some(IndexPath::new(0)), window, cx)
            }));
        }
    }

    pub fn get_options(&self, cx: &App) -> CreateVolumeOptions {
        let name = self
            .name_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .unwrap_or_default();

        let driver = self
            .driver_select
            .as_ref()
            .and_then(|s| s.read(cx).selected_value().cloned())
            .unwrap_or_default();

        CreateVolumeOptions {
            name,
            driver,
            labels: self.labels.clone(),
            driver_opts: Vec::new(),
        }
    }
}

impl Focusable for CreateVolumeDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CreateVolumeDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);
        let colors = cx.theme().colors.clone();

        let name_input = self.name_input.clone().unwrap();
        let driver_select = self.driver_select.clone().unwrap();
        let label_key_input = self.label_key_input.clone();
        let label_value_input = self.label_value_input.clone();

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
                        .child(div().text_xs().text_color(muted).child(description)),
                )
                .child(content)
        };

        // Helper to render section header
        let render_section_header = |title: &'static str, bg: Hsla, muted: Hsla| {
            div()
                .w_full()
                .py(px(8.))
                .px(px(16.))
                .bg(bg)
                .child(div().text_xs().text_color(muted).child(title))
        };

        // Build labels section
        let labels_section = v_flex()
            .w_full()
            .gap(px(8.))
            .p(px(16.))
            // Add label row
            .child(
                h_flex()
                    .w_full()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&label_key_input.clone().unwrap()).small()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&label_value_input.clone().unwrap()).small()),
                    )
                    .child(
                        Button::new("add-label")
                            .label("Add")
                            .small()
                            .ghost()
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                if let (Some(key_input), Some(value_input)) =
                                    (&this.label_key_input, &this.label_value_input)
                                {
                                    let key = key_input.read(cx).text().to_string();
                                    let value = value_input.read(cx).text().to_string();
                                    if !key.is_empty() {
                                        this.labels.push((key, value));
                                        this.label_key_input = None;
                                        this.label_value_input = None;
                                        cx.notify();
                                    }
                                }
                            })),
                    ),
            )
            // Existing labels
            .children(self.labels.iter().enumerate().map(|(i, (key, value))| {
                let key = key.clone();
                let value = value.clone();
                h_flex()
                    .w_full()
                    .gap(px(8.))
                    .items_center()
                    .py(px(6.))
                    .px(px(8.))
                    .bg(colors.sidebar)
                    .rounded(px(4.))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(key),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.success)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(value),
                    )
                    .child(
                        Button::new(("remove-label", i))
                            .icon(IconName::Close)
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                if i < this.labels.len() {
                                    this.labels.remove(i);
                                    cx.notify();
                                }
                            })),
                    )
            }));

        v_flex()
            .w_full()
            .max_h(px(500.))
            .overflow_y_scrollbar()
            // Description
            .child(
                div()
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Volumes are for sharing data between containers. Unlike bind mounts, they are stored on a native Linux file system, making them faster and more reliable."),
            )
            // Name row (required)
            .child(render_form_row(
                "Name",
                div().w(px(300.)).child(Input::new(&name_input).small()).into_any_element(),
                colors.border,
                colors.foreground,
            ))
            // Driver
            .child(render_form_row_with_desc(
                "Driver",
                "Volume driver to use (--driver)",
                div().w(px(150.)).child(Select::new(&driver_select).small()).into_any_element(),
                colors.border,
                colors.foreground,
                colors.muted_foreground,
            ))
            // Labels section
            .child(render_section_header("Labels", colors.sidebar, colors.muted_foreground))
            .child(labels_section)
    }
}
