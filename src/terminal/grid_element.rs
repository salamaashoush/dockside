//! Custom GPUI element for the terminal grid.
//!
//! Renders a `TerminalContent` snapshot using:
//! - One `paint_quad` for the whole bounds (default bg)
//! - Merged background quads per run of same-colored adjacent cells (with
//!   `floor()` on x and `ceil()` on width so adjacent rects tile seamlessly)
//! - One `shape_line` per same-style text run, with `force_width = cell_width`
//!   so glyphs always land on the monospace grid regardless of the font's
//!   actual advance widths
//! - The cursor as a separate overlay quad after the grid pass (or, for the
//!   block style, as an inverted-color text run during the grid pass so the
//!   character on the cursor is still readable).
//!
//! This pattern is lifted from Zed's `TerminalElement`
//! (zed-industries/zed::crates/terminal_view/src/terminal_element.rs).

use std::panic;

use gpui::{
  App, Bounds, Element, ElementId, Font, FontFeatures, FontStyle, FontWeight, GlobalElementId, Hsla,
  InspectorElementId, IntoElement, LayoutId, Pixels, Point, SharedString, Size, Style, TextRun, Window, fill, hsla,
  point, px, relative, size,
};

use super::TerminalContent;
use crate::state::TerminalCursorStyle;

/// Primary monospace family used for measurement and paint. Single family,
/// no fallbacks — the shaper must produce stable advances across cells.
const PRIMARY_FAMILY: &str = "DejaVu Sans Mono";

/// Cell-grid coordinate (column, row) inside the rendered viewport.
pub type GridPoint = (u16, u16);

/// Normalized selection range in viewport cell coords. `start` precedes
/// `end` in reading order (smaller row, or same row with smaller col).
#[derive(Debug, Clone, Copy)]
pub struct GridSelection {
  pub start: GridPoint,
  pub end: GridPoint,
}

/// Live cell metrics published by [`TerminalGrid::prepaint`] each frame so
/// the parent view can convert window-space mouse coords back into cell
/// coords using the same metrics the grid actually painted with.
#[derive(Debug, Clone, Copy)]
pub struct GridMetrics {
  pub origin: Point<Pixels>,
  pub cell_width: Pixels,
  pub line_height: Pixels,
  pub cols: u16,
  pub rows: u16,
}

/// Render a `TerminalContent` snapshot inside its parent's bounds.
pub struct TerminalGrid {
  pub content: TerminalContent,
  pub font_size: Pixels,
  pub line_height_factor: f32,
  pub default_fg: Hsla,
  pub default_bg: Hsla,
  pub cursor_color: Hsla,
  pub cursor_style: TerminalCursorStyle,
  /// Optional callback invoked from `prepaint` when the grid's cell-derived
  /// `(cols, rows)` differ from the last reported size. Lets the element
  /// drive PTY resize directly off real container bounds.
  pub on_resize: Option<std::sync::Arc<dyn Fn(u16, u16) + Send + Sync + 'static>>,
  /// Last-known reported size (used to dedupe resize callbacks).
  pub last_reported_size: Option<(u16, u16)>,
  /// Active text selection (already normalized).
  pub selection: Option<GridSelection>,
  /// Selection highlight color (translucent overlay).
  pub selection_color: Hsla,
  /// Per-cell-range search match highlights (row, `col_start`, `col_end` inclusive).
  pub search_matches: Vec<(u16, u16, u16)>,
  /// Highlight color for search matches.
  pub search_match_color: Hsla,
  /// Out-param: the metrics used this frame, written in `prepaint`.
  pub metrics_sink: Option<std::sync::Arc<std::sync::Mutex<Option<GridMetrics>>>>,
}

/// Computed layout state for a single frame.
pub struct GridLayout {
  #[allow(dead_code)]
  cell_width: Pixels,
  line_height: Pixels,
  origin: Point<Pixels>,
  bounds: Bounds<Pixels>,
  bg_runs: Vec<BgRun>,
  text_runs: Vec<TextRunSpec>,
  cursor_overlay: Option<CursorOverlay>,
  selection_rects: Vec<Bounds<Pixels>>,
  selection_color: Hsla,
  search_rects: Vec<Bounds<Pixels>>,
  search_color: Hsla,
}

