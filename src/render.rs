use ratatui::{prelude::Backend, Frame, Terminal};
use std::{cell::RefCell, rc::Rc};
use web_sys::{wasm_bindgen::prelude::*, window};

use crate::{
    error::Error,
    event::{KeyEvent, MouseEvent},
};

/// Trait for rendering on the web.
///
/// It provides all the necessary methods to render the terminal on the web
/// and also interact with the browser such as handling key and mouse events.
pub trait WebRenderer {
    /// Renders the terminal on the web.
    ///
    /// This method takes a closure that will be called on every update
    /// that the browser makes during [`requestAnimationFrame`] calls.
    ///
    /// [`requestAnimationFrame`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame
    fn draw_web<F>(self, render_callback: F)
    where
        F: FnMut(&mut Frame) + 'static;

    /// Handles key events.
    ///
    /// This method takes a closure that will be called on every `keydown` event.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend does not support key events or if
    /// event listener attachment fails.
    fn on_key_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static;

    /// Handles mouse events.
    ///
    /// This method takes a closure that will be called on mouse events.
    /// The callback receives [`MouseEvent`]s with terminal grid coordinates
    /// (`col`, `row`) instead of raw pixel coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error if event listener attachment fails.
    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static;

    /// Requests an animation frame.
    fn request_animation_frame(f: &Closure<dyn FnMut()>) {
        window()
            .unwrap()
            .request_animation_frame(f.as_ref().unchecked_ref())
            .unwrap();
    }
}

/// Implement [`WebRenderer`] for Ratatui's [`Terminal`].
///
/// This implementation delegates event handling to the backend's
/// [`WebEventHandler`] implementation.
impl<T> WebRenderer for Terminal<T>
where
    T: Backend + WebEventHandler + 'static,
{
    fn draw_web<F>(mut self, mut render_callback: F)
    where
        F: FnMut(&mut Frame) + 'static,
    {
        let callback = Rc::new(RefCell::new(None));
        *callback.borrow_mut() = Some(Closure::wrap(Box::new({
            let cb = callback.clone();
            move || {
                self.draw(|frame| {
                    render_callback(frame);
                })
                .unwrap();
                Self::request_animation_frame(cb.borrow().as_ref().unwrap());
            }
        }) as Box<dyn FnMut()>));
        Self::request_animation_frame(callback.borrow().as_ref().unwrap());
    }

    fn on_key_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        self.backend_mut().on_key_event(callback)
    }

    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        self.backend_mut().on_mouse_event(callback)
    }
}

/// Backend-specific event handling with lifecycle management.
///
/// This trait provides proper event handling for terminal backends, including:
/// - Coordinate translation from pixels to terminal grid positions
/// - Automatic cleanup of event listeners when replaced or dropped
/// - Extended mouse event support (enter/leave, click/dblclick)
///
/// # Example
///
/// ```no_run
/// use ratzilla::{CanvasBackend, WebRenderer};
/// use ratatui::Terminal;
///
/// let mut terminal = Terminal::new(CanvasBackend::new().unwrap()).unwrap();
///
/// // Set up mouse events with grid coordinate translation
/// terminal.on_mouse_event(|event| {
///     // event.col and event.row are terminal grid coordinates
///     println!("Mouse at ({}, {})", event.col, event.row);
/// }).unwrap();
/// ```
pub trait WebEventHandler {
    /// Sets up mouse event handlers with coordinate translation.
    ///
    /// The callback receives [`MouseEvent`]s with terminal grid coordinates
    /// (`col`, `row`) instead of raw pixel coordinates. Coordinates are
    /// relative to the terminal element, not the viewport.
    ///
    /// Calling this method again will automatically clean up the previous
    /// event listeners before setting up new ones.
    ///
    /// # Errors
    ///
    /// Returns an error if event listener attachment fails.
    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static;

    /// Removes all mouse event handlers.
    ///
    /// This is automatically called when new handlers are set up, but can be
    /// called manually to stop receiving mouse events.
    fn clear_mouse_events(&mut self);

    /// Sets up keyboard event handlers.
    ///
    /// The callback receives [`KeyEvent`]s for `keydown` events.
    ///
    /// Calling this method again will automatically clean up the previous
    /// event listeners before setting up new ones.
    ///
    /// **Note**: Some backends (e.g., [`WebGl2Backend`]) do not support key events
    /// and will silently succeed without registering any handlers.
    ///
    /// # Errors
    ///
    /// Returns an error if event listener attachment fails.
    ///
    /// [`WebGl2Backend`]: crate::WebGl2Backend
    fn on_key_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static;

    /// Removes all keyboard event handlers.
    ///
    /// This is automatically called when new handlers are set up, but can be
    /// called manually to stop receiving key events.
    fn clear_key_events(&mut self);
}
