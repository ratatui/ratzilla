use ratatui::{backend::ClearType, layout::Rect};
use std::{
    cell::RefCell,
    io::{Error as IoError, Result as IoResult},
    rc::Rc,
};

use crate::{
    backend::{
        cell_sized::CellSized,
        color::{actual_bg_color, actual_fg_color},
        event_callback::{
            create_mouse_event, EventCallback, MouseConfig, KEY_EVENT_TYPES, MOUSE_EVENT_TYPES,
        },
        utils::*,
    },
    error::Error,
    event::{KeyEvent, MouseEvent},
    render::WebEventHandler,
    CursorShape,
};
use ratatui::{
    backend::WindowSize,
    buffer::Cell,
    layout::{Position, Size},
    prelude::Backend,
    style::{Color, Modifier},
};
use web_sys::{
    js_sys::{Boolean, Map},
    wasm_bindgen::{JsCast, JsValue},
};

/// Default width of a single cell when measurement fails.
const DEFAULT_CELL_WIDTH: f64 = 10.0;

/// Default height of a single cell when measurement fails.
const DEFAULT_CELL_HEIGHT: f64 = 19.0;

/// Padding offset used by the canvas backend.
const CANVAS_PADDING: f64 = 0.0;

/// Mouse selection mode for the canvas backend.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum SelectionMode {
    /// Select text linearly, following text flow.
    #[default]
    Linear,
    /// Select a rectangular block of cells.
    Block,
}

/// Options for the [`CanvasBackend`].
#[derive(Debug, Default)]
pub struct CanvasBackendOptions {
    /// The element ID.
    grid_id: Option<String>,
    /// Override the automatically detected size.
    size: Option<(u32, u32)>,
    /// Optional mouse selection mode.
    selection_mode: Option<SelectionMode>,
}

impl CanvasBackendOptions {
    /// Constructs a new [`CanvasBackendOptions`].
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the element id of the canvas' parent element.
    pub fn grid_id(mut self, id: &str) -> Self {
        self.grid_id = Some(id.to_string());
        self
    }

    /// Sets the size of the canvas, in pixels.
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.size = Some(size);
        self
    }

    /// Enable mouse selection with the default mode.
    pub fn enable_mouse_selection(self) -> Self {
        self.enable_mouse_selection_with_mode(SelectionMode::default())
    }

    /// Enable mouse selection with the provided mode.
    pub fn enable_mouse_selection_with_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = Some(mode);
        self
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SelectionPoint {
    col: u16,
    row: u16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SelectionRange {
    anchor: SelectionPoint,
    focus: SelectionPoint,
}

#[derive(Debug, Default)]
struct SelectionState {
    active: Option<SelectionRange>,
    drag_anchor: Option<SelectionPoint>,
    dragging: bool,
    pending_copy: bool,
    revision: u64,
}

impl SelectionState {
    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn begin(&mut self, point: SelectionPoint) {
        self.drag_anchor = Some(point);
        self.dragging = true;
        self.pending_copy = false;
        if self.active.take().is_some() {
            self.bump();
        }
    }

    fn update(&mut self, point: SelectionPoint) {
        let Some(anchor) = self.drag_anchor else {
            return;
        };

        let next = if anchor == point {
            None
        } else {
            Some(SelectionRange {
                anchor,
                focus: point,
            })
        };

        if self.active != next {
            self.active = next;
            self.bump();
        }
    }

    fn finish(&mut self, point: SelectionPoint) {
        self.update(point);
        self.dragging = false;
        self.drag_anchor = None;
        self.pending_copy = self.active.is_some();
    }
}

/// Canvas renderer.
#[derive(Debug)]
struct Canvas {
    /// Canvas element.
    inner: web_sys::HtmlCanvasElement,
    /// Visible rendering context.
    display_context: web_sys::CanvasRenderingContext2d,
    /// Offscreen frame canvas.
    frame: web_sys::HtmlCanvasElement,
    /// Offscreen frame context used for all drawing operations.
    frame_context: web_sys::CanvasRenderingContext2d,
    /// Background color.
    background_color: Color,
}

impl Canvas {
    fn create_context(
        canvas: &web_sys::HtmlCanvasElement,
    ) -> Result<web_sys::CanvasRenderingContext2d, Error> {
        let context_options = Map::new();
        context_options.set(&JsValue::from_str("alpha"), &Boolean::from(JsValue::FALSE));

        canvas
            .get_context_with_context_options("2d", &context_options)?
            .ok_or_else(|| Error::UnableToRetrieveCanvasContext)?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| Error::UnableToRetrieveCanvasContext)
    }

    fn configure_text_context(context: &web_sys::CanvasRenderingContext2d) {
        context.set_font("16px 'JetBrains Mono', monospace");
        context.set_text_align("left");
        context.set_text_baseline("alphabetic");
        context.set_image_smoothing_enabled(false);
        context.set_shadow_blur(0.0);
        context.set_global_alpha(1.0);
    }

    /// Constructs a new [`Canvas`].
    fn new(
        parent_element: web_sys::Element,
        width: u32,
        height: u32,
        background_color: Color,
    ) -> Result<Self, Error> {
        let canvas = create_canvas_in_element(&parent_element, width, height)?;
        let display_context = Self::create_context(&canvas)?;

        let frame = get_document()?
            .create_element("canvas")?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| Error::UnableToRetrieveCanvasContext)?;
        frame.set_width(width);
        frame.set_height(height);

        let frame_context = Self::create_context(&frame)?;
        Self::configure_text_context(&frame_context);

        Ok(Self {
            inner: canvas,
            display_context,
            frame,
            frame_context,
            background_color,
        })
    }
}