struct BgRun {
  bounds: Bounds<Pixels>,
  color: Hsla,
}

struct TextRunSpec {
  origin: Point<Pixels>,
  text: SharedString,
  run: TextRun,
}

struct CursorOverlay {
  bounds: Bounds<Pixels>,
  kind: TerminalCursorStyle,
  color: Hsla,
}

#[derive(Debug, Clone, PartialEq)]
struct CellAttrs {
  fg: Hsla,
  bold: bool,
  italic: bool,
  underline: bool,
  strikethrough: bool,
}

impl IntoElement for TerminalGrid {
  type Element = Self;
  fn into_element(self) -> Self {
    self
  }
}

impl Element for TerminalGrid {
  type RequestLayoutState = ();
  type PrepaintState = GridLayout;

  fn id(&self) -> Option<ElementId> {
    None
  }

  fn source_location(&self) -> Option<&'static panic::Location<'static>> {
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
    bounds: Bounds<Pixels>,
    _request_layout: &mut (),
    window: &mut Window,
    _cx: &mut App,
  ) -> GridLayout {
    // Measurement font: single deterministic family with NO fallbacks. The
    // shaper would otherwise swap to a different fallback per glyph, and the
    // measured advance would no longer match the per-cell paint advance.
    // Pattern lifted from superhq-ai/superhq's `gpui-terminal` crate
    // (`crates/gpui-terminal/src/render.rs:262-293`).
    let measure_font = Font {
      family: PRIMARY_FAMILY.into(),
      features: FontFeatures::default(),
      fallbacks: None,
      weight: FontWeight::NORMAL,
      style: FontStyle::Normal,
    };
    let probe_run = TextRun {
      len: "│".len(),
      font: measure_font,
      color: hsla(0.0, 0.0, 0.5, 1.0),
      background_color: None,
      underline: None,
      strikethrough: None,
    };
    let probe = window
      .text_system()
      .shape_line(SharedString::from("│"), self.font_size, &[probe_run], None);
    let cell_width = probe.width;
    let line_height = (probe.ascent + probe.descent).ceil() * self.line_height_factor;

    // Fire a PTY resize when the bounds resolve to a different (cols, rows).
    // Done in prepaint because that's the first place we know the actual
    // container size and the real cell metrics.
    if let Some(cb) = &self.on_resize
      && f32::from(cell_width) > 0.0
      && f32::from(line_height) > 0.0
    {
      #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let cols = (f32::from(bounds.size.width) / f32::from(cell_width)).floor() as u16;
      #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let rows = (f32::from(bounds.size.height) / f32::from(line_height)).floor() as u16;
      let cols = cols.clamp(20, 1000);
      let rows = rows.clamp(5, 1000);
      if self.last_reported_size != Some((cols, rows)) {
        cb(cols, rows);
        self.last_reported_size = Some((cols, rows));
      }
    }

    // Snap origin to device pixels so the grid doesn't shimmer when the
    // window is dragged between subpixel positions.
    let scale = window.scale_factor();
    let origin = Point::new(
      px((f32::from(bounds.origin.x) * scale).floor() / scale),
      px((f32::from(bounds.origin.y) * scale).floor() / scale),
    );

    // Publish live metrics so the parent view can convert mouse coords
    // back to cell coords using the same numbers we paint with.
    if let Some(sink) = &self.metrics_sink {
      #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let cols = if f32::from(cell_width) > 0.0 {
        (f32::from(bounds.size.width) / f32::from(cell_width)).floor() as u16
      } else {
        0
      };
      let rows_u16 = u16::try_from(self.content.lines.len()).unwrap_or(u16::MAX);
      if let Ok(mut g) = sink.lock() {
        *g = Some(GridMetrics {
          origin,
          cell_width,
          line_height,
          cols,
          rows: rows_u16,
        });
      }
    }

    let bg_default = self.default_bg;
    let fg_default = self.default_fg;
    let mut bg_runs: Vec<BgRun> = Vec::new();
    let mut text_runs: Vec<TextRunSpec> = Vec::new();
    let mut cursor_overlay: Option<CursorOverlay> = None;

