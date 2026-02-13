use bitvec::{bitvec, prelude::BitVec};
use ratatui::{backend::ClearType, layout::Rect};
use std::io::{Error as IoError, Result as IoResult};

use crate::{
    backend::{
        color::{actual_bg_color, actual_fg_color},
        utils::*,
    },
    error::Error,
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

/// Width of a single cell.
///
/// This will be used for multiplying the cell's x position to get the actual pixel
/// position on the canvas.
const CELL_WIDTH: f64 = 10.0;

/// Height of a single cell.
///
/// This will be used for multiplying the cell's y position to get the actual pixel
/// position on the canvas.
const CELL_HEIGHT: f64 = 19.0;

/// Options for the [`CanvasBackend`].
#[derive(Debug)]
pub struct CanvasBackendOptions {
    /// The element ID.
    grid_id: Option<String>,
    /// Override the automatically detected size.
    size: Option<(u32, u32)>,
    /// Scale factor for the canvas. Setting this to a value greater than 1.0
    /// will improve the rendering quality on high-DPI displays, at the cost
    /// of performance.
    ///
    /// The memory usage of the canvas will increase with the square of the scale factor.
    /// For example, a scale factor of 2.0 will use 4 times the memory.
    ///
    /// The default value is 1.0.
    ///
    /// Generally, a scale factor of 2.0 is a good compromise between quality and performance.
    scale: f64,
    /// Always clip foreground drawing to the cell rectangle. Helpful when
    /// dealing with out-of-bounds rendering from problematic fonts. Enabling
    /// this option may cause some performance issues when dealing with large
    /// numbers of simultaneous changes.
    always_clip_cells: bool,
}

impl Default for CanvasBackendOptions {
    fn default() -> Self {
        Self {
            grid_id: None,
            size: None,
            scale: 1.0,
            always_clip_cells: false,
        }
    }
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

    /// Sets the scale factor for the canvas. Setting this to a value greater than 1.0
    /// will improve the rendering quality on high-DPI displays, at the cost
    /// of performance.
    ///
    /// The memory usage of the canvas will increase with the square of the scale factor.
    /// For example, a scale factor of 2.0 will use 4 times the memory.
    ///
    /// The default value is 1.0.
    ///
    /// Generally, a scale factor of 2.0 is a good compromise between quality and performance.
    ///
    /// # Panics
    ///
    /// Panics if `scale` is not positive.
    pub fn scale(mut self, scale: f64) -> Self {
        if scale <= 0.0 {
            panic!("Scale must be greater than 0");
        }
        self.scale = scale;
        self
    }
}

/// Canvas renderer.
#[derive(Debug)]
struct Canvas {
    /// Canvas element.
    inner: web_sys::HtmlCanvasElement,
    /// Rendering context.
    context: web_sys::CanvasRenderingContext2d,
    /// Background color.
    background_color: Color,
}

impl Canvas {
    /// Constructs a new [`Canvas`].
    fn new(
        parent_element: web_sys::Element,
        width: u32,
        height: u32,
        scale: f64,
        background_color: Color,
    ) -> Result<Self, Error> {
        let inner_width = (width as f64 * scale) as u32;
        let inner_height = (height as f64 * scale) as u32;
        let canvas = create_canvas_in_element(&parent_element, inner_width, inner_height)?;
        canvas.set_attribute("style", &format!("width: {width}px; height: {height}px;"))?;

        let context_options = Map::new();
        context_options.set(&JsValue::from_str("alpha"), &Boolean::from(JsValue::TRUE));
        context_options.set(
            &JsValue::from_str("desynchronized"),
            &Boolean::from(JsValue::TRUE),
        );
        let context = canvas
            .get_context_with_context_options("2d", &context_options)?
            .ok_or_else(|| Error::UnableToRetrieveCanvasContext)?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .expect("Unable to cast canvas context");
        context.set_font("16px monospace");
        context.set_text_baseline("top");
        context.scale(scale, scale)?;

        Ok(Self {
            inner: canvas,
            context,
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
    /// Always clip foreground drawing to the cell rectangle. Helpful when
    /// dealing with out-of-bounds rendering from problematic fonts. Enabling
    /// this option may cause some performance issues when dealing with large
    /// numbers of simultaneous changes.
    always_clip_cells: bool,
    /// Current buffer.
    buffer: Vec<Vec<Cell>>,
    /// Previous buffer.
    prev_buffer: Vec<Vec<Cell>>,
    /// Changed buffer cells
    changed_cells: BitVec,
    /// Canvas.
    canvas: Canvas,
    /// Cursor position.
    cursor_position: Option<Position>,
    /// The cursor shape.
    cursor_shape: CursorShape,
    /// Draw cell boundaries with specified color.
    debug_mode: Option<String>,
}

impl CanvasBackend {
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

        let canvas = Canvas::new(parent, width, height, options.scale, Color::Black)?;
        let buffer = get_sized_buffer_from_canvas(&canvas.inner);
        let changed_cells = bitvec![0; buffer.len() * buffer[0].len()];
        Ok(Self {
            prev_buffer: buffer.clone(),
            always_clip_cells: options.always_clip_cells,
            buffer,
            initialized: false,
            changed_cells,
            canvas,
            cursor_position: None,
            cursor_shape: CursorShape::SteadyBlock,
            debug_mode: None,
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

    // Compare the current buffer to the previous buffer and updates the canvas
    // accordingly.
    //
    // If `force_redraw` is `true`, the entire canvas will be cleared and redrawn.
    fn update_grid(&mut self, force_redraw: bool) -> Result<(), Error> {
        if force_redraw {
            self.canvas.context.clear_rect(
                0.0,
                0.0,
                self.canvas.inner.client_width() as f64,
                self.canvas.inner.client_height() as f64,
            );
        }
        self.canvas.context.translate(5_f64, 5_f64)?;

        // NOTE: The draw_* functions each traverse the buffer once, instead of
        // traversing it once per cell; this is done to reduce the number of
        // WASM calls per cell.
        self.resolve_changed_cells(force_redraw);
        self.draw_background()?;
        self.draw_symbols()?;
        self.draw_cursor()?;
        if self.debug_mode.is_some() {
            self.draw_debug()?;
        }

        self.canvas.context.translate(-5_f64, -5_f64)?;
        Ok(())
    }

    /// Updates the representation of the changed cells.
    ///
    /// This function updates the `changed_cells` vector to indicate which cells
    /// have changed.
    fn resolve_changed_cells(&mut self, force_redraw: bool) {
        let mut index = 0;
        for (y, line) in self.buffer.iter().enumerate() {
            for (x, cell) in line.iter().enumerate() {
                let prev_cell = &self.prev_buffer[y][x];
                self.changed_cells
                    .set(index, force_redraw || cell != prev_cell);
                index += 1;
            }
        }
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
    /// 1. Only processes cells that have changed since the last render.
    /// 2. Tracks the last foreground color used to avoid unnecessary style changes
    /// 3. Only creates clipping paths for potentially problematic glyphs (non-ASCII)
    /// or when `always_clip_cells` is enabled.
    fn draw_symbols(&mut self) -> Result<(), Error> {
        let changed_cells = &self.changed_cells;
        let mut index = 0;

        self.canvas.context.save();
        let mut last_color = None;
        for (y, line) in self.buffer.iter().enumerate() {
            for (x, cell) in line.iter().enumerate() {
                // Skip empty cells
                if !changed_cells[index] || cell.symbol() == " " {
                    index += 1;
                    continue;
                }
                let color = actual_fg_color(cell);

                // We need to reset the canvas context state in two scenarios:
                // 1. When we need to create a clipping path (for potentially problematic glyphs)
                // 2. When the text color changes
                if self.always_clip_cells || !cell.symbol().is_ascii() {
                    self.canvas.context.restore();
                    self.canvas.context.save();

                    self.canvas.context.begin_path();
                    self.canvas.context.rect(
                        x as f64 * CELL_WIDTH,
                        y as f64 * CELL_HEIGHT,
                        CELL_WIDTH,
                        CELL_HEIGHT,
                    );
                    self.canvas.context.clip();

                    last_color = None; // reset last color to avoid clipping
                    let color = get_canvas_color(color, Color::White);
                    self.canvas.context.set_fill_style_str(&color);
                } else if last_color != Some(color) {
                    self.canvas.context.restore();
                    self.canvas.context.save();

                    last_color = Some(color);

                    let color = get_canvas_color(color, Color::White);
                    self.canvas.context.set_fill_style_str(&color);
                }

                self.canvas.context.fill_text(
                    cell.symbol(),
                    x as f64 * CELL_WIDTH,
                    y as f64 * CELL_HEIGHT,
                )?;

                index += 1;
            }
        }
        self.canvas.context.restore();

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
        let changed_cells = &self.changed_cells;
        self.canvas.context.save();

        let draw_region = |(rect, color): (Rect, Color)| {
            let color = get_canvas_color(color, self.canvas.background_color);

            self.canvas.context.set_fill_style_str(&color);
            self.canvas.context.fill_rect(
                rect.x as f64 * CELL_WIDTH,
                rect.y as f64 * CELL_HEIGHT,
                rect.width as f64 * CELL_WIDTH,
                rect.height as f64 * CELL_HEIGHT,
            );
        };

        let mut index = 0;
        for (y, line) in self.buffer.iter().enumerate() {
            let mut row_renderer = RowColorOptimizer::new();
            for (x, cell) in line.iter().enumerate() {
                if changed_cells[index] {
                    // Only calls `draw_region` if the color is different from the previous one
                    row_renderer
                        .process_color((x, y), actual_bg_color(cell))
                        .map(draw_region);
                } else {
                    // Cell is unchanged so we must flush any held region
                    // to avoid clearing the foreground (symbol) of the cell
                    row_renderer.flush().map(draw_region);
                }
                index += 1;
            }
            // Flush the remaining region after traversing the row
            row_renderer.flush().map(draw_region);
        }

        self.canvas.context.restore();

        Ok(())
    }

    /// Draws the cursor on the canvas.
    fn draw_cursor(&mut self) -> Result<(), Error> {
        if let Some(pos) = self.cursor_position {
            let cell = &self.buffer[pos.y as usize][pos.x as usize];

            if cell.modifier.contains(Modifier::UNDERLINED) {
                self.canvas.context.save();

                self.canvas.context.fill_text(
                    "_",
                    pos.x as f64 * CELL_WIDTH,
                    pos.y as f64 * CELL_HEIGHT,
                )?;

                self.canvas.context.restore();
            }
        }

        Ok(())
    }

    /// Draws cell boundaries for debugging.
    fn draw_debug(&mut self) -> Result<(), Error> {
        self.canvas.context.save();

        let color = self.debug_mode.as_ref().unwrap();
        for (y, line) in self.buffer.iter().enumerate() {
            for (x, _) in line.iter().enumerate() {
                self.canvas.context.set_stroke_style_str(color);
                self.canvas.context.stroke_rect(
                    x as f64 * CELL_WIDTH,
                    y as f64 * CELL_HEIGHT,
                    CELL_WIDTH,
                    CELL_HEIGHT,
                );
            }
        }

        self.canvas.context.restore();

        Ok(())
    }
}

impl Backend for CanvasBackend {
    type Error = IoError;

    // Populates the buffer with the given content.
    fn draw<'a, I>(&mut self, content: I) -> IoResult<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
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
        // Only runs once.
        if !self.initialized {
            self.update_grid(true)?;
            self.prev_buffer = self.buffer.clone();
            self.initialized = true;
            return Ok(());
        }

        if self.buffer != self.prev_buffer {
            self.update_grid(false)?;
        }

        self.prev_buffer = self.buffer.clone();

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
        self.buffer = get_sized_buffer();
        Ok(())
    }

    fn size(&self) -> IoResult<Size> {
        Ok(Size::new(
            self.buffer[0].len().saturating_sub(1) as u16,
            self.buffer.len().saturating_sub(1) as u16,
        ))
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

/// Optimizes canvas rendering by batching adjacent cells with the same color into a single rectangle.
///
/// This reduces the number of draw calls to the canvas API by coalescing adjacent cells
/// with identical colors into larger rectangles, which is particularly beneficial for
/// WASM where calls are quiteexpensive.
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