/// Canvas backend.
///
/// This backend renders the buffer onto a HTML canvas element.
#[derive(Debug)]
pub struct CanvasBackend {
    /// Whether the canvas has been initialized.
    initialized: bool,
    /// Current buffer.
    buffer: Vec<Vec<Cell>>,
    /// Previous buffer.
    prev_buffer: Vec<Vec<Cell>>,
    /// Canvas.
    canvas: Canvas,
    /// Measured cell width in CSS pixels.
    cell_width: f64,
    /// Measured cell height in CSS pixels.
    cell_height: f64,
    /// Alphabetic baseline offset within a cell.
    text_baseline_offset: f64,
    /// Cursor position.
    cursor_position: Option<Position>,
    /// The cursor shape.
    cursor_shape: CursorShape,
    /// Draw cell boundaries with specified color.
    debug_mode: Option<String>,
    /// Mouse selection mode.
    selection_mode: Option<SelectionMode>,
    /// Mouse selection state shared with event handlers.
    selection_state: Rc<RefCell<SelectionState>>,
    /// Last observed selection state revision.
    selection_revision: u64,
    /// Mouse event callback handler.
    mouse_callback: Option<MouseCallbackState>,
    /// Key event callback handler.
    key_callback: Option<EventCallback<web_sys::KeyboardEvent>>,
}

/// Type alias for mouse event callback state.
type MouseCallbackState = EventCallback<web_sys::MouseEvent>;

impl CanvasBackend {
    fn content_draw_size(&self) -> (f64, f64) {
        let (grid_width, grid_height) = self.canvas_grid_size();
        let width = (grid_width as f64 * self.cell_width).ceil();
        let height = (grid_height as f64 * self.cell_height).ceil();
        (width, height)
    }

    fn content_offset(&self) -> (f64, f64) {
        let (content_width, content_height) = self.content_draw_size();
        // Snap the centered origin to whole pixels so adjacent cell backgrounds
        // share exact edges instead of landing on half-pixel seams.
        let offset_x =
            (((self.canvas.inner.client_width() as f64 - content_width) / 2.0).max(0.0)).round();
        let offset_y = (((self.canvas.inner.client_height() as f64 - content_height) / 2.0)
            .max(0.0))
        .round();
        (offset_x, offset_y)
    }

