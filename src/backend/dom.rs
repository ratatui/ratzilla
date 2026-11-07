use std::{
    cell::Cell as StdCell,
    io::{Error as IoError, Result as IoResult},
    rc::Rc,
};

use ratatui::{
    backend::WindowSize,
    buffer::Cell,
    layout::{Position, Size},
    prelude::{backend::ClearType, Backend},
};
use web_sys::{window, Document, Element};

use unicode_width::UnicodeWidthStr;

use crate::{
    backend::{
        cell_sized::CellSized,
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

/// Default cell size used as a fallback when measurement fails.
const DEFAULT_CELL_SIZE: (f64, f64) = (10.0, 20.0);

/// Options for the [`DomBackend`].
#[derive(Debug, Default)]
pub struct DomBackendOptions {
    /// The element ID.
    grid_id: Option<String>,
    /// The cursor shape.
    cursor_shape: CursorShape,
}

impl DomBackendOptions {
    /// Constructs a new [`DomBackendOptions`].
    pub fn new(grid_id: Option<String>, cursor_shape: CursorShape) -> Self {
        Self {
            grid_id,
            cursor_shape,
        }
    }

    /// Returns the grid ID.
    ///
    /// - If the grid ID is not set, it returns `"grid"`.
    /// - If the grid ID is set, it returns the grid ID suffixed with
    ///     `"_ratzilla_grid"`.
    pub fn grid_id(&self) -> String {
        match &self.grid_id {
            Some(id) => format!("{id}_ratzilla_grid"),
            None => "grid".to_string(),
        }
    }

    /// Returns the [`CursorShape`].
    pub fn cursor_shape(&self) -> &CursorShape {
        &self.cursor_shape
    }
}

/// DOM backend.
///
/// This backend uses the DOM to render the content to the screen.
///
/// In other words, it transforms the [`Cell`]s into `<span>`s which are then
/// appended to a `<pre>` element.
pub struct DomBackend {
    /// Whether the backend has been initialized.
    initialized: bool,
    /// Size measured on resize, applied by the next draw.
    pending_resize: Rc<StdCell<Option<GridMeasurement>>>,
    /// Cells.
    cells: Vec<Element>,
    /// Grid element.
    grid: Element,
    /// The parent of the grid element.
    grid_parent: Element,
    /// Grid size shared with the mouse listener, so it maps clicks with the current size.
    grid_size: Rc<StdCell<Size>>,
    /// Document.
    document: Document,
    /// Options.
    options: DomBackendOptions,
    /// Cursor position.
    cursor_position: Option<Position>,
    /// Last Cursor position.
    last_cursor_position: Option<Position>,
    /// Buffer size to pass to [`ratatui::Terminal`]
    size: Size,
    /// Measured cell dimensions in pixels (width, height).
    cell_size: (f64, f64),
    /// Resize event callback handler.
    _resize_callback: EventCallback<web_sys::Event>,
    /// Mouse event callback handler.
    mouse_callback: Option<DomMouseCallbackState>,
    /// Key event callback handler.
    key_callback: Option<EventCallback<web_sys::KeyboardEvent>>,
}

/// Cell and grid size measured on resize.
#[derive(Debug, Clone, Copy)]
struct GridMeasurement {
    /// Cell dimensions in pixels (width, height).
    cell_size: (f64, f64),
    /// Grid size in cells.
    size: Size,
}

/// Type alias for mouse event callback state.
type DomMouseCallbackState = EventCallback<web_sys::MouseEvent>;

impl std::fmt::Debug for DomBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DomBackend")
            .field("initialized", &self.initialized)
            .field("pending_resize", &self.pending_resize)
            .field("cells", &format!("[{} cells]", self.cells.len()))
            .field("size", &self.size)
            .field("cell_size", &self.cell_size)
            .field("cursor_position", &self.cursor_position)
            .field("resize_callback", &"...")
            .field("mouse_callback", &self.mouse_callback.is_some())
            .field("key_callback", &self.key_callback.is_some())
            .finish()
    }
}

impl DomBackend {
    /// Constructs a new [`DomBackend`].
    pub fn new() -> Result<Self, Error> {
        Self::new_with_options(DomBackendOptions::default())
    }

    /// Constructs a new [`DomBackend`] and uses the given element ID for the grid.
    pub fn new_by_id(id: &str) -> Result<Self, Error> {
        Self::new_with_options(DomBackendOptions::new(
            Some(id.to_string()),
            CursorShape::default(),
        ))
    }

    /// Set the [`CursorShape`].
    pub fn set_cursor_shape(mut self, shape: CursorShape) -> Self {
        self.options.cursor_shape = shape;
        self
    }

