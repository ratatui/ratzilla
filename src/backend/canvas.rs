use indexmap::IndexMap;
use ratatui::{backend::ClearType, layout::Rect};
use std::{
    fmt::Debug,
    io::{Error as IoError, Result as IoResult},
    iter::Peekable,
    vec::Drain,
};

use crate::{
    backend::{
        color::{actual_bg_color, actual_fg_color, to_rgb},
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
    style::Color,
};
use sledgehammer_bindgen::bindgen;
use web_sys::wasm_bindgen::{self, prelude::*};

/// Width of a single cell.
///
/// This will be used for multiplying the cell's x position to get the actual pixel
/// position on the canvas.
const CELL_WIDTH: u16 = 10;

/// Height of a single cell.
///
/// This will be used for multiplying the cell's y position to get the actual pixel
/// position on the canvas.
const CELL_HEIGHT: u16 = 19;

/// Options for the [`CanvasBackend`].
#[derive(Debug, Default)]
pub struct CanvasBackendOptions {
    /// The element ID.
    grid_id: Option<String>,
    /// Override the automatically detected size.
    size: Option<(u32, u32)>,
    /// Always clip foreground drawing to the cell rectangle. Helpful when
    /// dealing with out-of-bounds rendering from problematic fonts. Enabling
    /// this option may cause some performance issues when dealing with large
    /// numbers of simultaneous changes.
    always_clip_cells: bool,
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
}

// Mirrors usage in https://github.com/DioxusLabs/dioxus/blob/main/packages/interpreter/src/unified_bindings.rs
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen]
    /// External JS class for managing the actual HTML canvas, context,
    /// and parent element.
    pub type RatzillaCanvas;

    #[wasm_bindgen(method)]
    /// Does the initial construction of the RatzillaCanvas class
    ///
    /// `sledgehammer_bindgen` only lets you have an empty constructor,
    /// so we must initialize the class after construction
    fn create_canvas_in_element(
        this: &RatzillaCanvas,
        parent: &web_sys::Element,
        width: u32,
        height: u32,
    );

    #[wasm_bindgen(method)]
    /// Initializes the canvas 2D context with the appropriate properties
    fn init_ctx(this: &RatzillaCanvas);

    #[wasm_bindgen(method)]
    /// Shares the canvas 2D context with the other buffer
    fn share_ctx_with_other(this: &RatzillaCanvas, other: &RatzillaCanvas);

    #[wasm_bindgen(method)]
    fn get_canvas(this: &RatzillaCanvas) -> web_sys::HtmlCanvasElement;

    #[wasm_bindgen(method)]
    fn get_ctx(this: &RatzillaCanvas) -> web_sys::CanvasRenderingContext2d;
}

#[bindgen]
mod js {
    #[extends(RatzillaCanvas)]
    /// Responsible for buffering the calls to the canvas and
    /// canvas context
    struct Buffer;

    const BASE: &str = r#"src/backend/ratzilla_canvas.js"#;

    fn clear_rect() {
        r#"
            this.ctx.fillRect(
                0, 0, this.canvas.width, this.canvas.height
            );
        "#
    }

    fn save() {
        r#"
            this.ctx.save();
        "#
    }

    fn restore() {
        r#"
            this.ctx.restore();
        "#
    }

    fn fill() {
        r#"
            this.ctx.fill();
        "#
    }

    fn translate(x: u16, y: u16) {
        r#"
            this.ctx.translate($x$, $y$);
        "#
    }

    fn translate_neg(x: u16, y: u16) {
        r#"
            this.ctx.translate(-$x$, -$y$);
        "#
    }

    fn begin_path() {
        r#"
            this.ctx.beginPath();
        "#
    }

    fn rect(x: u16, y: u16, w: u16, h: u16) {
        r#"
            this.ctx.rect($x$, $y$, $w$, $h$);
        "#
    }

    fn clip() {
        r#"
            this.ctx.clip();
        "#
    }

    fn set_fill_style_str(style: &str) {
        r#"
            this.ctx.fillStyle = $style$;
        "#
    }

    fn set_fill_style(style: u32) {
        r#"
            this.ctx.fillStyle = `#\${$style$.toString(16).padStart(6, '0')}`;
        "#
    }

    fn set_stroke_style_str(style: &str) {
        r#"
            this.ctx.strokeStyle = $style$;
        "#
    }

    fn fill_text(text: &str, x: u16, y: u16) {
        r#"
            this.ctx.fillText($text$, $x$, $y$);
        "#
    }

