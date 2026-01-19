use crate::{
    backend::{color::to_rgb, utils::*},
    error::Error,
    widgets::hyperlink::HYPERLINK_MODIFIER,
    CursorShape,
};
pub use beamterm_renderer::SelectionMode;
use beamterm_renderer::{mouse::*, select, CellData, GlyphEffect, Terminal as Beamterm, Terminal};
use bitvec::prelude::BitVec;
use compact_str::CompactString;
use ratatui::{
    backend::{ClearType, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
    prelude::Backend,
    style::{Color, Modifier},
};
use std::{
    cell::RefCell,
    io::{Error as IoError, Result as IoResult},
    mem::swap,
    rc::Rc,
};
use web_sys::{wasm_bindgen::JsCast, window, Element};

/// Re-export beamterm's atlas data type. Used by [`WebGl2BackendOptions::font_atlas`].
pub use beamterm_renderer::FontAtlasData;

// Labels used by the Performance API
const SYNC_TERMINAL_BUFFER_MARK: &str = "sync-terminal-buffer";
const WEBGL_RENDER_MARK: &str = "webgl-render";

/// Options for the [`WebGl2Backend`].
#[derive(Default, Debug)]
pub struct WebGl2BackendOptions {
    /// The element ID.
    grid_id: Option<String>,
    /// Size of the render area.
    ///
    /// Overrides the automatically detected size if set.
    size: Option<(u32, u32)>,
    /// Fallback glyph to use for characters not in the font atlas.
    fallback_glyph: Option<CompactString>,
    /// Override the default font atlas.
    font_atlas: Option<FontAtlasData>,
    /// The canvas padding color.
    canvas_padding_color: Option<Color>,
    /// The cursor shape.
    cursor_shape: CursorShape,
    /// Hyperlink click callback.
    hyperlink_callback: Option<HyperlinkCallback>,
    /// Mouse selection mode (enables text selection with mouse).
    mouse_selection_mode: Option<SelectionMode>,
    /// Measure performance using the `performance` API.
    measure_performance: bool,
    /// Enable console debugging and introspection API.
    console_debug_api: bool,
}

impl WebGl2BackendOptions {
    /// Constructs a new [`WebGl2BackendOptions`].
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the element id of the canvas' parent element.
    pub fn grid_id(mut self, id: &str) -> Self {
        self.grid_id = Some(id.into());
        self
    }

    /// Sets the size of the canvas, in pixels.
    pub fn size(mut self, size: (u32, u32)) -> Self {
        self.size = Some(size);
        self
    }

    /// Enables frame-based measurements using the
    /// [Performance](https://developer.mozilla.org/en-US/docs/Web/API/Performance) API.
    pub fn measure_performance(mut self, measure: bool) -> Self {
        self.measure_performance = measure;
        self
    }

    /// Sets the fallback glyph to use for characters not in the font atlas.
    ///
    /// If not set, defaults to a space character (` `).
    pub fn fallback_glyph(mut self, glyph: &str) -> Self {
        self.fallback_glyph = Some(glyph.into());
        self
    }

    /// Sets the canvas padding color.
    ///
    /// The padding area is the space not covered by the terminal grid.
    pub fn canvas_padding_color(mut self, color: Color) -> Self {
        self.canvas_padding_color = Some(color);
        self
    }

    /// Sets the cursor shape to use when cursor is visible.
    pub fn cursor_shape(mut self, shape: CursorShape) -> Self {
        self.cursor_shape = shape;
        self
    }

    /// Sets a custom font atlas to use for rendering.
    pub fn font_atlas(mut self, atlas: FontAtlasData) -> Self {
        self.font_atlas = Some(atlas);
        self
    }

    /// Enables mouse selection with automatic copy to clipboard on selection.
    ///
    /// Uses [`SelectionMode::Block`] for rectangular selection.
    #[deprecated(
        note = "use `enable_mouse_selection_with_mode` instead",
        since = "0.3.0"
    )]
    pub fn enable_mouse_selection(self) -> Self {
        self.enable_mouse_selection_with_mode(SelectionMode::default())
    }

    /// Enables mouse text selection with the specified selection mode.
    ///
    /// - [`SelectionMode::Block`]: Rectangular selection of cells (default)
    /// - [`SelectionMode::Linear`]: Linear selection following text flow
    pub fn enable_mouse_selection_with_mode(mut self, mode: SelectionMode) -> Self {
        self.mouse_selection_mode = Some(mode);
        self
    }

    /// Enables hyperlinks in the canvas.
    ///
    /// Sets up a default mouse handler using [`WebGl2BackendOptions::on_hyperlink_click`].
    pub fn enable_hyperlinks(self) -> Self {
        self.on_hyperlink_click(|url| {
            if let Some(w) = window() {
                w.open_with_url_and_target(url, "_blank")
                    .unwrap_or_default();
            }
        })
    }

    /// Sets a callback for when hyperlinks are clicked.
    pub fn on_hyperlink_click<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.hyperlink_callback = Some(HyperlinkCallback::new(callback));
        self
    }

    /// Gets the canvas padding color, defaulting to black if not set.
    fn get_canvas_padding_color(&self) -> u32 {
        self.canvas_padding_color
            .map(|c| to_rgb(c, 0x000000))
            .unwrap_or(0x000000)
    }

    /// Enables debug API during terminal creation.
    ///
    /// The debug api is accessible from the browser console under `window.__beamterm_debug`.
    pub fn enable_console_debug_api(mut self) -> Self {
        self.console_debug_api = true;
        self
    }
}

