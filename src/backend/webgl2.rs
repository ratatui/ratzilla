use crate::{
    backend::{color::to_rgb, utils::*},
    error::Error,
    event::{KeyEvent, MouseEvent},
    render::WebEventHandler,
    CursorShape,
};
pub use beamterm_renderer::SelectionMode;
use beamterm_renderer::{
    mouse::*, CellData, CursorPosition, GlyphEffect, Terminal as Beamterm, Terminal,
};
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

/// Re-export beamterm's atlas data type. Used by [`FontAtlasConfig::Static`].
pub use beamterm_renderer::FontAtlasData;

/// Font atlas configuration.
#[derive(Debug)]
pub enum FontAtlasConfig {
    /// Static pre-generated font atlas.
    Static(FontAtlasData),
    /// Dynamic font atlas with runtime font selection.
    ///
    /// The tuple contains: (font_family, font_size)
    Dynamic(Vec<String>, f32),
}

impl FontAtlasConfig {
    /// Constructs a new [`FontAtlasConfig::Dynamic`]. The font family string should be
    /// the same as the font family name in the CSS font-family property.
    pub fn dynamic(font_family: &[&str], font_size: f32) -> Self {
        Self::Dynamic(
            font_family.iter().map(|s| s.to_string()).collect(),
            font_size,
        )
    }
}

