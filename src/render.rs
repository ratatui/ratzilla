use ratatui::{prelude::Backend, Frame, Terminal};
use std::{cell::RefCell, rc::Rc};
use web_sys::{wasm_bindgen::prelude::*, window};

use crate::event::{KeyEvent, MouseEvent};

/// Trait for rendering on the web.
///
/// It provides all the necessary methods to render the terminal on the web
/// and also interact with the browser such as handling key events.
pub trait WebRenderer {
    /// Renders the terminal on the web.
    ///
    /// This method takes a closure that will be called on every update
    /// that the browser makes during [`requestAnimationFrame`] calls.
    ///
    /// TODO: Clarify and validate this.
    ///
    /// [`requestAnimationFrame`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame
    fn draw_web<F>(self, render_callback: F)
    where
        F: FnMut(&mut Frame) + 'static;

    /// Handles key events.
    ///
    /// This method takes a closure that will be called on every `keydown`
    /// event.
    fn on_key_event<F>(&self, mut callback: F)
    where
        F: FnMut(KeyEvent) + 'static,
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            callback(event.into());
        });
        let window = window().unwrap();
        let document = window.document().unwrap();
        document
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    /// Handles mouse events.
    ///
    /// This method takes a closure that will be called on every `mousemove`, 'mousedown', and `mouseup`
    /// event.
    fn on_mouse_event<F>(&self, mut callback: F)
    where
        F: FnMut(MouseEvent) + 'static,
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            callback(event.into());
        });
        let window = window().unwrap();
        let document = window.document().unwrap();
        document
            .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
            .unwrap();
        document
            .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
            .unwrap();
        document
            .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    /// Handles wheel events.
    ///
    /// This method takes a closure that will be called on every `wheel` event.
    fn on_wheel_event<F>(&self, mut callback: F)
    where
        F: FnMut(MouseEvent) + 'static,
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            callback(event.into());
        });
        let window = window().unwrap();
        let document = window.document().unwrap();
        document
            .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

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
/// This implementation creates a loop that calls the [`Terminal::draw`] method.
impl<T> WebRenderer for Terminal<T>
where
    T: Backend + 'static,
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
}