/// WebGl2 backend for high-performance terminal rendering.
///
/// This backend renders the terminal buffer onto an HTML canvas element using [WebGL2]
/// and the [beamterm renderer].
///
/// [WebGL2]: https://developer.mozilla.org/en-US/docs/Web/API/WebGL_API
/// [beamterm renderer]: https://crates.io/crates/beamterm-renderer
///
/// WebGL2 is supported in all modern browsers (Chrome 56+, Firefox 51+, Safari 15+).
///
/// ## Font Atlas Limitation
///
/// [`WebGl2Backend`] uses prebuilt font atlases for performance. Characters not in the atlas
/// will display as ` `. Use [`CanvasBackend`] if you need dynamic Unicode/emoji support.
///
/// [`CanvasBackend`]: crate::backend::canvas::CanvasBackend
///
/// # Performance Measurement
///
/// The backend supports built-in performance profiling using the browser's Performance API.
/// When enabled via [`WebGl2BackendOptions::measure_performance`], it tracks the duration
/// of each operation:
///
/// | Label                  | Operation                                                   |
/// |------------------------|-------------------------------------------------------------|
/// | `sync-terminal-buffer` | Synchronizes Ratatui's cell data with beamterm's            |
/// | `webgl-render`         | Flushes the GPU buffers and executes the WebGL draw call    |
///
/// ## Viewing Performance Measurements
///
/// To view the performance measurements in your browser:
///
/// 1. Enable performance measurement when creating the backend
/// 2. Open your browser's Developer Tools (F12 or Ctrl+Shift+I/J)
/// 3. Navigate to the **Performance** tab
/// 4. Collect measurements with the "Record" button, then stop recording
/// 4. Zoom in on a frame and look for the **User Timing** section which will show:
///    - Individual timing marks for each operation
///    - Duration measurements between start and end of each operation
///
/// Alternatively, in the browser console, you can query measurements:
///
/// ```javascript
/// // View all measurements
/// performance.getEntriesByType('measure')
///
/// // View specific operation
/// performance.getEntriesByName('webgl-render')
///
/// // Calculate average time for last 100 measurements
/// const avg = (name) => {
///   const entries = performance.getEntriesByName(name).slice(-100);
///   return entries.reduce((sum, e) => sum + e.duration, 0) / entries.length;
/// };
/// avg('webgl-render')
/// avg('upload-cells-to-gpu')
/// avg('sync-terminal-buffer')
/// ```
pub struct WebGl2Backend {
    /// WebGl2 terminal renderer.
    beamterm: Beamterm,
    /// The options used to create this backend.
    options: WebGl2BackendOptions,
    /// Cursor position.
    cursor_position: Option<Position>,
    /// Performance measurement.
    performance: Option<web_sys::Performance>,
    /// Hyperlink tracking.
    hyperlink_cells: Option<Rc<RefCell<BitVec>>>,
    /// Mouse handler for hyperlink clicks.
    hyperlink_mouse_handler: Option<TerminalMouseHandler>,
    /// Current cursor state over hyperlinks (shared with mouse handler).
    cursor_over_hyperlink: Option<Rc<RefCell<bool>>>,
    /// Hyperlink click callback.
    _hyperlink_callback: Option<HyperlinkCallback>,
}