/// Pending hyperlink mouse events, communicated from the mouse handler to
/// [`WebGl2Backend::process_hyperlink_events`].
#[derive(Clone, Copy, Default)]
struct PendingHyperlinkEvent {
    hover: Option<(u16, u16)>,
    click: Option<(u16, u16)>,
}

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
    /// Font atlas configuration (static or dynamic).
    font_atlas_config: Option<FontAtlasConfig>,
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
    /// Disable automatic canvas CSS sizing (let external CSS control dimensions).
    disable_auto_css_resize: bool,
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

    /// Sets the fallback glyph for missing characters.
    ///
    /// Used when a glyph is missing from the font atlas. Defaults to a space character.
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

    /// Sets a custom static font atlas to use for rendering.
    ///
    /// Static atlases are pre-generated using the beamterm-atlas CLI tool and
    /// loaded from binary .atlas files.
    #[deprecated(
        note = "use `font_atlas_config(FontAtlasConfig::Static(atlas))` instead",
        since = "0.3.0"
    )]
    pub fn font_atlas(self, atlas: FontAtlasData) -> Self {
        self.font_atlas_config(FontAtlasConfig::Static(atlas))
    }

    /// Sets a custom font atlas configuration (static or dynamic).
    /// Defaults to the static font atlas that ships with beamterm.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratzilla::backend::webgl2::{WebGl2BackendOptions, FontAtlasConfig};
    /// use ratzilla::backend::webgl2::FontAtlasData;
    ///
    /// // Static atlas
    /// let options = WebGl2BackendOptions::new()
    ///     .font_atlas_config(FontAtlasConfig::Static(FontAtlasData::default()));
    ///
    /// // Dynamic atlas
    /// let options = WebGl2BackendOptions::new()
    ///     .font_atlas_config(FontAtlasConfig::dynamic(
    ///         // monospace is an implicit fallback font in browsers
    ///         &["JetBrains Mono"],
    ///         16.0
    ///     ));
    /// ```
    pub fn font_atlas_config(mut self, config: FontAtlasConfig) -> Self {
        self.font_atlas_config = Some(config);
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

    /// Disables automatic canvas CSS sizing for CSS-controlled layouts.
    ///
    /// When called, the renderer does not set inline CSS `width` and `height`
    /// properties on the canvas, allowing external CSS rules (flexbox, grid,
    /// percentages) to control the canvas display size. The canvas buffer is
    /// still sized correctly for crisp HiDPI rendering.
    ///
    /// Use this when the canvas should fill its container based on CSS layout
    /// rules rather than having a fixed pixel size.
    pub fn disable_auto_css_resize(mut self) -> Self {
        self.disable_auto_css_resize = true;
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
/// ## Font Atlas Options
///
/// [`WebGl2Backend`] supports two font atlas modes via [`FontAtlasConfig`]:
///
/// - **Dynamic**: Rasterizes glyphs on demand with full Unicode/emoji and font variant support.
/// - **Static** (default): Uses pre-generated `.atlas` files. The default atlas is embedded
///   in beamterm. Characters not in the atlas display as the fallback glyph (space by default).
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
    /// Mouse handler for hyperlink clicks.
    _hyperlink_mouse_handler: Option<TerminalMouseHandler>,
    /// Whether cursor is currently over a hyperlink.
    cursor_over_hyperlink: bool,
    /// Hyperlink click callback.
    hyperlink_callback: Option<HyperlinkCallback>,
    /// Shared state for deferred hyperlink processing in [`WebGl2Backend::flush`].
    hyperlink_state: Option<Rc<std::cell::Cell<PendingHyperlinkEvent>>>,
    /// User-provided mouse event handler.
    _user_mouse_handler: Option<TerminalMouseHandler>,
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

        // Extract hyperlink callback from options
        let hyperlink_callback = options.hyperlink_callback.take();

        // Set up hyperlink mouse handler and shared state if callback is provided
        let (hyperlink_mouse_handler, hyperlink_state) = if hyperlink_callback.is_some() {
            let state = Rc::new(std::cell::Cell::new(PendingHyperlinkEvent::default()));
            let handler = Self::create_hyperlink_mouse_handler(&beamterm, state.clone())?;
            (Some(handler), Some(state))
        } else {
            (None, None)
        };

        Ok(Self {
            beamterm,
            cursor_position: None,
            options,
            _hyperlink_mouse_handler: hyperlink_mouse_handler,
            performance,
            cursor_over_hyperlink: false,
            hyperlink_callback,
            hyperlink_state,
            _user_mouse_handler: None,
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

    /// Resizes the terminal to match the current CSS display size of the canvas.
    ///
    /// This method reads the canvas's CSS dimensions and updates beamterm's
    /// internal state, viewport, and grid layout accordingly.
    pub fn resize_canvas(&mut self) -> Result<(), Error> {
        let width = self.beamterm.canvas().client_width();
        let height = self.beamterm.canvas().client_height();

        // resize the terminal grid and viewport
        self.beamterm.resize(width, height)?;

        // Reset hyperlink cursor state when canvas is resized
        self.cursor_over_hyperlink = false;

        Ok(())
    }

    /// Checks if the canvas size matches the display size and resizes it if necessary.
    fn check_canvas_resize(&mut self) -> Result<(), Error> {
        // Compare CSS display size against beamterm's stored logical size.
        // Both are in CSS/logical pixels, so DPR scaling is handled correctly
        // by beamterm internally (buffer = logical × DPR).
        let display_width = self.beamterm.canvas().client_width();
        let display_height = self.beamterm.canvas().client_height();
        let (stored_width, stored_height) = self.beamterm.canvas_size();

        if display_width != stored_width || display_height != stored_height {
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

        let cells = content.map(|(x, y, cell)| (x, y, cell_data(cell)));
        self.beamterm
            .update_cells_by_position(cells)
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

    /// Creates a mouse handler that records hyperlink-relevant mouse events
    /// for deferred processing in [`WebGl2Backend::process_hyperlink_events`].
    fn create_hyperlink_mouse_handler(
        beamterm: &Beamterm,
        hyperlink_state: Rc<std::cell::Cell<PendingHyperlinkEvent>>,
    ) -> Result<TerminalMouseHandler, Error> {
        let grid = beamterm.grid();
        let canvas = beamterm.canvas();

        let mouse_handler = TerminalMouseHandler::new(
            canvas,
            grid,
            move |event: TerminalMouseEvent, _grid: &beamterm_renderer::TerminalGrid| {
                let mut state = hyperlink_state.get();
                match event.event_type {
                    MouseEventType::MouseUp if event.button() == 0 => {
                        state.click = Some((event.col, event.row));
                    }
                    MouseEventType::MouseMove => {
                        state.hover = Some((event.col, event.row));
                    }
                    _ => return,
                }
                hyperlink_state.set(state);
            },
        )?;

        Ok(mouse_handler)
    }

    /// Processes pending hyperlink events using [`Beamterm::find_url_at`].
    ///
    /// Called during [`WebGl2Backend::flush`] where `self.beamterm` is accessible.
    fn process_hyperlink_events(&mut self) {
        let state = match self.hyperlink_state.clone() {
            Some(state) => state,
            None => return,
        };

        let mut pending = state.get();

        // Process pending click
        if let Some((col, row)) = pending.click {
            pending.click = None;
            if let Some(url_match) = self.beamterm.find_url_at(CursorPosition::new(col, row)) {
                if let Some(ref callback) = self.hyperlink_callback {
                    if let Ok(mut cb) = callback.callback.try_borrow_mut() {
                        cb(&url_match.url);
                    }
                }
            }
        }

        // Update cursor style on hover
        if let Some((col, row)) = pending.hover {
            let is_over = self
                .beamterm
                .find_url_at(CursorPosition::new(col, row))
                .is_some();
            if self.cursor_over_hyperlink != is_over {
                self.cursor_over_hyperlink = is_over;
                Self::update_canvas_cursor_style(&self.beamterm.canvas(), is_over);
            }
        }

        state.set(pending);
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

        let mut beamterm = Beamterm::builder(canvas)
            .canvas_padding_color(options.get_canvas_padding_color())
            .fallback_glyph(options.fallback_glyph.as_ref().unwrap_or(&" ".into()));

        // Configure font atlas (static or dynamic)
        beamterm = match options.font_atlas_config.take() {
            Some(FontAtlasConfig::Dynamic(font_family, font_size)) => {
                let font_family_refs: Vec<&str> = font_family.iter().map(|s| s.as_str()).collect();
                beamterm.dynamic_font_atlas(&font_family_refs, font_size)
            }
            Some(FontAtlasConfig::Static(atlas)) => beamterm.font_atlas(atlas),
            None => beamterm.font_atlas(FontAtlasData::default()),
        };

        let beamterm = if let Some(mode) = options.mouse_selection_mode {
            beamterm.mouse_selection_handler(
                MouseSelectOptions::new()
                    .selection_mode(mode)
                    .trim_trailing_whitespace(true),
            )
        } else {
            beamterm
        };

        let beamterm = if options.console_debug_api {
            beamterm.enable_debug_api()
        } else {
            beamterm
        };

        let beamterm = beamterm.auto_resize_canvas_css(!options.disable_auto_css_resize);

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
        self.process_hyperlink_events();
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

/// Mouse event handling for [`WebGl2Backend`].
///
/// This implementation delegates to beamterm's [`TerminalMouseHandler`], which provides
/// native grid coordinate translation. However, beamterm only supports a subset of
/// mouse events:
///
/// | Supported | Event Type |
/// |-----------|------------|
/// | ✓ | [`MouseEventKind::Moved`] |
/// | ✓ | [`MouseEventKind::ButtonDown`] |
/// | ✓ | [`MouseEventKind::ButtonUp`] |
/// | ✗ | [`MouseEventKind::SingleClick`] |
/// | ✗ | [`MouseEventKind::DoubleClick`] |
/// | ✗ | [`MouseEventKind::Entered`] |
/// | ✗ | [`MouseEventKind::Exited`] |
///
/// For full mouse event support, consider using [`CanvasBackend`] or [`DomBackend`].
///
/// **Note**: Keyboard events are not supported by this backend.
///
/// [`CanvasBackend`]: crate::CanvasBackend
/// [`DomBackend`]: crate::DomBackend
/// [`MouseEventKind::Moved`]: crate::event::MouseEventKind::Moved
/// [`MouseEventKind::ButtonDown`]: crate::event::MouseEventKind::ButtonDown
/// [`MouseEventKind::ButtonUp`]: crate::event::MouseEventKind::ButtonUp
/// [`MouseEventKind::SingleClick`]: crate::event::MouseEventKind::SingleClick
/// [`MouseEventKind::DoubleClick`]: crate::event::MouseEventKind::DoubleClick
/// [`MouseEventKind::Entered`]: crate::event::MouseEventKind::Entered
/// [`MouseEventKind::Exited`]: crate::event::MouseEventKind::Exited
impl WebEventHandler for WebGl2Backend {
    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        // Clear any existing handlers first
        self.clear_mouse_events();

        let grid = self.beamterm.grid();
        let canvas = self.beamterm.canvas();

        // Wrap the callback in Rc<RefCell> for sharing
        let callback = Rc::new(RefCell::new(callback));
        let callback_clone = callback.clone();

        // Create a TerminalMouseHandler that delegates to our callback
        let mouse_handler = TerminalMouseHandler::new(
            canvas,
            grid,
            move |event: TerminalMouseEvent, _grid: &beamterm_renderer::TerminalGrid| {
                let mouse_event = beamterm_event_to_mouse_event(&event);
                if let Ok(mut cb) = callback_clone.try_borrow_mut() {
                    cb(mouse_event);
                }
            },
        )?;

        self._user_mouse_handler = Some(mouse_handler);

        Ok(())
    }

    fn clear_mouse_events(&mut self) {
        self._user_mouse_handler = None;
    }

    fn on_key_event<F>(&mut self, _callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        // Key events are not supported for WebGl2Backend.
        // The canvas would need to be made focusable and handle keyboard events.
        // We silently succeed here so apps can use the same code for all backends.
        Ok(())
    }

    fn clear_key_events(&mut self) {
        // No-op for WebGl2Backend since key events aren't supported
    }
}

/// Converts a beamterm `TerminalMouseEvent` to our `MouseEvent` type.
fn beamterm_event_to_mouse_event(event: &TerminalMouseEvent) -> MouseEvent {
    use crate::event::{MouseButton, MouseEventKind};

    let button = match event.button() {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        _ => MouseButton::Unidentified,
    };

    // beamterm only provides MouseMove, MouseDown, and MouseUp events
    let kind = match event.event_type {
        MouseEventType::MouseMove => MouseEventKind::Moved,
        MouseEventType::MouseDown => MouseEventKind::ButtonDown(button),
        MouseEventType::MouseUp => MouseEventKind::ButtonUp(button),
    };

    MouseEvent {
        kind,
        col: event.col,
        row: event.row,
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
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