    fn cell_rect(&self, x: usize, y: usize) -> (f64, f64, f64, f64) {
        let left = (x as f64 * self.cell_width).floor();
        let top = (y as f64 * self.cell_height).floor();
        let right = ((x + 1) as f64 * self.cell_width).ceil();
        let bottom = ((y + 1) as f64 * self.cell_height).ceil();
        (left, top, (right - left).max(1.0), (bottom - top).max(1.0))
    }

    fn symbol_position(&self, x: usize, y: usize) -> (f64, f64) {
        let (left, top, _, _) = self.cell_rect(x, y);
        (left, top + self.text_baseline_offset)
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        self.selection_state.borrow().active
    }

    fn selection_revision(&self) -> u64 {
        self.selection_state.borrow().revision
    }

    fn selection_row_bounds(
        mode: SelectionMode,
        range: SelectionRange,
        row: usize,
        width: usize,
    ) -> Option<(usize, usize)> {
        if width == 0 {
            return None;
        }

        match mode {
            SelectionMode::Linear => {
                let (start, end) = if (range.anchor.row, range.anchor.col)
                    <= (range.focus.row, range.focus.col)
                {
                    (range.anchor, range.focus)
                } else {
                    (range.focus, range.anchor)
                };

                if row < start.row as usize || row > end.row as usize {
                    return None;
                }

                let start_col = if row == start.row as usize {
                    start.col as usize
                } else {
                    0
                };
                let end_col = if row == end.row as usize {
                    end.col as usize
                } else {
                    width.saturating_sub(1)
                };

                Some((start_col.min(width), end_col.saturating_add(1).min(width)))
            }
            SelectionMode::Block => {
                let min_col = range.anchor.col.min(range.focus.col) as usize;
                let max_col = range.anchor.col.max(range.focus.col) as usize;
                let min_row = range.anchor.row.min(range.focus.row) as usize;
                let max_row = range.anchor.row.max(range.focus.row) as usize;

                if row < min_row || row > max_row {
                    return None;
                }

                Some((min_col.min(width), max_col.saturating_add(1).min(width)))
            }
        }
    }

    fn selected_text(&self, range: SelectionRange) -> String {
        let Some(mode) = self.selection_mode else {
            return String::new();
        };

        let mut lines = Vec::new();
        for (row_idx, row) in self.buffer.iter().enumerate() {
            let Some((start, end)) = Self::selection_row_bounds(mode, range, row_idx, row.len()) else {
                continue;
            };

            let mut line = String::new();
            for cell in &row[start..end] {
                line.push_str(cell.symbol());
            }
            while line.ends_with(' ') {
                line.pop();
            }
            lines.push(line);
        }

        lines.join("\n")
    }

    fn copy_selection_to_clipboard(&self) {
        let Some(range) = self.selection_range() else {
            return;
        };
        let text = self.selected_text(range);
        if text.is_empty() {
            return;
        }

        if let Some(window) = web_sys::window() {
            let clipboard = window.navigator().clipboard();
            let _ = clipboard.write_text(&text);
        }
    }

    fn measure_text_baseline(
        context: &web_sys::CanvasRenderingContext2d,
        cell_height: f64,
    ) -> f64 {
        let metrics = context.measure_text("Mg").ok();
        let ascent = metrics
            .as_ref()
            .map(|metrics| metrics.actual_bounding_box_ascent())
            .filter(|ascent| *ascent > 0.0)
            .unwrap_or(cell_height * 0.75);
        let descent = metrics
            .as_ref()
            .map(|metrics| metrics.actual_bounding_box_descent())
            .filter(|descent| *descent >= 0.0)
            .unwrap_or(cell_height * 0.2);

        (ascent + ((cell_height - (ascent + descent)).max(0.0) / 2.0)).round()
    }