    /// Constructs a new [`DomBackend`] with the given options.
    pub fn new_with_options(options: DomBackendOptions) -> Result<Self, Error> {
        let window = window().ok_or(Error::UnableToRetrieveWindow)?;
        let document = window.document().ok_or(Error::UnableToRetrieveDocument)?;
        let grid_parent = get_element_by_id_or_body(options.grid_id.as_ref())?;
        let cell_size =
            Self::measure_cell_size(&document, &grid_parent).unwrap_or(DEFAULT_CELL_SIZE);
        let size = Self::calculate_size(&grid_parent, cell_size);

        // Measured here, not in draw: ratatui calls size() before draw.
        let pending_resize = Rc::new(StdCell::new(None));
        let resize_callback = EventCallback::new(window.clone(), Self::RESIZE_EVENT_TYPES, {
            let pending_resize = pending_resize.clone();
            let document = document.clone();
            let grid_parent = grid_parent.clone();
            let grid_id = options.grid_id();
            move |_: web_sys::Event| {
                if document.get_element_by_id(&grid_id).is_some() {
                    grid_parent.set_inner_html("");
                }
                let cell_size =
                    Self::measure_cell_size(&document, &grid_parent).unwrap_or(DEFAULT_CELL_SIZE);
                let size = Self::calculate_size(&grid_parent, cell_size);
                pending_resize.set(Some(GridMeasurement { cell_size, size }));
            }
        })?;

        let mut backend = Self {
            initialized: false,
            pending_resize,
            cells: vec![],
            grid: document.create_element("div")?,
            grid_parent,
            grid_size: Rc::new(StdCell::new(size)),
            options,
            document,
            cursor_position: None,
            last_cursor_position: None,
            size,
            cell_size,
            _resize_callback: resize_callback,
            mouse_callback: None,
            key_callback: None,
        };
        backend.reset_grid()?;
        Ok(backend)
    }

    /// Measures the pixel dimensions of a single terminal cell.
    ///
    /// Creates a temporary `<pre><span>` probe element that inherits the
    /// page's CSS (font-family, font-size, etc.), measures it with
    /// `getBoundingClientRect()`, then removes the probe.
    fn measure_cell_size(document: &Document, parent: &Element) -> Result<(f64, f64), Error> {
        let pre = document.create_element("pre")?;
        pre.set_attribute(
            "style",
            "margin: 0; padding: 0; border: 0; line-height: normal;",
        )?;
        let span = document.create_element("span")?;
        span.set_inner_html("\u{2588}");
        span.set_attribute("style", "display: inline-block; width: 1ch;")?;
        pre.append_child(&span)?;
        parent.append_child(&pre)?;

        let rect = span.get_bounding_client_rect();
        let width = rect.width();
        let height = rect.height();

        parent.remove_child(&pre)?;

        if width > 0.0 && height > 0.0 {
            Ok((width, height))
        } else {
            Ok(DEFAULT_CELL_SIZE)
        }
    }

    /// Calculates the grid size in cells based on the parent element's dimensions and cell size.
    fn calculate_size(parent: &Element, cell_size: (f64, f64)) -> Size {
        let rect = parent.get_bounding_client_rect();
        let (parent_w, parent_h) = (rect.width(), rect.height());

        // Fall back to window dimensions if the parent has no size
        // (e.g. empty <body> with no explicit height)
        let (w, h) = if parent_w > 0.0 && parent_h > 0.0 {
            (parent_w, parent_h)
        } else {
            let (ww, wh) = get_raw_window_size();
            (ww as f64, wh as f64)
        };

        Size::new((w / cell_size.0) as u16, (h / cell_size.1) as u16)
    }

    /// Resize event types.
    const RESIZE_EVENT_TYPES: &[&str] = &["resize"];

    /// Reset the grid and clear the cells.
    ///
    /// The grid element itself is kept, so the key and mouse listeners
    /// attached to it keep working after a resize.
    fn reset_grid(&mut self) -> Result<(), Error> {
        self.grid.set_inner_html("");
        self.grid.set_attribute("id", &self.options.grid_id())?;
        self.cells.clear();
        Ok(())
    }