impl WebGl2Backend {
    /// Constructs a new [`WebGl2Backend`].
    pub fn new() -> Result<Self, Error> {
        let (width, height) = get_raw_window_size();
        Self::new_with_size(width.into(), height.into())
    }

    /// Constructs a new [`WebGl2Backend`] with the given size.
    pub fn new_with_size(width: u32, height: u32) -> Result<Self, Error> {
        Self::new_with_options(WebGl2BackendOptions {
            size: Some((width, height)),
            ..Default::default()
        })
    }

    /// Constructs a new [`WebGl2Backend`] with the given options.
    pub fn new_with_options(mut options: WebGl2BackendOptions) -> Result<Self, Error> {
        let performance = if options.measure_performance {
            Some(performance()?)
        } else {
            None
        };

        // Parent element of canvas (uses <body> unless specified)
        let parent = get_element_by_id_or_body(options.grid_id.as_ref())?;

        let beamterm = Self::init_beamterm(&mut options, &parent)?;

        let hyperlink_cells = if options.hyperlink_callback.is_some() {
            let indices = BitVec::repeat(false, beamterm.cell_count());
            Some(Rc::new(RefCell::new(indices)))
        } else {
            None
        };

        // Extract hyperlink callback from options
        let hyperlink_callback = options.hyperlink_callback.take();

        // Initialize cursor state tracking if hyperlinks are enabled
        let cursor_over_hyperlink = if hyperlink_callback.is_some() {
            Some(Rc::new(RefCell::new(false)))
        } else {
            None
        };

        // Set up hyperlink mouse handler if callback is provided
        let hyperlink_mouse_handler = if let Some(ref callback) = hyperlink_callback {
            let hyperlink_cells = hyperlink_cells
                .clone()
                .expect("known to exist at this point");
            let cursor_state = cursor_over_hyperlink
                .clone()
                .expect("known to exist at this point");
            Some(Self::create_hyperlink_mouse_handler(
                &beamterm,
                hyperlink_cells.clone(),
                callback.callback.clone(),
                cursor_state,
            )?)
        } else {
            None
        };

        Ok(Self {
            beamterm,
            cursor_position: None,
            options,
            hyperlink_cells,
            hyperlink_mouse_handler,
            performance,
            cursor_over_hyperlink,
            _hyperlink_callback: hyperlink_callback,
        })
    }

    /// Returns the options objects used to create this backend.
    pub fn options(&self) -> &WebGl2BackendOptions {
        &self.options
    }

    /// Returns the [`CursorShape`].
    pub fn cursor_shape(&self) -> &CursorShape {
        &self.options.cursor_shape
    }

    /// Set the [`CursorShape`].
    pub fn set_cursor_shape(mut self, shape: CursorShape) -> Self {
        self.options.cursor_shape = shape;
        self
    }

    /// Sets the canvas viewport and projection, reconfigures the terminal grid.
    pub fn resize_canvas(&mut self) -> Result<(), Error> {
        let size_px = self.beamterm.canvas_size();

        // resize the terminal grid and viewport
        self.beamterm.resize(size_px.0, size_px.1)?;

        // Update mouse handler dimensions if it exists
        if let Some(mouse_handler) = &mut self.hyperlink_mouse_handler {
            let (cols, rows) = self.beamterm.terminal_size();
            mouse_handler.update_dimensions(cols, rows);
        }

        // clear any hyperlink cells; we'll get them in the next draw call
        if let Some(hyperlink_cells) = &mut self.hyperlink_cells {
            let cell_count = self.beamterm.cell_count();

            let mut hyperlink_cells = hyperlink_cells.borrow_mut();
            hyperlink_cells.clear();
            hyperlink_cells.resize(cell_count, false);
        }

        // Reset cursor state when canvas is resized
        if let Some(cursor_state) = &self.cursor_over_hyperlink {
            if let Ok(mut state) = cursor_state.try_borrow_mut() {
                *state = false;
            }
        }

        Ok(())
    }