    fn present(&self) -> Result<(), Error> {
        self.canvas.display_context.save();
        self.canvas
            .display_context
            .set_global_composite_operation("copy")?;
        self.canvas
            .display_context
            .draw_image_with_html_canvas_element(&self.canvas.frame, 0.0, 0.0)?;
        self.canvas.display_context.restore();
        Ok(())
    }

    fn canvas_grid_size(&self) -> (usize, usize) {
        let width = ((self.canvas.inner.client_width() as f64) / self.cell_width)
            .floor()
            .max(1.0) as usize;
        let height = ((self.canvas.inner.client_height() as f64) / self.cell_height)
            .floor()
            .max(1.0) as usize;
        (width, height)
    }

    fn sync_canvas_size(&mut self) {
        let width = self.canvas.inner.width();
        let height = self.canvas.inner.height();

        if self.canvas.frame.width() != width || self.canvas.frame.height() != height {
            self.canvas.frame.set_width(width);
            self.canvas.frame.set_height(height);
            Canvas::configure_text_context(&self.canvas.frame_context);
            self.canvas.display_context.set_image_smoothing_enabled(false);
            self.initialized = false;
        }

        let (grid_width, grid_height) = self.canvas_grid_size();
        let needs_buffer_resize = self.buffer.len() != grid_height
            || self
                .buffer
                .first()
                .map(|line| line.len() != grid_width)
                .unwrap_or(true);

        if needs_buffer_resize {
            self.buffer = vec![vec![Cell::default(); grid_width]; grid_height];
            self.prev_buffer = self.buffer.clone();
            self.initialized = false;
        }
    }

    fn measure_cell_size(parent: &web_sys::Element) -> Result<(f64, f64), Error> {
        let document = get_document()?;
        let pre = document.create_element("pre")?;
        pre.set_attribute(
            "style",
            "margin: 0; padding: 0; border: 0; line-height: 1; font: 16px 'JetBrains Mono', monospace;",
        )?;

        let span = document.create_element("span")?;
        span.set_inner_html("\u{2588}");
        span.set_attribute(
            "style",
            "display: inline-block; width: 1ch; line-height: 1; font: 16px 'JetBrains Mono', monospace;",
        )?;

        pre.append_child(&span)?;
        parent.append_child(&pre)?;

        let rect = span.get_bounding_client_rect();
        let width = rect.width();
        let height = rect.height();

        parent.remove_child(&pre)?;

        if width > 0.0 && height > 0.0 {
            Ok((width, height))
        } else {
            Ok((DEFAULT_CELL_WIDTH, DEFAULT_CELL_HEIGHT))
        }
    }

    /// Constructs a new [`CanvasBackend`].
    pub fn new() -> Result<Self, Error> {
        let (width, height) = get_raw_window_size();
        Self::new_with_size(width.into(), height.into())
    }

    /// Constructs a new [`CanvasBackend`] with the given size.
    pub fn new_with_size(width: u32, height: u32) -> Result<Self, Error> {
        Self::new_with_options(CanvasBackendOptions {
            size: Some((width, height)),
            ..Default::default()
        })
    }

    /// Constructs a new [`CanvasBackend`] with the given options.
    pub fn new_with_options(options: CanvasBackendOptions) -> Result<Self, Error> {
        // Parent element of canvas (uses <body> unless specified)
        let parent = get_element_by_id_or_body(options.grid_id.as_ref())?;

        let (width, height) = options
            .size
            .unwrap_or_else(|| (parent.client_width() as u32, parent.client_height() as u32));

        let cell_size = Self::measure_cell_size(&parent)?;
        let canvas = Canvas::new(parent, width, height, Color::Black)?;
        let text_baseline_offset = Self::measure_text_baseline(&canvas.frame_context, cell_size.1);
        let buffer = get_sized_buffer_from_canvas(&canvas.inner, cell_size.0, cell_size.1);
        Ok(Self {
            prev_buffer: buffer.clone(),
            buffer,
            initialized: false,
            canvas,
            cell_width: cell_size.0,
            cell_height: cell_size.1,
            text_baseline_offset,
            cursor_position: None,
            cursor_shape: CursorShape::SteadyBlock,
            debug_mode: None,
            selection_mode: options.selection_mode,
            selection_state: Rc::new(RefCell::new(SelectionState::default())),
            selection_revision: 0,
            mouse_callback: None,
            key_callback: None,
        })
    }