    /// Pre-render a blank content to the screen.
    ///
    /// This function is called from [`draw`] once (or after a resize)
    /// to render the right number of cells to the screen.
    fn populate(&mut self) -> Result<(), Error> {
        for _y in 0..self.size.height {
            let mut line_cells: Vec<Element> = Vec::new();
            for _x in 0..self.size.width {
                let span = create_span(&self.document, &Cell::default())?;
                self.cells.push(span.clone());
                line_cells.push(span);
            }

            // Create a <pre> element for the line
            let pre = self.document.create_element("pre")?;
            let line_height = format!("height: {}px;", self.cell_size.1);
            pre.set_attribute("style", &line_height)?;

            // Append all elements (spans and anchors) to the <pre>
            for elem in line_cells {
                pre.append_child(&elem)?;
            }

            // Append the <pre> to the grid
            self.grid.append_child(&pre)?;
        }
        Ok(())
    }
}

impl CellSized for DomBackend {
    fn cell_size_px(&self) -> (f32, f32) {
        let dpr = get_device_pixel_ratio();
        (self.cell_size.0 as f32 * dpr, self.cell_size.1 as f32 * dpr)
    }

    fn cell_size_css_px(&self) -> (f32, f32) {
        (self.cell_size.0 as f32, self.cell_size.1 as f32)
    }
}

impl Backend for DomBackend {
    type Error = IoError;

    /// Draw the new content to the screen.
    ///
    /// This function is called in the [`ratatui::Terminal::flush`] function.
    /// This function recreate the DOM structure when it gets a resize event.
    fn draw<'a, I>(&mut self, content: I) -> IoResult<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        // Only runs on resize event.
        if let Some(measured) = self.pending_resize.take() {
            self.cell_size = measured.cell_size;
            if measured.size == self.size {
                // ratatui redraws only changed cells at an unchanged size, so
                // put back the old grid, which the resize listener detached.
                let line_height = format!("height: {}px;", self.cell_size.1);
                let mut row = self.grid.first_element_child();
                while let Some(line) = row {
                    line.set_attribute("style", &line_height)
                        .map_err(Error::from)?;
                    row = line.next_element_sibling();
                }
                if self.initialized {
                    self.grid_parent
                        .append_child(&self.grid)
                        .map_err(Error::from)?;
                }
            } else {
                self.size = measured.size;
                self.grid_size.set(measured.size);
                self.reset_grid()?;
                self.initialized = false;
            }
        }

        if !self.initialized {
            self.initialized = true;

            // Clear cursor position to avoid modifying css style of a non-existent cell
            self.cursor_position = None;
            self.last_cursor_position = None;

            self.grid_parent
                .append_child(&self.grid)
                .map_err(Error::from)?;
            self.populate()?;
        }

        for (x, y, cell) in content {
            let cell_position = (y * self.size.width + x) as usize;
            let elem = &self.cells[cell_position];

            elem.set_inner_html(cell.symbol());
            elem.set_attribute("style", &get_cell_style_as_css(cell))
                .map_err(Error::from)?;

            // don't display the next cell if a fullwidth glyph preceeds it
            if cell.symbol().len() > 1 && cell.symbol().width() == 2 {
                if (cell_position + 1) < self.cells.len() {
                    let next_elem = &self.cells[cell_position + 1];
                    next_elem.set_inner_html("");
                    next_elem
                        .set_attribute("style", &get_cell_style_as_css(&Cell::new("")))
                        .map_err(Error::from)?;
                }
            }
        }

