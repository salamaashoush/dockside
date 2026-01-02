use gpui::{
    div, prelude::*, px, App, AppContext, Context, Entity, FocusHandle, Focusable, Render,
    SharedString, Styled, Window,
};
use gpui_component::{
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectItem, SelectState},
    theme::{ActiveTheme, Theme, ThemeRegistry},
    v_flex, IndexPath, Sizable,
};

use crate::state::{settings_state, SettingsChanged, SettingsState, ThemeName};

/// Theme wrapper for Select
#[derive(Debug, Clone)]
struct ThemeOption {
    theme: ThemeName,
    label: SharedString,
}

impl ThemeOption {
    fn new(theme: ThemeName) -> Self {
        let label = SharedString::from(theme.display_name().to_string());
        Self { theme, label }
    }

    fn all() -> Vec<Self> {
        ThemeName::all().into_iter().map(|t| Self::new(t)).collect()
    }
}

impl SelectItem for ThemeOption {
    type Value = ThemeOption;

    fn value(&self) -> &Self::Value {
        self
    }

    fn title(&self) -> SharedString {
        self.label.clone()
    }
}

/// Settings dialog
pub struct SettingsDialog {
    focus_handle: FocusHandle,
    settings_state: Entity<SettingsState>,
    // Form state
    theme_select: Entity<SelectState<Vec<ThemeOption>>>,
    docker_socket_input: Entity<InputState>,
    colima_profile_input: Entity<InputState>,
    container_refresh_input: Entity<InputState>,
    stats_refresh_input: Entity<InputState>,
    log_lines_input: Entity<InputState>,
    font_size_input: Entity<InputState>,
}

impl SettingsDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let settings_state = settings_state(cx);
        let settings = settings_state.read(cx).settings.clone();

        // Find the current theme index
        let themes = ThemeOption::all();
        let current_theme_idx = themes
            .iter()
            .position(|t| t.theme == settings.theme)
            .unwrap_or(0);

        let theme_select = cx.new(|cx| {
            SelectState::new(themes, Some(IndexPath::new(current_theme_idx)), window, cx)
        });

        // Initialize input fields
        let docker_socket_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Default socket")
                .default_value(&settings.docker_socket)
        });

        let colima_profile_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("default")
                .default_value(&settings.default_colima_profile)
        });

        let container_refresh_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&settings.container_refresh_interval.to_string())
        });

        let stats_refresh_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&settings.stats_refresh_interval.to_string())
        });

        let log_lines_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&settings.max_log_lines.to_string())
        });

        let font_size_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&settings.terminal_font_size.to_string())
        });

        Self {
            focus_handle,
            settings_state,
            theme_select,
            docker_socket_input,
            colima_profile_input,
            container_refresh_input,
            stats_refresh_input,
            log_lines_input,
            font_size_input,
        }
    }

    pub fn apply_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Get current values from inputs
        let docker_socket = self.docker_socket_input.read(cx).text().to_string();
        let colima_profile = self.colima_profile_input.read(cx).text().to_string();
        let container_refresh = self.container_refresh_input.read(cx).text().to_string()
            .parse::<u64>()
            .unwrap_or(5);
        let stats_refresh = self.stats_refresh_input.read(cx).text().to_string()
            .parse::<u64>()
            .unwrap_or(2);
        let log_lines = self.log_lines_input.read(cx).text().to_string()
            .parse::<usize>()
            .unwrap_or(1000);
        let font_size = self.font_size_input.read(cx).text().to_string()
            .parse::<f32>()
            .unwrap_or(14.0);

        // Get selected theme
        let theme = self.theme_select.read(cx)
            .selected_value()
            .map(|opt| opt.theme.clone())
            .unwrap_or_default();

        // Apply the theme immediately using ThemeRegistry
        let theme_name = SharedString::from(theme.theme_name().to_string());
        if let Some(theme_config) = ThemeRegistry::global(cx)
            .themes()
            .get(&theme_name)
            .cloned()
        {
            Theme::global_mut(cx).apply_config(&theme_config);
        }

        // Update settings state
        self.settings_state.update(cx, |state, cx| {
            let old_theme = state.settings.theme.clone();
            state.settings.docker_socket = docker_socket;
            state.settings.default_colima_profile = colima_profile;
            state.settings.container_refresh_interval = container_refresh;
            state.settings.stats_refresh_interval = stats_refresh;
            state.settings.max_log_lines = log_lines;
            state.settings.terminal_font_size = font_size;
            state.settings.theme = theme.clone();
            let _ = state.settings.save();

            if old_theme != theme {
                cx.emit(SettingsChanged::ThemeChanged(theme.clone()));
            }
            cx.emit(SettingsChanged::SettingsUpdated);
        });

        cx.notify();
    }

    fn render_section_header(&self, title: &str, cx: &Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        div()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .mt(px(8.))
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.muted_foreground)
            .child(title.to_uppercase())
    }

    fn render_form_row(&self, label: &'static str, description: &'static str, content: impl IntoElement, cx: &Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        h_flex()
            .w_full()
            .py(px(10.))
            .px(px(16.))
            .justify_between()
            .items_center()
            .gap(px(16.))
            .border_b_1()
            .border_color(colors.border)
            .child(
                v_flex()
                    .flex_1()
                    .gap(px(2.))
                    .child(Label::new(label).text_color(colors.foreground))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(description),
                    ),
            )
            .child(
                div().w(px(200.)).child(content)
            )
    }
}

impl Focusable for SettingsDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .max_h(px(550.))
            .overflow_y_scrollbar()
            // Appearance section
            .child(self.render_section_header("Appearance", cx))
            .child(self.render_form_row(
                "Theme",
                "Choose the color theme for the application",
                Select::new(&self.theme_select).w_full().small(),
                cx,
            ))
            // Docker section
            .child(self.render_section_header("Docker", cx))
            .child(self.render_form_row(
                "Docker Socket",
                "Path to Docker socket (leave empty for default)",
                Input::new(&self.docker_socket_input).small().w_full(),
                cx,
            ))
            .child(self.render_form_row(
                "Default Colima Profile",
                "Name of the default Colima VM profile",
                Input::new(&self.colima_profile_input).small().w_full(),
                cx,
            ))
            // Refresh intervals section
            .child(self.render_section_header("Refresh Intervals", cx))
            .child(self.render_form_row(
                "Container Refresh",
                "How often to refresh container list (seconds)",
                Input::new(&self.container_refresh_input).small().w_full(),
                cx,
            ))
            .child(self.render_form_row(
                "Stats Refresh",
                "How often to refresh stats (seconds)",
                Input::new(&self.stats_refresh_input).small().w_full(),
                cx,
            ))
            // Display section
            .child(self.render_section_header("Display", cx))
            .child(self.render_form_row(
                "Max Log Lines",
                "Maximum number of log lines to display",
                Input::new(&self.log_lines_input).small().w_full(),
                cx,
            ))
            .child(self.render_form_row(
                "Terminal Font Size",
                "Font size for terminal views (pixels)",
                Input::new(&self.font_size_input).small().w_full(),
                cx,
            ))
    }
}