    /// Sets the background color of the canvas.
    pub fn set_background_color(&mut self, color: Color) {
        self.canvas.background_color = color;
    }

    /// Returns the [`CursorShape`].
    pub fn cursor_shape(&self) -> &CursorShape {
        &self.cursor_shape
    }

    /// Set the [`CursorShape`].
    pub fn set_cursor_shape(mut self, shape: CursorShape) -> Self {
        self.cursor_shape = shape;
        self
    }

    /// Enable or disable debug mode to draw cells with a specified color.
    ///
    /// The format of the color is the same as the CSS color format, e.g.:
    /// - `#666`
    /// - `#ff0000`
    /// - `red`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ratzilla::CanvasBackend;
    /// let mut backend = CanvasBackend::new().unwrap();
    ///
    /// backend.set_debug_mode(Some("#666"));
    /// backend.set_debug_mode(Some("red"));
    /// ```
    pub fn set_debug_mode<T: Into<String>>(&mut self, color: Option<T>) {
        self.debug_mode = color.map(Into::into);
    }

    // Redraw the entire offscreen frame, then present it in a single blit.
    fn render_frame(&mut self) -> Result<(), Error> {
        let background = get_canvas_color(self.canvas.background_color, Color::Black);
        self.canvas.frame_context.set_fill_style_str(&background);
        self.canvas.frame_context.fill_rect(
            0.0,
            0.0,
            self.canvas.frame.width() as f64,
            self.canvas.frame.height() as f64,
        );
        let (offset_x, offset_y) = self.content_offset();
        self.canvas
            .frame_context
            .translate(CANVAS_PADDING + offset_x, CANVAS_PADDING + offset_y)?;

        self.draw_background()?;
        self.draw_selection()?;
        self.draw_symbols()?;
        self.draw_cursor()?;
        if self.debug_mode.is_some() {
            self.draw_debug()?;
        }

        self.canvas
            .frame_context
            .translate(-(CANVAS_PADDING + offset_x), -(CANVAS_PADDING + offset_y))?;
        self.present()?;
        Ok(())
    }

    fn draw_selection(&mut self) -> Result<(), Error> {
        let Some(mode) = self.selection_mode else {
            return Ok(());
        };
        let Some(range) = self.selection_range() else {
            return Ok(());
        };

        self.canvas.frame_context.save();
        self.canvas
            .frame_context
            .set_fill_style_str("rgba(170, 190, 230, 0.24)");

        for (row_idx, row) in self.buffer.iter().enumerate() {
            let Some((start, end)) = Self::selection_row_bounds(mode, range, row_idx, row.len()) else {
                continue;
            };
            if start >= end {
                continue;
            }

            let start_x = (start as f64 * self.cell_width).floor();
            let start_y = (row_idx as f64 * self.cell_height).floor();
            let end_x = (end as f64 * self.cell_width).ceil();
            let end_y = ((row_idx + 1) as f64 * self.cell_height).ceil();
            self.canvas
                .frame_context
                .fill_rect(start_x, start_y, end_x - start_x, end_y - start_y);
        }

        self.canvas.frame_context.restore();
        Ok(())
    }