    for (row_idx, line) in self.content.lines.iter().enumerate() {
      #[allow(clippy::cast_precision_loss)]
      let row_y = origin.y + line_height * (row_idx as f32);
      let cursor_in_row = self.content.cursor_visible && row_idx == self.content.cursor_row;
      let cursor_col = self.content.cursor_col;

      // ----- Background pass: merge same-color neighbors. -----
      let mut bg_run: Option<(usize, usize, Hsla)> = None; // (start, end, color)
      for (col, cell) in line.cells.iter().enumerate() {
        let mut bg = cell.bg.map_or(bg_default, rgb_to_hsla);
        // Cursor block paints the cursor cell with the cursor color so the
        // glyph below stays readable. Apply on the bg pass only.
        if cursor_in_row && col == cursor_col && matches!(self.cursor_style, TerminalCursorStyle::Block) {
          bg = self.cursor_color;
        }
        let drop = colors_match(bg, bg_default) && !(cursor_in_row && col == cursor_col);

        match (&mut bg_run, drop) {
          (Some(_), true) => {
            let (start, end, color) = bg_run.take().unwrap();
            bg_runs.push(make_bg(origin, row_y, start, end, color, cell_width, line_height));
          }
          (Some((_, end, color)), false) if colors_match(*color, bg) && *end + 1 == col => {
            *end = col;
          }
          (Some(_), false) => {
            let (start, end, color) = bg_run.take().unwrap();
            bg_runs.push(make_bg(origin, row_y, start, end, color, cell_width, line_height));
            bg_run = Some((col, col, bg));
          }
          (None, true) => {}
          (None, false) => {
            bg_run = Some((col, col, bg));
          }
        }
      }
      if let Some((start, end, color)) = bg_run.take() {
        bg_runs.push(make_bg(origin, row_y, start, end, color, cell_width, line_height));
      }

      // ----- Text pass: ONE shape_line per cell, painted at an explicit
      // x = origin.x + cell_width * col. Following superhq-ai/superhq's
      // gpui-terminal pattern (`crates/gpui-terminal/src/render.rs:686-754`).
      // Per-cell painting trades batch perf for pixel-exact column placement
      // — `force_width` snapping in shape_line drifts under sub-pixel
      // mismatches and produces phantom letter-spacing.
      for (col, cell) in line.cells.iter().enumerate() {
        if cell.char == '\0' || cell.char == ' ' {
          continue;
        }

        let is_block_cursor =
          cursor_in_row && col == cursor_col && matches!(self.cursor_style, TerminalCursorStyle::Block);

        let mut fg = cell.fg.map_or(fg_default, rgb_to_hsla);
        if cell.dim {
          fg = hsla(fg.h, fg.s * 0.7, fg.l * 0.7, fg.a);
        }
        if is_block_cursor {
          fg = bg_default;
        }
        let _ = fg_default; // available for future palette-default handling

        let attrs = CellAttrs {
          fg,
          bold: cell.bold,
          italic: cell.italic,
          underline: cell.underline,
          strikethrough: cell.strikethrough,
        };

        let mut buf = [0u8; 4];
        let ch_str = cell.char.encode_utf8(&mut buf);
        text_runs.push(make_text_run(origin, row_y, col, ch_str, &attrs, cell_width));
      }

      // ----- Cursor overlay (Bar / Underline). Block was painted inline. -----
      if cursor_in_row && !matches!(self.cursor_style, TerminalCursorStyle::Block) {
        #[allow(clippy::cast_precision_loss)]
        let cx_px = origin.x + cell_width * (cursor_col as f32);
        let cell_bounds = Bounds::new(Point::new(cx_px, row_y), Size::new(cell_width, line_height));
        cursor_overlay = Some(CursorOverlay {
          bounds: cell_bounds,
          kind: self.cursor_style,
          color: self.cursor_color,
        });
      }
    }

