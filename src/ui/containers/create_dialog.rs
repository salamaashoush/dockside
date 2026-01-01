use gpui::{
    div, prelude::*, px, rgb, App, Context, Entity, FocusHandle, Focusable,
    ParentElement, Render, SharedString, Styled, Window,
};
use gpui_component::{
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectItem, SelectState},
    switch::Switch,
    v_flex, IndexPath, Sizable,
};

/// Platform options for container
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Platform {
    #[default]
    Auto,
    LinuxAmd64,
    LinuxArm64,
    LinuxArmV7,
}

impl Platform {
    pub fn label(&self) -> &'static str {
        match self {
            Platform::Auto => "auto",
            Platform::LinuxAmd64 => "linux/amd64",
            Platform::LinuxArm64 => "linux/arm64",
            Platform::LinuxArmV7 => "linux/arm/v7",
        }
    }

    pub fn as_docker_arg(&self) -> Option<&'static str> {
        match self {
            Platform::Auto => None,
            Platform::LinuxAmd64 => Some("linux/amd64"),
            Platform::LinuxArm64 => Some("linux/arm64"),
            Platform::LinuxArmV7 => Some("linux/arm/v7"),
        }
    }

    pub fn all() -> Vec<Platform> {
        vec![
            Platform::Auto,
            Platform::LinuxAmd64,
            Platform::LinuxArm64,
            Platform::LinuxArmV7,
        ]
    }
}

impl SelectItem for Platform {
    type Value = Platform;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

/// Restart policy options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestartPolicy {
    #[default]
    No,
    Always,
    OnFailure,
    UnlessStopped,
}

impl RestartPolicy {
    pub fn label(&self) -> &'static str {
        match self {
            RestartPolicy::No => "no",
            RestartPolicy::Always => "always",
            RestartPolicy::OnFailure => "on-failure",
            RestartPolicy::UnlessStopped => "unless-stopped",
        }
    }

    pub fn as_docker_arg(&self) -> Option<&'static str> {
        match self {
            RestartPolicy::No => None,
            RestartPolicy::Always => Some("always"),
            RestartPolicy::OnFailure => Some("on-failure"),
            RestartPolicy::UnlessStopped => Some("unless-stopped"),
        }
    }

    pub fn all() -> Vec<RestartPolicy> {
        vec![
            RestartPolicy::No,
            RestartPolicy::Always,
            RestartPolicy::OnFailure,
            RestartPolicy::UnlessStopped,
        ]
    }
}

impl SelectItem for RestartPolicy {
    type Value = RestartPolicy;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self::Value {
        self
    }
}

/// Options for creating a new container
#[derive(Debug, Clone, Default)]
pub struct CreateContainerOptions {
    pub image: String,
    pub platform: Platform,
    pub name: Option<String>,
    pub remove_after_stop: bool,
    pub restart_policy: RestartPolicy,
    pub command: Option<String>,
    pub entrypoint: Option<String>,
    pub workdir: Option<String>,
    pub privileged: bool,
    pub read_only: bool,
    pub docker_init: bool,
    pub start_after_create: bool,
}

impl CreateContainerOptions {
    pub fn to_docker_args(&self) -> Vec<String> {
        let mut args = vec!["run".to_string()];

        // Always run detached
        args.push("-d".to_string());

        // Platform
        if let Some(platform) = self.platform.as_docker_arg() {
            args.push("--platform".to_string());
            args.push(platform.to_string());
        }

        if let Some(ref name) = self.name {
            if !name.is_empty() {
                args.push("--name".to_string());
                args.push(name.clone());
            }
        }

        if self.remove_after_stop {
            args.push("--rm".to_string());
        }

        if let Some(restart) = self.restart_policy.as_docker_arg() {
            args.push("--restart".to_string());
            args.push(restart.to_string());
        }

        if let Some(ref entrypoint) = self.entrypoint {
            if !entrypoint.is_empty() {
                args.push("--entrypoint".to_string());
                args.push(entrypoint.clone());
            }
        }

        if let Some(ref workdir) = self.workdir {
            if !workdir.is_empty() {
                args.push("--workdir".to_string());
                args.push(workdir.clone());
            }
        }

        if self.privileged {
            args.push("--privileged".to_string());
        }

        if self.read_only {
            args.push("--read-only".to_string());
        }

        if self.docker_init {
            args.push("--init".to_string());
        }

        args.push(self.image.clone());

        if let Some(ref cmd) = self.command {
            if !cmd.is_empty() {
                // Split command by whitespace for docker
                args.extend(cmd.split_whitespace().map(String::from));
            }
        }

        args
    }
}