    /// Draws the text symbols on the canvas.
    ///
    /// This method renders the textual content of each cell in the buffer, optimizing canvas operations
    /// by minimizing state changes across the WebAssembly boundary.
    ///
    /// # Optimization Strategy
    ///
    /// Rather than saving/restoring the canvas context for every cell (which would be expensive),
    /// this implementation:
    ///
    /// Tracks the last foreground color used to avoid unnecessary style changes.
    ///
    fn draw_symbols(&mut self) -> Result<(), Error> {
        self.canvas.frame_context.save();
        let mut last_color = None;
        for (y, line) in self.buffer.iter().enumerate() {
            for (x, cell) in line.iter().enumerate() {
                if cell.symbol() == " " {
                    continue;
                }
                let color = actual_fg_color(cell);

                if last_color != Some(color) {
                    self.canvas.frame_context.restore();
                    self.canvas.frame_context.save();

                    last_color = Some(color);

                    let color = get_canvas_color(color, Color::White);
                    self.canvas.frame_context.set_fill_style_str(&color);
                }

                let (text_x, text_y) = self.symbol_position(x, y);
                self.canvas
                    .frame_context
                    .fill_text(cell.symbol(), text_x, text_y)?;
            }
        }
        self.canvas.frame_context.restore();

        Ok(())
    }

    /// Draws the background of the cells.
    ///
    /// This function uses [`RowColorOptimizer`] to optimize the drawing of the background
    /// colors by batching adjacent cells with the same color into a single rectangle.
    ///
    /// In other words, it accumulates "what to draw" until it finds a different
    /// color, and then it draws the accumulated rectangle.
    fn draw_background(&mut self) -> Result<(), Error> {
        self.canvas.frame_context.save();

        let draw_region = |(rect, color): (Rect, Color)| {
            let color = get_canvas_color(color, self.canvas.background_color);
            let start_x = (rect.x as f64 * self.cell_width).floor();
            let start_y = (rect.y as f64 * self.cell_height).floor();
            let end_x = ((rect.x + rect.width) as f64 * self.cell_width).ceil();
            let end_y = ((rect.y + rect.height) as f64 * self.cell_height).ceil();

            self.canvas.frame_context.set_fill_style_str(&color);
            self.canvas
                .frame_context
                .fill_rect(start_x, start_y, end_x - start_x, end_y - start_y);
        };

        for (y, line) in self.buffer.iter().enumerate() {
            let mut row_renderer = RowColorOptimizer::new();
            for (x, cell) in line.iter().enumerate() {
                row_renderer
                    .process_color((x, y), actual_bg_color(cell))
                    .map(draw_region);
            }
            row_renderer.flush().map(draw_region);
        }

        self.canvas.frame_context.restore();

        Ok(())
    }

    /// Draws the cursor on the canvas.
    fn draw_cursor(&mut self) -> Result<(), Error> {
        if let Some(pos) = self.cursor_position {
            let cell = &self.buffer[pos.y as usize][pos.x as usize];

            if cell.modifier.contains(Modifier::UNDERLINED) {
                self.canvas.frame_context.save();

                self.canvas.frame_context.fill_text(
                    "_",
                    pos.x as f64 * self.cell_width,
                    pos.y as f64 * self.cell_height,
                )?;

                self.canvas.frame_context.restore();
            }
        }

        Ok(())
    }

    /// Draws cell boundaries for debugging.
    fn draw_debug(&mut self) -> Result<(), Error> {
        self.canvas.frame_context.save();

        let color = self.debug_mode.as_ref().unwrap();
        for (y, line) in self.buffer.iter().enumerate() {
            for (x, _) in line.iter().enumerate() {
                self.canvas.frame_context.set_stroke_style_str(color);
                self.canvas.frame_context.stroke_rect(
                    x as f64 * self.cell_width,
                    y as f64 * self.cell_height,
                    self.cell_width,
                    self.cell_height,
                );
            }
        }

        self.canvas.frame_context.restore();

        Ok(())
    }
}

impl CellSized for CanvasBackend {
    fn cell_size_px(&self) -> (f32, f32) {
        (self.cell_width as f32, self.cell_height as f32)
    }

    fn cell_size_css_px(&self) -> (f32, f32) {
        (self.cell_width as f32, self.cell_height as f32)
    }
}

impl Backend for CanvasBackend {
    type Error = IoError;

    // Populates the buffer with the given content.
    fn draw<'a, I>(&mut self, content: I) -> IoResult<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.sync_canvas_size();