    // Selection rects: one bounds per row spanned, partial on first/last.
    let mut selection_rects: Vec<Bounds<Pixels>> = Vec::new();
    if let Some(sel) = self.selection {
      let total_rows = self.content.lines.len();
      let total_cols = self
        .content
        .lines
        .first()
        .map_or(0usize, |l| l.cells.len());
      if total_rows > 0 && total_cols > 0 {
        let last_row = u16::try_from(total_rows - 1).unwrap_or(u16::MAX);
        let last_col = u16::try_from(total_cols - 1).unwrap_or(u16::MAX);
        let s_row = sel.start.1.min(last_row);
        let e_row = sel.end.1.min(last_row);
        for row in s_row..=e_row {
          let (c0, c1) = if s_row == e_row {
            (sel.start.0.min(last_col), sel.end.0.min(last_col))
          } else if row == s_row {
            (sel.start.0.min(last_col), last_col)
          } else if row == e_row {
            (0u16, sel.end.0.min(last_col))
          } else {
            (0u16, last_col)
          };
          if c0 > c1 {
            continue;
          }
          #[allow(clippy::cast_precision_loss)]
          let x = f32::from(origin.x) + f32::from(cell_width) * f32::from(c0);
          #[allow(clippy::cast_precision_loss)]
          let span = f32::from(c1 - c0 + 1);
          let w = f32::from(cell_width) * span;
          #[allow(clippy::cast_precision_loss)]
          let y = origin.y + line_height * f32::from(row);
          selection_rects.push(Bounds::new(
            point(px(x.floor()), y),
            size(px(w.ceil()), line_height),
          ));
        }
      }
    }

    // Search match rects.
    let mut search_rects: Vec<Bounds<Pixels>> = Vec::new();
    if !self.search_matches.is_empty() {
      let total_rows = self.content.lines.len();
      if total_rows > 0 {
        let last_row = u16::try_from(total_rows - 1).unwrap_or(u16::MAX);
        for &(row, c0, c1) in &self.search_matches {
          if row > last_row || c1 < c0 {
            continue;
          }
          #[allow(clippy::cast_precision_loss)]
          let x = f32::from(origin.x) + f32::from(cell_width) * f32::from(c0);
          #[allow(clippy::cast_precision_loss)]
          let span = f32::from(c1 - c0 + 1);
          let w = f32::from(cell_width) * span;
          #[allow(clippy::cast_precision_loss)]
          let y = origin.y + line_height * f32::from(row);
          search_rects.push(Bounds::new(
            point(px(x.floor()), y),
            size(px(w.ceil()), line_height),
          ));
        }
      }
    }

    GridLayout {
      cell_width,
      line_height,
      origin,
      bounds,
      bg_runs,
      text_runs,
      cursor_overlay,
      selection_rects,
      selection_color: self.selection_color,
      search_rects,
      search_color: self.search_match_color,
    }
  }

  fn paint(
    &mut self,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    _bounds: Bounds<Pixels>,
    _request_layout: &mut (),
    layout: &mut GridLayout,
    window: &mut Window,
    cx: &mut App,
  ) {
    // 1. Whole-grid background.
    window.paint_quad(fill(layout.bounds, self.default_bg));

    // 2. Per-run background quads.
    for bg in &layout.bg_runs {
      window.paint_quad(fill(bg.bounds, bg.color));
    }

    // 3. One shaped glyph per cell. `force_width = None` so the shaper
    //    reports natural advance; the column origin already places each
    //    glyph at `origin.x + cell_width * col`.
    for tr in &layout.text_runs {
      let shaped =
        window
          .text_system()
          .shape_line(tr.text.clone(), self.font_size, std::slice::from_ref(&tr.run), None);
      let _ = shaped.paint(tr.origin, layout.line_height, window, cx);
    }

    // 4. Search match overlay — painted under selection so a match the
    //    user has selected reads as both highlighted AND selected.
    for rect in &layout.search_rects {
      window.paint_quad(fill(*rect, layout.search_color));
    }

    // 5. Selection overlay — translucent rect on top of bg + text. Painted
    //    before the cursor so the cursor still shows above selection.
    for rect in &layout.selection_rects {
      window.paint_quad(fill(*rect, layout.selection_color));
    }

    // 5. Cursor overlay (Bar / Underline). Block already painted inline.
    if let Some(cur) = &layout.cursor_overlay {
      let bounds = match cur.kind {
        TerminalCursorStyle::Block => cur.bounds,
        TerminalCursorStyle::Underline => {
          let stripe = px(2.0);
          Bounds::new(
            Point::new(
              cur.bounds.origin.x,
              cur.bounds.origin.y + cur.bounds.size.height - stripe,
            ),
            Size::new(cur.bounds.size.width, stripe),
          )
        }
        // Thin I-beam — 1px wide for a true beam look (not a thick stripe).
        TerminalCursorStyle::Bar => Bounds::new(cur.bounds.origin, Size::new(px(1.0), cur.bounds.size.height)),
      };
      window.paint_quad(fill(bounds, cur.color));
    }

    // Suppress unused-binding warnings for the snapped origin (used for math
    // above; the bounds field carries the original rect).
    let _ = layout.origin;
  }
}