/// Dialog for creating a new container
pub struct CreateContainerDialog {
    focus_handle: FocusHandle,

    // Input states - created lazily
    image_input: Option<Entity<InputState>>,
    name_input: Option<Entity<InputState>>,
    command_input: Option<Entity<InputState>>,
    entrypoint_input: Option<Entity<InputState>>,
    workdir_input: Option<Entity<InputState>>,

    // Select states
    platform_select: Option<Entity<SelectState<Vec<Platform>>>>,
    restart_policy_select: Option<Entity<SelectState<Vec<RestartPolicy>>>>,

    // Toggle state
    remove_after_stop: bool,
    privileged: bool,
    read_only: bool,
    docker_init: bool,
}

impl CreateContainerDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            image_input: None,
            name_input: None,
            command_input: None,
            entrypoint_input: None,
            workdir_input: None,
            platform_select: None,
            restart_policy_select: None,
            remove_after_stop: false,
            privileged: false,
            read_only: false,
            docker_init: false,
        }
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.image_input.is_none() {
            self.image_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("e.g. nginx:latest")
            }));
        }

        if self.name_input.is_none() {
            self.name_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("Container name (optional)")
            }));
        }

        if self.command_input.is_none() {
            self.command_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("Command (optional)")
            }));
        }

        if self.entrypoint_input.is_none() {
            self.entrypoint_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("Entrypoint (optional)")
            }));
        }

        if self.workdir_input.is_none() {
            self.workdir_input = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("Working directory (optional)")
            }));
        }

        if self.platform_select.is_none() {
            self.platform_select = Some(cx.new(|cx| {
                SelectState::new(Platform::all(), Some(IndexPath::new(0)), window, cx)
            }));
        }

        if self.restart_policy_select.is_none() {
            self.restart_policy_select = Some(cx.new(|cx| {
                SelectState::new(RestartPolicy::all(), Some(IndexPath::new(0)), window, cx)
            }));
        }
    }

    pub fn get_options(&self, cx: &App, start_after_create: bool) -> CreateContainerOptions {
        let image = self
            .image_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .unwrap_or_default();

        let name = self
            .name_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .filter(|s| !s.is_empty());

        let command = self
            .command_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .filter(|s| !s.is_empty());

        let entrypoint = self
            .entrypoint_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .filter(|s| !s.is_empty());

        let workdir = self
            .workdir_input
            .as_ref()
            .map(|s| s.read(cx).text().to_string())
            .filter(|s| !s.is_empty());

        let platform = self
            .platform_select
            .as_ref()
            .and_then(|s| s.read(cx).selected_value().cloned())
            .unwrap_or_default();

        let restart_policy = self
            .restart_policy_select
            .as_ref()
            .and_then(|s| s.read(cx).selected_value().cloned())
            .unwrap_or_default();

        CreateContainerOptions {
            image,
            platform,
            name,
            remove_after_stop: self.remove_after_stop,
            restart_policy,
            command,
            entrypoint,
            workdir,
            privileged: self.privileged,
            read_only: self.read_only,
            docker_init: self.docker_init,
            start_after_create,
        }
    }

    fn render_form_row(&self, label: &'static str, content: impl IntoElement) -> gpui::Div {
        h_flex()
            .w_full()
            .py(px(12.))
            .px(px(16.))
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(rgb(0x414868))
            .child(Label::new(label).text_color(rgb(0xa9b1d6)))
            .child(content)
    }

    fn render_form_row_with_desc(
        &self,
        label: &'static str,
        description: &'static str,
        content: impl IntoElement,
    ) -> gpui::Div {
        h_flex()
            .w_full()
            .py(px(12.))
            .px(px(16.))
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(rgb(0x414868))
            .child(
                v_flex()
                    .gap(px(2.))
                    .child(Label::new(label).text_color(rgb(0xa9b1d6)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x565f89))
                            .child(description),
                    ),
            )
            .child(content)
    }

    fn render_section_header(&self, title: &'static str) -> gpui::Div {
        div()
            .w_full()
            .py(px(8.))
            .px(px(16.))
            .bg(rgb(0x1a1b26))
            .child(div().text_xs().text_color(rgb(0x565f89)).child(title))
    }
}

