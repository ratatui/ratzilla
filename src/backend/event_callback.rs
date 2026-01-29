//! Event callback management with automatic cleanup.
//!
//! This module provides utilities for managing web event listeners with proper
//! lifecycle management and coordinate translation for mouse events.

use std::fmt::Formatter;
use web_sys::{
    wasm_bindgen::{convert::FromWasmAbi, prelude::Closure, JsCast},
    Element,
};

use crate::{
    error::Error,
    event::{MouseButton, MouseEvent, MouseEventKind},
};

/// Manages web event listeners with automatic cleanup.
///
/// When this struct is dropped, all registered event listeners are removed
/// from the element, preventing memory leaks.
pub(super) struct EventCallback<T: 'static> {
    /// The event types this callback is registered for.
    event_types: &'static [&'static str],
    /// The element the listeners are attached to.
    element: Element,
    /// The closure that handles the events.
    #[allow(dead_code)]
    closure: Closure<dyn FnMut(T)>,
}

impl<T: 'static> EventCallback<T> {
    /// Creates a new [`EventCallback`] and attaches listeners to the element.
    pub fn new<F>(
        element: Element,
        event_types: &'static [&'static str],
        callback: F,
    ) -> Result<Self, Error>
    where
        F: FnMut(T) + 'static,
        T: JsCast + FromWasmAbi,
    {
        let closure = Closure::<dyn FnMut(T)>::new(callback);

        for event_type in event_types {
            element
                .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())?;
        }

        Ok(Self {
            event_types,
            element,
            closure,
        })
    }
}

impl<T: 'static> Drop for EventCallback<T> {
    fn drop(&mut self) {
        for event_type in self.event_types {
            let _ = self.element.remove_event_listener_with_callback(
                event_type,
                self.closure.as_ref().unchecked_ref(),
            );
        }
    }
}

impl<T: 'static> std::fmt::Debug for EventCallback<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventCallback")
            .field("event_types", &self.event_types)
            .field("element", &self.element)
            .finish()
    }
}

/// Configuration for mouse coordinate transformation.
///
/// This struct holds the information needed to translate raw pixel coordinates
/// from mouse events into terminal grid coordinates.
#[derive(Debug, Clone)]
pub(super) struct MouseConfig {
    /// Terminal grid width in characters.
    pub grid_width: u16,
    /// Terminal grid height in characters.
    pub grid_height: u16,
    /// Pixel offset from the element edge (e.g., canvas padding/translation).
    pub offset: Option<f64>,
    /// Cell dimensions in pixels (width, height).
    /// If provided, used for pixel-perfect coordinate calculation.
    pub cell_dimensions: Option<(f64, f64)>,
}

impl MouseConfig {
    /// Creates a new [`MouseConfig`] with the given grid dimensions.
    pub fn new(grid_width: u16, grid_height: u16) -> Self {
        Self {
            grid_width,
            grid_height,
            offset: None,
            cell_dimensions: None,
        }
    }

    /// Sets the pixel offset from the element edge.
    pub fn with_offset(mut self, offset: f64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Sets the cell dimensions in pixels.
    pub fn with_cell_dimensions(mut self, width: f64, height: f64) -> Self {
        self.cell_dimensions = Some((width, height));
        self
    }
}

/// The event types for keyboard events.
pub(super) const KEY_EVENT_TYPES: &[&str] = &["keydown"];

/// Mouse event types (excluding wheel which needs special handling).
pub(super) const MOUSE_EVENT_TYPES: &[&str] = &[
    "mousemove",
    "mousedown",
    "mouseup",
    "click",
    "dblclick",
    "mouseenter",
    "mouseleave",
];

/// Translates mouse event pixel coordinates to terminal grid coordinates.
///
/// This function calculates the grid position (col, row) from raw pixel
/// coordinates, taking into account element positioning and optional offsets.
pub(super) fn mouse_to_grid_coords(
    event: &web_sys::MouseEvent,
    element: &Element,
    config: &MouseConfig,
) -> (u16, u16) {
    let rect = element.get_bounding_client_rect();

    // Calculate relative position within element
    let offset = config.offset.unwrap_or(0.0);
    let relative_x = (event.client_x() as f64 - rect.left() - offset).max(0.0);
    let relative_y = (event.client_y() as f64 - rect.top() - offset).max(0.0);

    // Calculate drawable area
    let (drawable_width, drawable_height) = match config.cell_dimensions {
        Some((cw, ch)) => (
            config.grid_width as f64 * cw,
            config.grid_height as f64 * ch,
        ),
        None => (rect.width() - 2.0 * offset, rect.height() - 2.0 * offset),
    };

    // Avoid division by zero
    if drawable_width <= 0.0 || drawable_height <= 0.0 {
        return (0, 0);
    }

    // Map to grid coordinates
    let col = ((relative_x / drawable_width) * config.grid_width as f64) as u16;
    let row = ((relative_y / drawable_height) * config.grid_height as f64) as u16;

    // Clamp to bounds
    (
        col.min(config.grid_width.saturating_sub(1)),
        row.min(config.grid_height.saturating_sub(1)),
    )
}

/// Converts a web_sys::MouseEvent type string to a MouseEventKind.
pub(super) fn event_type_to_kind(event_type: &str, button: MouseButton) -> MouseEventKind {
    match event_type {
        "mousemove" => MouseEventKind::Moved,
        "mousedown" => MouseEventKind::ButtonDown(button),
        "mouseup" => MouseEventKind::ButtonUp(button),
        "click" => MouseEventKind::SingleClick(button),
        "dblclick" => MouseEventKind::DoubleClick(button),
        "mouseenter" => MouseEventKind::Entered,
        "mouseleave" => MouseEventKind::Exited,
        _ => MouseEventKind::Unidentified,
    }
}

/// Creates a MouseEvent from web_sys events with coordinate translation.
pub(super) fn create_mouse_event(
    event: &web_sys::MouseEvent,
    element: &Element,
    config: &MouseConfig,
) -> MouseEvent {
    let (col, row) = mouse_to_grid_coords(event, element, config);
    let button: MouseButton = event.button().into();
    let event_type = event.type_();

    MouseEvent {
        kind: event_type_to_kind(&event_type, button),
        col,
        row,
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_config_builder() {
        let config = MouseConfig::new(80, 24)
            .with_offset(5.0)
            .with_cell_dimensions(10.0, 19.0);

        assert_eq!(config.grid_width, 80);
        assert_eq!(config.grid_height, 24);
        assert_eq!(config.offset, Some(5.0));
        assert_eq!(config.cell_dimensions, Some((10.0, 19.0)));
    }
}