        Ok(())
    }

    /// This function is called after the [`DomBackend::draw`] function.
    ///
    /// This function does nothing because the content is directly
    /// displayed by the draw function.
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn hide_cursor(&mut self) -> IoResult<()> {
        if let Some(pos) = self.cursor_position {
            let cell_position = (pos.y * self.size.width + pos.x) as usize;

            // Use CursorShape::None to clear cursor CSS
            update_css_field(
                CursorShape::None.get_css_attribute(),
                &self.cells[cell_position],
            )
            .map_err(Error::from)?;
        }

        Ok(())
    }

    fn show_cursor(&mut self) -> IoResult<()> {
        // Remove cursor at last position
        if let Some(pos) = self.last_cursor_position {
            let cell_position = (pos.y * self.size.width + pos.x) as usize;
            update_css_field(
                CursorShape::None.get_css_attribute(),
                &self.cells[cell_position],
            )
            .map_err(Error::from)?;
        }

        // Show cursor at current position
        if let Some(pos) = self.cursor_position {
            let cell_position = (pos.y * self.size.width + pos.x) as usize;

            update_css_field(
                self.options.cursor_shape.get_css_attribute(),
                &self.cells[cell_position],
            )
            .map_err(Error::from)?;
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
        Ok(())
    }

    fn size(&self) -> IoResult<Size> {
        Ok(self
            .pending_resize
            .get()
            .map_or(self.size, |measured| measured.size))
    }

    fn window_size(&mut self) -> IoResult<WindowSize> {
        let (size, cell_size) = self
            .pending_resize
            .get()
            .map_or((self.size, self.cell_size), |measured| {
                (measured.size, measured.cell_size)
            });
        Ok(WindowSize {
            columns_rows: size,
            pixels: Size::new(
                (size.width as f64 * cell_size.0) as u16,
                (size.height as f64 * cell_size.1) as u16,
            ),
        })
    }

    fn get_cursor_position(&mut self) -> IoResult<Position> {
        match self.cursor_position {
            None => Ok((0, 0).into()),
            Some(position) => Ok(position),
        }
    }

    /// Update cursor_position and last_cursor_position
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> IoResult<()> {
        self.last_cursor_position = self.cursor_position;
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

impl WebEventHandler for DomBackend {
    fn on_mouse_event<F>(&mut self, mut callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        // Clear any existing handlers first
        self.clear_mouse_events();

        // Configure coordinate translation for DOM backend
        // Cell dimensions are derived from element dimensions / grid size
        // The size is read on each event, so clicks map correctly after a resize.
        let grid_size = self.grid_size.clone();

        // Use the grid element for coordinate calculation
        let element = self.grid.clone();

        // Create mouse event callback
        let mouse_callback = EventCallback::new(
            self.grid.clone(),
            MOUSE_EVENT_TYPES,
            move |event: web_sys::MouseEvent| {
                let size = grid_size.get();
                let config = MouseConfig::new(size.width, size.height);
                let mouse_event = create_mouse_event(&event, &element, &config);
                callback(mouse_event);
            },
        )?;

        self.mouse_callback = Some(mouse_callback);

        Ok(())
    }

    fn clear_mouse_events(&mut self) {
        self.mouse_callback = None;
    }

    fn on_key_event<F>(&mut self, mut callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        // Clear any existing handlers first
        self.clear_key_events();

        // Make the grid element focusable so it can receive key events
        self.grid.set_attribute("tabindex", "0")?;

        self.key_callback = Some(EventCallback::new(
            self.grid.clone(),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use ratatui::{widgets::Block, Terminal};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    /// Appends a parent of a fixed size, so the grid doesn't depend on the window.
    fn sized_parent(id: &str, width: u32, height: u32) -> Element {
        let document = window().unwrap().document().unwrap();
        let parent = document.create_element("div").unwrap();
        parent.set_id(id);
        set_parent_size(&parent, width, height);
        document.body().unwrap().append_child(&parent).unwrap();
        parent
    }

    fn set_parent_size(parent: &Element, width: u32, height: u32) {
        parent
            .set_attribute("style", &format!("width: {width}px; height: {height}px;"))
            .unwrap();
    }

    /// The rows and columns of the grid in the DOM.
    fn dom_grid_size(parent: &Element) -> Size {
        let grid = parent.first_element_child().unwrap();
        let rows = grid.child_element_count();
        let cols = grid.first_element_child().unwrap().child_element_count();
        Size::new(cols as u16, rows as u16)
    }

    fn fire_resize() {
        let resize = web_sys::Event::new("resize").unwrap();
        window().unwrap().dispatch_event(&resize).unwrap();
    }

    fn draw(terminal: &mut Terminal<DomBackend>) {
        terminal
            .draw(|frame| frame.render_widget(Block::bordered(), frame.area()))
            .unwrap();
    }

    #[wasm_bindgen_test]
    fn size_matches_grid() {
        let parent = sized_parent("size_matches_grid", 300, 200);
        let mut terminal =
            Terminal::new(DomBackend::new_by_id("size_matches_grid").unwrap()).unwrap();
        draw(&mut terminal);
        assert_eq!(terminal.size().unwrap(), dom_grid_size(&parent));
    }

    #[wasm_bindgen_test]
    fn size_matches_grid_after_resize() {
        let parent = sized_parent("size_matches_grid_after_resize", 300, 200);
        let mut terminal =
            Terminal::new(DomBackend::new_by_id("size_matches_grid_after_resize").unwrap())
                .unwrap();
        draw(&mut terminal);
        let before = terminal.size().unwrap();

        set_parent_size(&parent, 200, 100);
        fire_resize();
        draw(&mut terminal);

        assert_ne!(terminal.size().unwrap(), before);
        assert_eq!(terminal.size().unwrap(), dom_grid_size(&parent));
    }

    #[wasm_bindgen_test]
    fn keeps_content_after_same_size_resize() {
        let parent = sized_parent("keeps_content_after_same_size_resize", 300, 200);
        let mut terminal =
            Terminal::new(DomBackend::new_by_id("keeps_content_after_same_size_resize").unwrap())
                .unwrap();
        draw(&mut terminal);
        let drawn = parent.text_content().unwrap();

        fire_resize();
        draw(&mut terminal);

        assert_eq!(parent.text_content().unwrap(), drawn);
    }

    #[wasm_bindgen_test]
    fn rows_follow_cell_height_after_same_size_resize() {
        // Sized in `em`, so a bigger font gives bigger cells but the same grid.
        let parent = sized_parent("rows_follow_cell_height", 0, 0);
        let style = |font: u32| format!("font-size: {font}px; width: 30em; height: 20em;");
        parent.set_attribute("style", &style(16)).unwrap();
        let mut terminal =
            Terminal::new(DomBackend::new_by_id("rows_follow_cell_height").unwrap()).unwrap();
        draw(&mut terminal);
        let size = terminal.size().unwrap();
        let row_height = || {
            let grid = parent.first_element_child().unwrap();
            grid.first_element_child()
                .unwrap()
                .get_bounding_client_rect()
                .height() as f32
        };
        let before = row_height();

        parent.set_attribute("style", &style(24)).unwrap();
        fire_resize();
        draw(&mut terminal);

        assert_eq!(terminal.size().unwrap(), size);
        assert_ne!(row_height(), before);
        assert_eq!(row_height(), terminal.backend().cell_size_css_px().1);
    }

    /// Counts the key and mouse events that reach the backend's callbacks.
    fn count_events(terminal: &mut Terminal<DomBackend>) -> (Rc<StdCell<u32>>, Rc<StdCell<u32>>) {
        let keys = Rc::new(StdCell::new(0));
        let mice = Rc::new(StdCell::new(0));
        let backend = terminal.backend_mut();
        backend
            .on_key_event({
                let keys = keys.clone();
                move |_| keys.set(keys.get() + 1)
            })
            .unwrap();
        backend
            .on_mouse_event({
                let mice = mice.clone();
                move |_| mice.set(mice.get() + 1)
            })
            .unwrap();
        (keys, mice)
    }

    /// Dispatches a key and a mouse event on the grid that is in the DOM.
    fn fire_input(parent: &Element) {
        let grid = parent.first_element_child().unwrap();
        grid.dispatch_event(&web_sys::KeyboardEvent::new("keydown").unwrap())
            .unwrap();
        grid.dispatch_event(&web_sys::MouseEvent::new("mousedown").unwrap())
            .unwrap();
    }

    #[wasm_bindgen_test]
    fn mouse_uses_new_size_after_resize() {
        let parent = sized_parent("mouse_uses_new_size_after_resize", 300, 200);
        let mut terminal =
            Terminal::new(DomBackend::new_by_id("mouse_uses_new_size_after_resize").unwrap())
                .unwrap();
        let last = Rc::new(StdCell::new((u16::MAX, u16::MAX)));
        terminal
            .backend_mut()
            .on_mouse_event({
                let last = last.clone();
                move |e| last.set((e.col, e.row))
            })
            .unwrap();
        draw(&mut terminal);

        set_parent_size(&parent, 150, 100);
        fire_resize();
        draw(&mut terminal);
        let size = terminal.backend().size().unwrap();

        // Click the bottom-right corner of the grid.
        let grid = parent.first_element_child().unwrap();
        let rect = grid.get_bounding_client_rect();
        let init = web_sys::MouseEventInit::new();
        init.set_client_x((rect.right() - 1.0) as i32);
        init.set_client_y((rect.bottom() - 1.0) as i32);
        grid.dispatch_event(
            &web_sys::MouseEvent::new_with_mouse_event_init_dict("mousedown", &init).unwrap(),
        )
        .unwrap();
        assert_eq!(last.get(), (size.width - 1, size.height - 1));
    }

    #[wasm_bindgen_test]
    fn input_works_after_resize() {
        let parent = sized_parent("input_works_after_resize", 300, 200);
        let mut terminal =
            Terminal::new(DomBackend::new_by_id("input_works_after_resize").unwrap()).unwrap();
        let (keys, mice) = count_events(&mut terminal);
        draw(&mut terminal);
        fire_input(&parent);
        assert_eq!((keys.get(), mice.get()), (1, 1));

        set_parent_size(&parent, 200, 100);
        fire_resize();
        draw(&mut terminal);
        fire_input(&parent);
        assert_eq!((keys.get(), mice.get()), (2, 2));

        fire_resize();
        draw(&mut terminal);
        fire_input(&parent);
        assert_eq!((keys.get(), mice.get()), (3, 3));
    }
}