    fn fill_rect(x: u16, y: u16, w: u16, h: u16) {
        r#"
            this.ctx.fillRect($x$, $y$, $w$, $h$);
        "#
    }

    fn stroke_rect(x: u16, y: u16, w: u16, h: u16) {
        r#"
            this.ctx.strokeRect($x$, $y$, $w$, $h$);
        "#
    }
}

impl Buffer {
    /// Converts the buffer to its baseclass
    pub fn ratzilla_canvas(&self) -> &RatzillaCanvas {
        self.js_channel().unchecked_ref()
    }
}

impl Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Buffer")
    }
}

/// Canvas renderer.
#[derive(Debug)]
struct Canvas {
    /// Foreground (symbol) Rendering context.
    fg_context: Buffer,
    /// Background Rendering context.
    bg_context: Buffer,
    /// Background color.
    background_color: Color,
}

impl Canvas {
    /// Constructs a new [`Canvas`].
    fn new(
        parent_element: web_sys::Element,
        width: u32,
        height: u32,
        background_color: Color,
    ) -> Result<Self, Error> {
        let fg_context = Buffer::default();
        fg_context
            .ratzilla_canvas()
            .create_canvas_in_element(&parent_element, width, height);

        fg_context.ratzilla_canvas().init_ctx();

        let bg_context = Buffer::default();
        bg_context
            .ratzilla_canvas()
            .share_ctx_with_other(fg_context.ratzilla_canvas());

        Ok(Self {
            fg_context,
            bg_context,
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
    /// Groups together and merges rectangles with
    /// the same fill color
    bg_rect_optimizer: RectColorOptimizer,
    /// Canvas.
    canvas: Canvas,
    /// Is true if the cursor is currently visible
    cursor_shown: bool,
    /// Cursor position.
    cursor_position: Position,
    /// The cursor shape.
    cursor_shape: CursorShape,
    /// Draw cell boundaries with specified color.
    debug_mode: Option<String>,
    /// Mouse event callback handler.
    mouse_callback: Option<MouseCallbackState>,
    /// Key event callback handler.
    key_callback: Option<EventCallback<web_sys::KeyboardEvent>>,
}

/// Type alias for mouse event callback state.
type MouseCallbackState = EventCallback<web_sys::MouseEvent>;

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

        let canvas = Canvas::new(parent, width, height, Color::Black)?;
        let buffer =
            get_sized_buffer_from_canvas(&canvas.fg_context.ratzilla_canvas().get_canvas());
        Ok(Self {
            always_clip_cells: options.always_clip_cells,
            buffer,
            initialized: false,
            bg_rect_optimizer: RectColorOptimizer::default(),
            canvas,
            cursor_position: Position::MIN,
            cursor_shape: CursorShape::SteadyBlock,
            cursor_shown: false,
            debug_mode: None,
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

    // Compare the current buffer to the previous buffer and updates the canvas
    // accordingly.
    //
    // If `force_redraw` is `true`, the entire canvas will be cleared and redrawn.
    fn update_grid(&mut self, force_redraw: bool) -> Result<(), Error> {
        // Happens immediately (unbuffered)
        if force_redraw {
            let ctx = self.canvas.bg_context.ratzilla_canvas().get_ctx();

            ctx.set_fill_style_str(&get_canvas_color(
                self.canvas.background_color,
                self.canvas.background_color,
            ));
            // Infallible
            let size = self.size().unwrap();
            ctx.fill_rect(
                0.0,
                0.0,
                (size.width * CELL_WIDTH) as f64,
                (size.height * CELL_HEIGHT) as f64,
            );
        }

        // NOTE: The draw_* functions each traverse the buffer once, instead of
        // traversing it once per cell; this is done to reduce the number of
        // WASM calls per cell.
        if self.debug_mode.is_some() {
            self.draw_debug()?;
        }

        self.canvas.bg_context.flush();
        self.canvas.fg_context.flush();

        Ok(())
    }

    /// Draws cell boundaries for debugging.
    fn draw_debug(&mut self) -> Result<(), Error> {
        let color = self.debug_mode.as_ref().unwrap();
        for (y, line) in self.buffer.iter().enumerate() {
            for (x, _) in line.iter().enumerate() {
                self.canvas.fg_context.set_stroke_style_str(color);
                self.canvas.fg_context.stroke_rect(
                    x as u16 * CELL_WIDTH,
                    y as u16 * CELL_HEIGHT,
                    CELL_WIDTH,
                    CELL_HEIGHT,
                );
            }
        }

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
        let mut last_color = None;
        self.canvas.fg_context.save();

        for (x, y, cell) in content {
            {
                let x = x as usize;
                let y = y as usize;
                if let Some(line) = self.buffer.get_mut(y) {
                    line.get_mut(x).map(|c| *c = cell.clone());
                }
            }

            self.bg_rect_optimizer
                .process_color((x as usize, y as usize), actual_bg_color(cell));

            // Draws the text symbols on the canvas.
            //
            // This method renders the textual content of each cell in the buffer, optimizing canvas operations
            // by minimizing state changes across the WebAssembly boundary.
            //
            // # Optimization Strategy
            //
            // Rather than saving/restoring the canvas context forself. {
            // cursor_positionvery cell (which would be expensive),
            // this implementation:
            //
            // 1. Only processes cells that have changed since the last render.
            // 2. Tracks the last foreground color used to avoid unnecessary style changes
            // 3. Only creates clipping paths for potentially problematic glyphs (non-ASCII)
            // or when `always_clip_cells` is enabled.
            let color = actual_fg_color(cell);

            // We need to reset the canvas context state in two scenarios:
            // 1. When we need to create a clipping path (for potentially problematic glyphs)
            // 2. When the text color changes
            if self.always_clip_cells || !cell.symbol().is_ascii() {
                self.canvas.fg_context.restore();
                self.canvas.fg_context.save();

                self.canvas.fg_context.begin_path();
                self.canvas.fg_context.rect(
                    x * CELL_WIDTH,
                    y * CELL_HEIGHT,
                    CELL_WIDTH,
                    CELL_HEIGHT,
                );
                self.canvas.fg_context.clip();

                last_color = None; // reset last color to avoid clipping
                let color = to_rgb(color, 0xFFFFFFFF);
                self.canvas.fg_context.set_fill_style(color);
            } else if last_color != Some(color) {
                self.canvas.fg_context.restore();
                self.canvas.fg_context.save();

                last_color = Some(color);

                let color = to_rgb(color, 0xFFFFFFFF);
                self.canvas.fg_context.set_fill_style(color);
            }

            if cell.symbol() != " " {
                self.canvas
                    .fg_context
                    .fill_text(cell.symbol(), x * CELL_WIDTH, y * CELL_HEIGHT);
            }

            if self.cursor_shown && self.cursor_position == Position::new(x, y) {
                self.canvas
                    .fg_context
                    .fill_text("_", x * CELL_WIDTH, y * CELL_HEIGHT);
            }
        }

        self.canvas.fg_context.restore();

        for (color, rects) in self.bg_rect_optimizer.finish() {
            let color = to_rgb(color, to_rgb(self.canvas.background_color, 0x00000000));

            self.canvas.bg_context.begin_path();
            for rect in rects {
                self.canvas.bg_context.rect(
                    rect.x * CELL_WIDTH,
                    rect.y * CELL_HEIGHT,
                    rect.width * CELL_WIDTH,
                    rect.height * CELL_HEIGHT,
                );
            }

            self.canvas.bg_context.set_fill_style(color);
            self.canvas.bg_context.fill();
        }

        Ok(())
    }

    /// Flush the content to the screen.
    ///
    /// This function is called after the [`CanvasBackend::draw`] function to
    /// actually render the content to the screen.
    fn flush(&mut self) -> IoResult<()> {
        self.update_grid(
            // Only runs once.
            !self.initialized,
        )?;
        self.initialized = true;
        Ok(())
    }

    fn hide_cursor(&mut self) -> IoResult<()> {
        // Redraw the cell under the cursor, but without
        // the cursor style
        if self.cursor_shown {
            self.flush()?;
            self.cursor_shown = false;
            let x = self.cursor_position.x as usize;
            let y = self.cursor_position.y as usize;
            if let Some(line) = self.buffer.get(y) {
                if let Some(cell) = line.get(x).cloned() {
                    self.draw(
                        [(self.cursor_position.x, self.cursor_position.y, &cell)].into_iter(),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn show_cursor(&mut self) -> IoResult<()> {
        // Redraw the new cell under the cursor, but with
        // the cursor style
        if !self.cursor_shown {
            self.flush()?;
            self.cursor_shown = true;
            let x = self.cursor_position.x as usize;
            let y = self.cursor_position.y as usize;
            if let Some(line) = self.buffer.get(y) {
                if let Some(cell) = line.get(x).cloned() {
                    self.draw(
                        [(self.cursor_position.x, self.cursor_position.y, &cell)].into_iter(),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn get_cursor(&mut self) -> IoResult<(u16, u16)> {
        Ok((0, 0))
    }

    fn set_cursor(&mut self, _x: u16, _y: u16) -> IoResult<()> {
        Ok(())
    }

    fn clear(&mut self) -> IoResult<()> {
        self.canvas.bg_context.set_fill_style_str(&get_canvas_color(
            self.canvas.background_color,
            self.canvas.background_color,
        ));
        self.canvas.bg_context.clear_rect();
        self.buffer
            .iter_mut()
            .flatten()
            .for_each(|c| *c = Cell::default());
        Ok(())
    }

    fn size(&self) -> IoResult<Size> {
        Ok(Size::new(
            self.buffer
                .get(0)
                .map(|b| b.len())
                .unwrap_or(0)
                .saturating_sub(1) as u16,
            self.buffer.len().saturating_sub(1) as u16,
        ))
    }

    fn window_size(&mut self) -> IoResult<WindowSize> {
        unimplemented!()
    }

    fn get_cursor_position(&mut self) -> IoResult<Position> {
        Ok(self.cursor_position)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> IoResult<()> {
        let new_position = position.into();
        if self.cursor_position != new_position {
            self.hide_cursor()?;
            self.cursor_position = new_position;
            self.show_cursor()?;
        }
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
            .with_offset(5.0) // Canvas translation offset
            .with_cell_dimensions(CELL_WIDTH as f64, CELL_HEIGHT as f64);

        let element: web_sys::Element =
            self.canvas.fg_context.ratzilla_canvas().get_canvas().into();
        let element_for_closure = element.clone();

        // Create mouse event callback
        let mouse_callback = EventCallback::new(
            element,
            MOUSE_EVENT_TYPES,
            move |event: web_sys::MouseEvent| {
                let mouse_event = create_mouse_event(&event, &element_for_closure, &config);
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

        let element: web_sys::Element =
            self.canvas.fg_context.ratzilla_canvas().get_canvas().into();

        // Make the canvas focusable so it can receive key events
        element
            .set_attribute("tabindex", "0")
            .map_err(Error::from)?;

        self.key_callback = Some(EventCallback::new(
            element,
            KEY_EVENT_TYPES,
            move |event: web_sys::KeyboardEvent| {
                callback(event.into());
            },
        )?);

        Ok(())
    }

    fn clear_key_events(&mut self) {
        self.key_callback = None;
    }
}

struct RectColumnMerger<'a> {
    iter: Peekable<Drain<'a, Rect>>,
}

impl Iterator for RectColumnMerger<'_> {
    type Item = Rect;

    fn next(&mut self) -> Option<Self::Item> {
        let mut initial_rect = self.iter.next()?;

        let mut y = initial_rect.y;
        while let Some(next_rect) = self.iter.peek() {
            if initial_rect.x == next_rect.x
                && y + 1 == next_rect.y
                && initial_rect.width == next_rect.width
            {
                self.iter.next();
                y += 1;
                initial_rect.height += 1;
            } else {
                break;
            }
        }

        Some(initial_rect)
    }
}

struct RectColorMerger<'a> {
    iter: indexmap::map::IterMut<'a, Color, Vec<Rect>>,
}

impl<'a> Iterator for RectColorMerger<'a> {
    type Item = (Color, RectColumnMerger<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut next_item = self.iter.next()?;

        while next_item.1.is_empty() {
            next_item = self.iter.next()?;
        }

        next_item.1.sort_unstable_by_key(|r| (r.x, r.y));

        Some((
            *next_item.0,
            RectColumnMerger {
                iter: next_item.1.drain(..).peekable(),
            },
        ))
    }
}

#[derive(Debug, Default)]
struct RectColorOptimizer {
    rects: IndexMap<Color, Vec<Rect>>,
}

impl RectColorOptimizer {
    fn process_color(&mut self, pos: (usize, usize), color: Color) {
        let color_entry = self.rects.entry(color).or_default();
        let pending_region = color_entry.last_mut();

        if let Some(active_rect) = pending_region {
            if active_rect.right() as usize == pos.0 && active_rect.y as usize == pos.1 {
                // Directly next to active_rect: extend the rectangle
                active_rect.width += 1;
            } else {
                // Different color: flush the previous region and start a new one
                color_entry.push(Rect::new(pos.0 as _, pos.1 as _, 1, 1));
                // return Some((region, region_color));
            }
        } else {
            // First color: create a new rectangle
            let rect = Rect::new(pos.0 as _, pos.1 as _, 1, 1);
            color_entry.push(rect);
        }
    }

    fn finish(&mut self) -> RectColorMerger<'_> {
        RectColorMerger {
            iter: self.rects.iter_mut(),
        }
    }
}
