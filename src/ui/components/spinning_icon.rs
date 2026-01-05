//! Spinning icon component for loading indicators
//!
//! Provides an animated spinning icon for use in loading states.

use std::time::Duration;

use gpui::{Animation, AnimationExt, IntoElement, Pixels, Styled, Transformation, percentage};
use gpui_component::{Icon, IconName};

/// Create a spinning loader icon with animation
pub fn spinning_loader(size: Pixels, color: gpui::Hsla) -> impl IntoElement {
  Icon::new(IconName::Loader).size(size).text_color(color).with_animation(
    "spin",
    Animation::new(Duration::from_millis(1000)).repeat(),
    |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
  )
}

/// Create a spinning loader circle icon with animation (alternative style)
pub fn spinning_loader_circle(size: Pixels, color: gpui::Hsla) -> impl IntoElement {
  Icon::new(IconName::LoaderCircle)
    .size(size)
    .text_color(color)
    .with_animation(
      "spin-circle",
      Animation::new(Duration::from_millis(1000)).repeat(),
      |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
    )
}