    /// Checks if the canvas size matches the display size and resizes it if necessary.
    fn check_canvas_resize(&mut self) -> Result<(), Error> {
        let canvas = self.beamterm.canvas();
        let display_width = canvas.client_width() as u32;
        let display_height = canvas.client_height() as u32;

        let buffer_width = canvas.width();
        let buffer_height = canvas.height();

        if display_width != buffer_width || display_height != buffer_height {
            canvas.set_width(display_width);
            canvas.set_height(display_height);

            self.resize_canvas()?;
        }

        Ok(())
    }

    /// Updates the terminal grid with new cell content.
    fn update_grid<'a, I>(&mut self, content: I) -> Result<(), Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        // If enabled, measures the time taken to synchronize the terminal buffer.
        self.measure_begin(SYNC_TERMINAL_BUFFER_MARK);

        // If hyperlink support is enabled, we need to track which cells are hyperlinks,
        // before passing the content to the beamterm renderer.
        if let Some(hyperlink_cells) = self.hyperlink_cells.as_mut() {
            let w = self.beamterm.terminal_size().0 as usize;

            let mut hyperlink_cells = hyperlink_cells.borrow_mut();

            // Mark any cells that have the hyperlink modifier set (don't blink!).
            // At this stage, we don't care about the actual cell content,
            // as we can extract it on demand.
            let cells = content.inspect(|(x, y, c)| {
                let idx = *y as usize * w + *x as usize;
                let is_hyperlink = c.modifier.contains(HYPERLINK_MODIFIER);
                hyperlink_cells.set(idx, is_hyperlink);
            });
            let cells = cells.map(|(x, y, cell)| (x, y, cell_data(cell)));

            self.beamterm.update_cells_by_position(cells)
        } else {
            let cells = content.map(|(x, y, cell)| (x, y, cell_data(cell)));
            self.beamterm.update_cells_by_position(cells)
        }
        .map_err(Error::from)?;

        self.measure_end(SYNC_TERMINAL_BUFFER_MARK);