        for (x, y, cell) in content {
            let y = y as usize;
            let x = x as usize;
            let line = &mut self.buffer[y];
            line.extend(std::iter::repeat_with(Cell::default).take(x.saturating_sub(line.len())));
            line[x] = cell.clone();
        }

        // Draw the cursor if set
        if let Some(pos) = self.cursor_position {
            let y = pos.y as usize;
            let x = pos.x as usize;
            let line = &mut self.buffer[y];
            if x < line.len() {
                let cursor_style = self.cursor_shape.show(line[x].style());
                line[x].set_style(cursor_style);
            }
        }

        Ok(())
    }

    /// Flush the content to the screen.
    ///
    /// This function is called after the [`CanvasBackend::draw`] function to
    /// actually render the content to the screen.
    fn flush(&mut self) -> IoResult<()> {
        self.sync_canvas_size();
        let selection_revision = self.selection_revision();

        if !self.initialized
            || self.buffer != self.prev_buffer
            || self.selection_revision != selection_revision
        {
            self.render_frame()?;
            self.prev_buffer = self.buffer.clone();
            self.initialized = true;
            self.selection_revision = selection_revision;
        }

        let should_copy = {
            let mut selection_state = self.selection_state.borrow_mut();
            let should_copy = selection_state.pending_copy;
            selection_state.pending_copy = false;
            should_copy
        };
        if should_copy {
            self.copy_selection_to_clipboard();
        }

        Ok(())
    }

    fn hide_cursor(&mut self) -> IoResult<()> {
        if let Some(pos) = self.cursor_position {
            let y = pos.y as usize;
            let x = pos.x as usize;
            let line = &mut self.buffer[y];
            if x < line.len() {
                let style = self.cursor_shape.hide(line[x].style());
                line[x].set_style(style);
            }
        }
        self.cursor_position = None;
        Ok(())
    }

    fn show_cursor(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn get_cursor(&mut self) -> IoResult<(u16, u16)> {
        Ok((0, 0))
    }

    fn set_cursor(&mut self, _x: u16, _y: u16) -> IoResult<()> {
        Ok(())
    }

    fn clear(&mut self) -> IoResult<()> {
        self.sync_canvas_size();
        self.buffer =
            get_sized_buffer_from_canvas(&self.canvas.inner, self.cell_width, self.cell_height);
        self.prev_buffer = self.buffer.clone();
        self.initialized = false;
        Ok(())
    }

    fn size(&self) -> IoResult<Size> {
        let (width, height) = self.canvas_grid_size();
        Ok(Size::new(width as u16, height as u16))
    }

    fn window_size(&mut self) -> IoResult<WindowSize> {
        unimplemented!()
    }

    fn get_cursor_position(&mut self) -> IoResult<Position> {
        match self.cursor_position {
            None => Ok((0, 0).into()),
            Some(position) => Ok(position),
        }
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> IoResult<()> {
        let new_pos = position.into();
        if let Some(old_pos) = self.cursor_position {
            let y = old_pos.y as usize;
            let x = old_pos.x as usize;
            let line = &mut self.buffer[y];
            if x < line.len() && old_pos != new_pos {
                let style = self.cursor_shape.hide(line[x].style());
                line[x].set_style(style);
            }
        }
        self.cursor_position = Some(new_pos);
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(IoError::other("unimplemented")),
        }
    }
}

