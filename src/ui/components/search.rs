use gpui::{div, prelude::*, px, Context, Entity, Render, Styled, Window};
use gpui_component::{
    h_flex,
    input::{Input, InputState},
    theme::ActiveTheme,
    Icon, IconName, Sizable,
};

/// A search bar component for filtering lists
pub struct SearchBar {
    input_state: Entity<InputState>,
    placeholder: &'static str,
}

impl SearchBar {
    pub fn new(placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(placeholder)
        });

        Self {
            input_state,
            placeholder,
        }
    }

    pub fn query(&self, cx: &gpui::App) -> String {
        self.input_state.read(cx).text().to_string()
    }

    pub fn is_empty(&self, cx: &gpui::App) -> bool {
        self.input_state.read(cx).text().to_string().is_empty()
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Recreate the input state to clear it
        self.input_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder(self.placeholder)
        });
        cx.notify();
    }
}

impl Render for SearchBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        let query = self.input_state.read(cx).text().to_string();

        h_flex()
            .w_full()
            .h(px(36.))
            .px(px(12.))
            .gap(px(8.))
            .items_center()
            .bg(colors.sidebar)
            .border_b_1()
            .border_color(colors.border)
            .child(
                Icon::new(IconName::Search)
                    .size(px(16.))
                    .text_color(colors.muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&self.input_state).small().w_full()),
            )
            .when(!query.is_empty(), |el| {
                el.child(
                    div()
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .bg(colors.muted_foreground.opacity(0.2))
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(format!("{} chars", query.len())),
                )
            })
    }
}

/// Trait for items that can be searched
pub trait Searchable {
    fn matches_query(&self, query: &str) -> bool;
}