        Ok(())
    }

    /// Toggles the cursor visibility based on its current position.
    ///
    /// If there is no cursor position, it does nothing.
    fn toggle_cursor(&mut self) {
        if let Some(pos) = self.cursor_position {
            self.draw_cursor(pos);
        }
    }

    /// Draws the cursor at the specified position.
    fn draw_cursor(&mut self, pos: Position) {
        if let Some(c) = self
            .beamterm
            .grid()
            .borrow_mut()
            .cell_data_mut(pos.x, pos.y)
        {
            match self.options.cursor_shape {
                CursorShape::SteadyBlock => {
                    c.flip_colors();
                }
                CursorShape::SteadyUnderScore => {
                    // if the overall style is underlined, remove it, otherwise add it
                    c.style(c.get_style() ^ (GlyphEffect::Underline as u16));
                }
                CursorShape::None => (),
            }
        }
    }

    /// Measures the beginning of a performance mark.
    fn measure_begin(&self, label: &str) {
        if let Some(performance) = &self.performance {
            performance.mark(label).unwrap_or_default();
        }
    }

    /// Measures the end of a performance mark.
    fn measure_end(&self, label: &str) {
        if let Some(performance) = &self.performance {
            performance
                .measure_with_start_mark(label, label)
                .unwrap_or_default();
        }
    }

    /// Updates the canvas cursor style efficiently.
    fn update_canvas_cursor_style(canvas: &web_sys::HtmlCanvasElement, is_pointer: bool) {
        let cursor_value = if is_pointer { "pointer" } else { "default" };

        if let Ok(element) = canvas.clone().dyn_into::<Element>() {
            let current_style = element.get_attribute("style").unwrap_or_default();

            // Find and replace cursor property, or append if not present
            let new_style = if let Some(start) = current_style.find("cursor:") {
                // Find the end of the cursor property (either ';' or end of string)
                let after_cursor = &current_style[start..];
                let end_pos = after_cursor
                    .find(';')
                    .map(|p| p + 1)
                    .unwrap_or(after_cursor.len());
                let full_end = start + end_pos;

                format!(
                    "{}cursor: {}{}",
                    &current_style[..start],
                    cursor_value,
                    &current_style[full_end..]
                )
            } else if current_style.is_empty() {
                format!("cursor: {}", cursor_value)
            } else {
                format!(
                    "{}; cursor: {}",
                    current_style.trim_end_matches(';'),
                    cursor_value
                )
            };

            let _ = element.set_attribute("style", &new_style);
        }
    }

    /// Creates a mouse handler specifically for hyperlink clicks and hover effects.
    fn create_hyperlink_mouse_handler(
        beamterm: &Beamterm,
        hyperlink_cells: Rc<RefCell<BitVec>>,
        callback: Rc<RefCell<dyn FnMut(&str)>>,
        cursor_state: Rc<RefCell<bool>>,
    ) -> Result<TerminalMouseHandler, Error> {
        let grid = beamterm.grid();
        let canvas = beamterm.canvas();
        let hyperlink_cells_clone = hyperlink_cells.clone();
        let hyperlink_cells_move = hyperlink_cells.clone();
        let canvas_clone = canvas.clone();
        let cursor_state_clone = cursor_state.clone();

        let mouse_handler = TerminalMouseHandler::new(
            canvas,
            grid,
            move |event: TerminalMouseEvent, grid: &beamterm_renderer::TerminalGrid| {
                match event.event_type {
                    MouseEventType::MouseUp => {
                        // Handle hyperlink clicks (left mouse button only)
                        if event.button() == 0 {
                            if let Some(url) = extract_hyperlink_url(
                                hyperlink_cells_clone.clone(),
                                grid,
                                event.col,
                                event.row,
                            ) {
                                if let Ok(mut cb) = callback.try_borrow_mut() {
                                    cb(&url);
                                }
                            }
                        }
                    }
                    MouseEventType::MouseMove => {
                        // Handle cursor style changes on hover
                        let is_over_hyperlink = Self::is_over_hyperlink(
                            hyperlink_cells_move.clone(),
                            grid,
                            event.col,
                            event.row,
                        );

                        // Only update cursor style if state has changed
                        if let Ok(mut current_state) = cursor_state_clone.try_borrow_mut() {
                            if *current_state != is_over_hyperlink {
                                *current_state = is_over_hyperlink;
                                Self::update_canvas_cursor_style(&canvas_clone, is_over_hyperlink);
                            }
                        }
                    }
                    _ => {}
                }
            },
        )?;

        Ok(mouse_handler)
    }

    /// Checks if the given coordinates are over a hyperlink.
    fn is_over_hyperlink(
        hyperlink_cells: Rc<RefCell<BitVec>>,
        grid: &beamterm_renderer::TerminalGrid,
        col: u16,
        row: u16,
    ) -> bool {
        let (cols, _) = grid.terminal_size();
        let row_start_idx = row as usize * cols as usize;
        let cell_idx = row_start_idx + col as usize;

        hyperlink_cells
            .borrow()
            .get(cell_idx)
            .map(|b| *b)
            .unwrap_or(false)
    }

    /// Initializes the beamterm renderer with the given options and parent element.
    fn init_beamterm(
        options: &mut WebGl2BackendOptions,
        parent: &Element,
    ) -> Result<Terminal, Error> {
        let (width, height) = options
            .size
            .unwrap_or_else(|| (parent.client_width() as u32, parent.client_height() as u32));

        let canvas = create_canvas_in_element(parent, width, height)?;

        let beamterm = Beamterm::builder(canvas)
            .canvas_padding_color(options.get_canvas_padding_color())
            .fallback_glyph(options.fallback_glyph.as_ref().unwrap_or(&" ".into()))
            .font_atlas(options.font_atlas.take().unwrap_or_default());

        let beamterm = if let Some(mode) = options.mouse_selection_mode {
            beamterm.default_mouse_input_handler(mode, true)
        } else {
            beamterm
        };

        let beamterm = if options.console_debug_api {
            beamterm.enable_debug_api()
        } else {
            beamterm
        };

        Ok(beamterm.build()?)
    }
}

impl Backend for WebGl2Backend {
    type Error = IoError;