impl WebEventHandler for CanvasBackend {
    fn on_mouse_event<F>(&mut self, mut callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        // Clear any existing handlers first
        self.clear_mouse_events();

        // Get grid dimensions from the buffer
        let grid_width = self.buffer[0].len() as u16;
        let grid_height = self.buffer.len() as u16;

        // Configure coordinate translation for canvas backend
        let config = MouseConfig::new(grid_width, grid_height)
            .with_offsets(
                CANVAS_PADDING + self.content_offset().0,
                CANVAS_PADDING + self.content_offset().1,
            )
            .with_cell_dimensions(self.cell_width, self.cell_height);

        let element: web_sys::Element = self.canvas.inner.clone().into();
        let element_for_closure = element.clone();
        let selection_state = self.selection_state.clone();
        let selection_mode = self.selection_mode;

        // Create mouse event callback
        let mouse_callback = EventCallback::new(
            element,
            MOUSE_EVENT_TYPES,
            move |event: web_sys::MouseEvent| {
                let mouse_event = create_mouse_event(&event, &element_for_closure, &config);
                if selection_mode.is_some() {
                    let point = SelectionPoint {
                        col: mouse_event.col,
                        row: mouse_event.row,
                    };
                    let mut selection_state = selection_state.borrow_mut();
                    match event.type_().as_str() {
                        "mousedown" if event.button() == 0 => selection_state.begin(point),
                        "mousemove" if selection_state.dragging => selection_state.update(point),
                        "mouseup" if event.button() == 0 && selection_state.dragging => {
                            selection_state.finish(point)
                        }
                        "mouseleave" if selection_state.dragging => selection_state.finish(point),
                        _ => {}
                    }
                }
                callback(mouse_event);
            },
        )?;

        self.mouse_callback = Some(mouse_callback);

        Ok(())
    }

    fn clear_mouse_events(&mut self) {
        // Drop the callback, which will remove the event listeners
        self.mouse_callback = None;
    }

    fn on_key_event<F>(&mut self, mut callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        // Clear any existing handlers first
        self.clear_key_events();

        let element: web_sys::Element = self.canvas.inner.clone().into();

        // Make the canvas focusable so it can receive key events
        self.canvas
            .inner
            .set_attribute("tabindex", "0")
            .map_err(Error::from)?;

        let selection_state = self.selection_state.clone();
        self.key_callback = Some(EventCallback::new(
            element,
            KEY_EVENT_TYPES,
            move |event: web_sys::KeyboardEvent| {
                let is_copy = (event.ctrl_key() || event.meta_key())
                    && matches!(event.key().as_str(), "c" | "C");
                if is_copy && selection_state.borrow().active.is_some() {
                    event.prevent_default();
                    let mut selection_state = selection_state.borrow_mut();
                    selection_state.pending_copy = true;
                    return;
                }
                callback(event.into());
            },
        )?);

        Ok(())
    }

    fn clear_key_events(&mut self) {
        self.key_callback = None;
    }
}

/// Optimizes canvas rendering by batching adjacent cells with the same color into a single rectangle.
///
/// This reduces the number of draw calls to the canvas API by coalescing adjacent cells
/// with identical colors into larger rectangles, which is particularly beneficial for
/// WASM where calls are quite expensive.
struct RowColorOptimizer {
    /// The currently accumulating region and its color
    pending_region: Option<(Rect, Color)>,
}

impl RowColorOptimizer {
    /// Creates a new empty optimizer with no pending region.
    fn new() -> Self {
        Self {
            pending_region: None,
        }
    }

    /// Processes a cell with the given position and color.
    fn process_color(&mut self, pos: (usize, usize), color: Color) -> Option<(Rect, Color)> {
        if let Some((active_rect, active_color)) = self.pending_region.as_mut() {
            if active_color == &color {
                // Same color: extend the rectangle
                active_rect.width += 1;
            } else {
                // Different color: flush the previous region and start a new one
                let region = *active_rect;
                let region_color = *active_color;
                *active_rect = Rect::new(pos.0 as _, pos.1 as _, 1, 1);
                *active_color = color;
                return Some((region, region_color));
            }
        } else {
            // First color: create a new rectangle
            let rect = Rect::new(pos.0 as _, pos.1 as _, 1, 1);
            self.pending_region = Some((rect, color));
        }

        None
    }

    /// Finalizes and returns the current pending region, if any.
    fn flush(&mut self) -> Option<(Rect, Color)> {
        self.pending_region.take()
    }
}
