use std::{
    cell::RefCell,
    io::{Error as IoError, Result as IoResult},
    rc::Rc,
};

use ratatui::{
    backend::WindowSize,
    buffer::Cell,
    layout::{Position, Size},
    prelude::{backend::ClearType, Backend},
};
use web_sys::{
    wasm_bindgen::{prelude::Closure, JsCast},
    window, Document, Element, Window,
};

use unicode_width::UnicodeWidthStr;

use crate::{backend::utils::*, error::Error, CursorShape};

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
#[derive(Debug)]
pub struct DomBackend {
    /// Whether the backend has been initialized.
    initialized: Rc<RefCell<bool>>,
    /// Cells.
    cells: Vec<Element>,
    /// Grid element.
    grid: Element,
    /// The parent of the grid element.
    grid_parent: Element,
    /// Window.
    window: Window,
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
        let mut backend = Self {
            initialized: Rc::new(RefCell::new(false)),
            cells: vec![],
            grid: document.create_element("div")?,
            grid_parent: get_element_by_id_or_body(options.grid_id.as_ref())?,
            options,
            window,
            document,
            cursor_position: None,
            last_cursor_position: None,
            size: get_size(),
        };
        backend.add_on_resize_listener();
        backend.reset_grid()?;
        Ok(backend)
    }

    /// Add a listener to the window resize event.
    fn add_on_resize_listener(&mut self) {
        let initialized = self.initialized.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_: web_sys::Event| {
            initialized.replace(false);
        });
        self.window
            .set_onresize(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    /// Reset the grid and clear the cells.
    fn reset_grid(&mut self) -> Result<(), Error> {
        self.grid = self.document.create_element("div")?;
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
            pre.set_attribute("style", "height: 15px;")?;

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
        if !*self.initialized.borrow() {
            self.initialized.replace(true);

            // Clear cursor position to avoid modifying css style of a non-existent cell
            self.cursor_position = None;
            self.last_cursor_position = None;

            // Only runs on resize event.
            if self
                .document
                .get_element_by_id(&self.options.grid_id())
                .is_some()
            {
                self.grid_parent.set_inner_html("");
                self.reset_grid()?;

                // update size
                self.size = get_size();
            }

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
        Ok(Size::new(
            self.size.width.saturating_sub(1),
            self.size.height.saturating_sub(1),
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