#[allow(clippy::too_many_arguments)]
fn make_bg(
  origin: Point<Pixels>,
  row_y: Pixels,
  start_col: usize,
  end_col: usize,
  color: Hsla,
  cell_width: Pixels,
  line_height: Pixels,
) -> BgRun {
  #[allow(clippy::cast_precision_loss)]
  let x = f32::from(origin.x + cell_width * (start_col as f32));
  #[allow(clippy::cast_precision_loss)]
  let span = (end_col - start_col + 1) as f32;
  let w = f32::from(cell_width) * span;
  // floor() left, ceil() width — adjacent same-color rects tile with no seam.
  let bounds = Bounds::new(point(px(x.floor()), row_y), size(px(w.ceil()), line_height));
  BgRun { bounds, color }
}

fn make_text_run(
  origin: Point<Pixels>,
  row_y: Pixels,
  start_col: usize,
  text: &str,
  attrs: &CellAttrs,
  cell_width: Pixels,
) -> TextRunSpec {
  #[allow(clippy::cast_precision_loss)]
  let x = origin.x + cell_width * (start_col as f32);
  let weight = if attrs.bold {
    FontWeight::BOLD
  } else {
    FontWeight::NORMAL
  };
  let style = if attrs.italic {
    FontStyle::Italic
  } else {
    FontStyle::Normal
  };
  let underline = if attrs.underline {
    Some(gpui::UnderlineStyle {
      color: Some(attrs.fg),
      thickness: px(1.0),
      wavy: false,
    })
  } else {
    None
  };
  let strikethrough = if attrs.strikethrough {
    Some(gpui::StrikethroughStyle {
      color: Some(attrs.fg),
      thickness: px(1.0),
    })
  } else {
    None
  };
  let run = TextRun {
    len: text.len(),
    font: monospace_font(weight, style),
    color: attrs.fg,
    background_color: None,
    underline,
    strikethrough,
  };
  TextRunSpec {
    origin: Point::new(x, row_y),
    text: SharedString::from(text.to_string()),
    run,
  }
}

/// Build a `Font` for terminal cells. Single family, no fallbacks — a
/// fallback chain causes per-glyph font swaps in the shaper, which makes
/// natural advance widths drift between cells and produces visible
/// letter-spacing in the grid.
fn monospace_font(weight: FontWeight, style: FontStyle) -> Font {
  Font {
    family: PRIMARY_FAMILY.into(),
    features: FontFeatures::default(),
    weight,
    style,
    fallbacks: None,
  }
}

#[allow(clippy::many_single_char_names)]
fn rgb_to_hsla(rgb: (u8, u8, u8)) -> Hsla {
  let r = f32::from(rgb.0) / 255.0;
  let g = f32::from(rgb.1) / 255.0;
  let b = f32::from(rgb.2) / 255.0;
  let max = r.max(g).max(b);
  let min = r.min(g).min(b);
  let l = f32::midpoint(max, min);
  if (max - min).abs() < f32::EPSILON {
    return hsla(0.0, 0.0, l, 1.0);
  }
  let d = max - min;
  let s = if l > 0.5 {
    d / (2.0 - max - min)
  } else {
    d / (max + min)
  };
  let h = if (max - r).abs() < f32::EPSILON {
    let mut h = (g - b) / d;
    if g < b {
      h += 6.0;
    }
    h / 6.0
  } else if (max - g).abs() < f32::EPSILON {
    ((b - r) / d + 2.0) / 6.0
  } else {
    ((r - g) / d + 4.0) / 6.0
  };
  hsla(h, s, l, 1.0)
}

fn colors_match(a: Hsla, b: Hsla) -> bool {
  let eps = 1e-3_f32;
  (a.h - b.h).abs() < eps && (a.s - b.s).abs() < eps && (a.l - b.l).abs() < eps && (a.a - b.a).abs() < eps
}
