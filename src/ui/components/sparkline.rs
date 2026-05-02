//! Smooth area-style sparkline using `gpui::PathBuilder`.
//!
//! `gpui-component` ships a full `AreaChart` but it always paints axes /
//! grid lines, which is too much for the inline mini-charts we want on
//! the Stats tab and Activity Monitor. This element draws just the
//! stroke + filled area for a `Vec<f64>` series — no axes, fixed pad.

use gpui::{
  App, Background, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId,
  PathBuilder, Pixels, Point, Style, Window, point, px, relative,
};
use std::panic::Location;

#[derive(Clone)]
pub struct Sparkline {
  data: Vec<f64>,
  stroke: Hsla,
  fill: Hsla,
  stroke_width: Pixels,
  /// Optional explicit max — useful for percent series so the chart
  /// scales to 0..=100 instead of the data max.
  max_override: Option<f64>,
}

impl Sparkline {
  pub fn new(data: Vec<f64>, stroke: Hsla) -> Self {
    Self {
      data,
      stroke,
      fill: Hsla {
        a: 0.25,
        ..stroke
      },
      stroke_width: px(1.5),
      max_override: None,
    }
  }

  pub fn max(mut self, max: f64) -> Self {
    self.max_override = Some(max);
    self
  }
}

impl IntoElement for Sparkline {
  type Element = Self;
  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for Sparkline {
  type RequestLayoutState = ();
  type PrepaintState = ();

  fn id(&self) -> Option<ElementId> {
    None
  }
  fn source_location(&self) -> Option<&'static Location<'static>> {
    None
  }

  fn request_layout(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    window: &mut Window,
    cx: &mut App,
  ) -> (LayoutId, ()) {
    let mut style = Style::default();
    style.size.width = relative(1.).into();
    style.size.height = relative(1.).into();
    let layout_id = window.request_layout(style, [], cx);
    (layout_id, ())
  }

  fn prepaint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    _bounds: Bounds<Pixels>,
    _request_layout: &mut (),
    _window: &mut Window,
    _cx: &mut App,
  ) {
  }

  fn paint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    _request_layout: &mut (),
    _prepaint_state: &mut (),
    window: &mut Window,
    _cx: &mut App,
  ) {
    if self.data.len() < 2 {
      return;
    }
    let count = self.data.len();
    let max = self
      .max_override
      .unwrap_or_else(|| self.data.iter().copied().fold(0.0_f64, f64::max).max(1e-6));
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let pad = 2.0_f32;
    let plot_h = (height - pad * 2.0).max(1.0);
    let plot_w = (width - pad).max(1.0);
    let origin_x = f32::from(bounds.origin.x) + pad / 2.0;
    let origin_y = f32::from(bounds.origin.y) + pad;
    let baseline_y = origin_y + plot_h;

    #[allow(clippy::cast_precision_loss)]
    let step = if count > 1 { plot_w / (count - 1) as f32 } else { 0.0 };

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let pt = |i: usize, v: f64| -> Point<Pixels> {
      let x = origin_x + step * i as f32;
      let normalized = (v / max).clamp(0.0, 1.0) as f32;
      let y = origin_y + plot_h * (1.0 - normalized);
      point(px(x), px(y))
    };

    // Filled area: closed path (data line + back along baseline).
    let mut fill_builder = PathBuilder::fill();
    fill_builder.move_to(point(px(origin_x), px(baseline_y)));
    for (i, &v) in self.data.iter().enumerate() {
      fill_builder.line_to(pt(i, v));
    }
    #[allow(clippy::cast_precision_loss)]
    let last_x = origin_x + step * count.saturating_sub(1) as f32;
    fill_builder.line_to(point(px(last_x), px(baseline_y)));
    fill_builder.line_to(point(px(origin_x), px(baseline_y)));
    if let Ok(path) = fill_builder.build() {
      window.paint_path(path, Background::from(self.fill));
    }

    // Stroke line on top.
    let mut stroke_builder = PathBuilder::stroke(self.stroke_width);
    stroke_builder.move_to(pt(0, self.data[0]));
    for (i, &v) in self.data.iter().enumerate().skip(1) {
      stroke_builder.line_to(pt(i, v));
    }
    if let Ok(path) = stroke_builder.build() {
      window.paint_path(path, Background::from(self.stroke));
    }
  }
}