    // Populates the buffer with the *updated* cell content.
    fn draw<'a, I>(&mut self, content: I) -> IoResult<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        // we only update when we have new cell data or if the mouse selection
        // handler is enabled (otherwise, we fail to update the visualized selection).
        if content.size_hint().1 != Some(0) || self.options.mouse_selection_mode.is_some() {
            self.update_grid(content)?;
        }

        Ok(())
    }

    /// Flush the content to the screen.
    ///
    /// This function is called after the [`WebGl2Backend::draw`] function to
    /// actually render the content to the screen.
    fn flush(&mut self) -> IoResult<()> {
        self.check_canvas_resize()?;

        self.measure_begin(WEBGL_RENDER_MARK);

        // Flushes GPU buffers and render existing content to the canvas
        self.toggle_cursor(); // show cursor before rendering
        self.beamterm.render_frame().map_err(Error::from)?;
        self.toggle_cursor(); // restore cell to previous state

        self.measure_end(WEBGL_RENDER_MARK);

        Ok(())
    }

    fn hide_cursor(&mut self) -> IoResult<()> {
        self.cursor_position = None;
        Ok(())
    }

    fn show_cursor(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn clear(&mut self) -> IoResult<()> {
        let cells = [CellData::new_with_style_bits(" ", 0, 0xffffff, 0x000000)]
            .into_iter()
            .cycle()
            .take(self.beamterm.cell_count());

        self.beamterm.update_cells(cells).map_err(Error::from)?;

        if let Some(hyperlink_cells) = &mut self.hyperlink_cells {
            hyperlink_cells.borrow_mut().clear();
        }

        Ok(())
    }

    fn size(&self) -> IoResult<Size> {
        let (w, h) = self.beamterm.terminal_size();
        Ok(Size::new(w, h))
    }

    fn window_size(&mut self) -> IoResult<WindowSize> {
        let (cols, rows) = self.beamterm.terminal_size();
        let (w, h) = self.beamterm.canvas_size();

        Ok(WindowSize {
            columns_rows: Size::new(cols, rows),
            pixels: Size::new(w as _, h as _),
        })
    }

    fn get_cursor_position(&mut self) -> IoResult<Position> {
        match self.cursor_position {
            None => Ok((0, 0).into()),
            Some(position) => Ok(position),
        }
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> IoResult<()> {
        self.cursor_position = Some(position.into());
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(IoError::other("unimplemented")),
        }
    }
}

/// Extracts text from beamterm grid using `[get_text(CellQuery)`].
fn extract_text_from_grid(
    grid: &beamterm_renderer::TerminalGrid,
    start_col: u16,
    end_col: u16,
    row: u16,
) -> Option<String> {
    // Create a selection query for the hyperlink range
    let query = select(SelectionMode::Block)
        .start((start_col, row))
        .end((end_col, row))
        .trim_trailing_whitespace(true);

    let text = grid.get_text(query);
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Extracts hyperlink URL from grid coordinates.
fn extract_hyperlink_url(
    hyperlink_cells: Rc<RefCell<BitVec>>,
    grid: &beamterm_renderer::TerminalGrid,
    start_col: u16,
    row: u16,
) -> Option<String> {
    let hyperlink_cells = hyperlink_cells;
    let (cols, _) = grid.terminal_size();

    // Find hyperlink boundaries
    let (link_start, link_end) =
        find_hyperlink_bounds(&hyperlink_cells.borrow(), start_col, row, cols)?;

    // Extract text using beamterm's grid
    extract_text_from_grid(grid, link_start, link_end, row)
}

/// Finds the start and end boundaries of a hyperlink.
fn find_hyperlink_bounds(
    hyperlink_cells: &BitVec,
    start_col: u16,
    row: u16,
    cols: u16,
) -> Option<(u16, u16)> {
    let row_start_idx = row as usize * cols as usize;

    // Ensure clicked cell is a hyperlink
    if !hyperlink_cells
        .get(row_start_idx + start_col as usize)
        .map(|b| *b)
        .unwrap_or(false)
    {
        return None;
    }

    // Find start of hyperlink (scan left)
    let mut link_start = start_col;
    while link_start > 0 {
        let idx = row_start_idx + (link_start - 1) as usize;
        if !hyperlink_cells.get(idx).map(|b| *b).unwrap_or(false) {
            break;
        }
        link_start -= 1;
    }

    // Find end of hyperlink (scan right)
    let mut link_end = start_col;
    while link_end < cols - 1 {
        let idx = row_start_idx + (link_end + 1) as usize;
        if !hyperlink_cells.get(idx).map(|b| *b).unwrap_or(false) {
            break;
        }
        link_end += 1;
    }

    Some((link_start, link_end))
}

/// Resolves foreground and background colors for a [`Cell`].
fn resolve_fg_bg_colors(cell: &Cell) -> (u32, u32) {
    let mut fg = to_rgb(cell.fg, 0xffffff);
    let mut bg = to_rgb(cell.bg, 0x000000);

    if cell.modifier.contains(Modifier::REVERSED) {
        swap(&mut fg, &mut bg);
    }

    (fg, bg)
}

/// Converts a [`Cell`] into a [`CellData`] for the beamterm renderer.
fn cell_data(cell: &Cell) -> CellData<'_> {
    let (fg, bg) = resolve_fg_bg_colors(cell);
    CellData::new_with_style_bits(cell.symbol(), into_glyph_bits(cell.modifier), fg, bg)
}