impl Focusable for CreateContainerDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CreateContainerDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);

        let remove_after_stop = self.remove_after_stop;
        let privileged = self.privileged;
        let read_only = self.read_only;
        let docker_init = self.docker_init;

        let image_input = self.image_input.clone().unwrap();
        let name_input = self.name_input.clone().unwrap();
        let command_input = self.command_input.clone().unwrap();
        let entrypoint_input = self.entrypoint_input.clone().unwrap();
        let workdir_input = self.workdir_input.clone().unwrap();
        let platform_select = self.platform_select.clone().unwrap();
        let restart_policy_select = self.restart_policy_select.clone().unwrap();

        v_flex()
            .w_full()
            .max_h(px(600.))
            .overflow_y_scrollbar()
            // Image row (required)
            .child(self.render_form_row(
                "Image",
                div().w(px(250.)).child(Input::new(&image_input).small()),
            ))
            // Name row
            .child(self.render_form_row(
                "Name",
                div().w(px(250.)).child(Input::new(&name_input).small()),
            ))
            // Platform
            .child(self.render_form_row_with_desc(
                "Platform",
                "Target platform for the container. (--platform)",
                div().w(px(150.)).child(Select::new(&platform_select).small()),
            ))
            // Remove after stop
            .child(self.render_form_row_with_desc(
                "Remove after stop",
                "Automatically delete the container after it stops. (--rm)",
                Switch::new("remove-after-stop")
                    .checked(remove_after_stop)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.remove_after_stop = *checked;
                        cx.notify();
                    })),
            ))
            // Restart policy
            .child(self.render_form_row_with_desc(
                "Restart policy",
                "Whether to restart the container automatically. (--restart)",
                div().w(px(150.)).child(Select::new(&restart_policy_select).small()),
            ))
            // Payload section
            .child(self.render_section_header("Payload"))
            .child(self.render_form_row_with_desc(
                "Command",
                "Command to run in the container",
                div().w(px(250.)).child(Input::new(&command_input).small()),
            ))
            .child(self.render_form_row_with_desc(
                "Entrypoint",
                "Override the default entrypoint. (--entrypoint)",
                div()
                    .w(px(250.))
                    .child(Input::new(&entrypoint_input).small()),
            ))
            .child(self.render_form_row_with_desc(
                "Working directory",
                "Working directory for the command. (--workdir)",
                div().w(px(250.)).child(Input::new(&workdir_input).small()),
            ))
            // Advanced section
            .child(self.render_section_header("Advanced"))
            .child(self.render_form_row_with_desc(
                "Privileged",
                "Allow access to privileged APIs and resources. (--privileged)",
                Switch::new("privileged")
                    .checked(privileged)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.privileged = *checked;
                        cx.notify();
                    })),
            ))
            .child(self.render_form_row_with_desc(
                "Read-only",
                "Mount the container's root filesystem as read-only. (--read-only)",
                Switch::new("read-only")
                    .checked(read_only)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.read_only = *checked;
                        cx.notify();
                    })),
            ))
            .child(self.render_form_row_with_desc(
                "Use docker-init",
                "Run the container payload under a docker-init process. (--init)",
                Switch::new("docker-init")
                    .checked(docker_init)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.docker_init = *checked;
                        cx.notify();
                    })),
            ))
    }
}