/// Extracts glyph styling bits from cell modifiers.
///
/// # Performance Optimization
/// Bitwise operations are used instead of individual `contains()` checks.
/// This provides a ~50% performance improvement over the naive approach.
///
/// # Bit Layout Reference
///
/// ```plain
/// Modifier bits:     0000_0000_0000_0001  (BOLD at bit 0)
///                    0000_0000_0000_0100  (ITALIC at bit 2)
///                    0000_0000_0000_1000  (UNDERLINED at bit 3)
///                    0000_0001_0000_0000  (CROSSED_OUT at bit 8)
///
/// FontStyle bits:    0000_0100_0000_0000  (Bold as bit 10)
///                    0000_1000_0000_0000  (Italic as bit 11)
/// GlyphEffect bits:  0010_0000_0000_0000  (Underline at bit 13)
///                    0100_0000_0000_0000  (Strikethrough at bit 14)
///
/// Shift operations:  bit 0 << 10 = bit 10 (bold)
///                    bit 2 << 9  = bit 11 (italic)
///                    bit 3 << 10 = bit 13 (underline)
///                    bit 8 << 6  = bit 14 (strikethrough)
/// ```
const fn into_glyph_bits(modifier: Modifier) -> u16 {
    let m = modifier.bits();

    (m << 10) & (1 << 10)   // bold
    | (m << 9) & (1 << 11)  // italic
    | (m << 10) & (1 << 13) // underline
    | (m << 6) & (1 << 14) // strikethrough
}

/// A `Debug`-derive friendly convenience wrapper
#[derive(Clone)]
struct HyperlinkCallback {
    callback: Rc<RefCell<dyn FnMut(&str)>>,
}

impl HyperlinkCallback {
    /// Creates a new [`HyperlinkCallback`] with the given callback.
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        Self {
            callback: Rc::new(RefCell::new(callback)),
        }
    }
}

impl std::fmt::Debug for HyperlinkCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackWrapper")
            .field("callback", &"<callback>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beamterm_renderer::{FontStyle, GlyphEffect};
    use ratatui::style::Modifier;

    #[test]
    fn test_font_style() {
        [
            (FontStyle::Bold, Modifier::BOLD),
            (FontStyle::Italic, Modifier::ITALIC),
            (FontStyle::BoldItalic, Modifier::BOLD | Modifier::ITALIC),
        ]
        .into_iter()
        .map(|(style, modifier)| (style as u16, into_glyph_bits(modifier)))
        .for_each(|(expected, actual)| assert_eq!(expected, actual));
    }

    #[test]
    fn test_glyph_effect() {
        [
            (GlyphEffect::Underline, Modifier::UNDERLINED),
            (GlyphEffect::Strikethrough, Modifier::CROSSED_OUT),
        ]
        .into_iter()
        .map(|(effect, modifier)| (effect as u16, into_glyph_bits(modifier)))
        .for_each(|(expected, actual)| assert_eq!(expected, actual));
    }
}
