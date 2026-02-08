/// A key event.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Whether the control key is pressed.
    pub ctrl: bool,
    /// Whether the alt key is pressed.
    pub alt: bool,
    /// Whether the shift key is pressed.
    pub shift: bool,
}

/// A mouse movement event.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MouseEvent {
    /// The mouse button that was pressed.
    pub button: MouseButton,
    /// The triggered event.
    pub event: MouseEventKind,
    /// The x coordinate of the mouse.
    pub x: u32,
    /// The y coordinate of the mouse.
    pub y: u32,
    /// Whether the control key is pressed.
    pub ctrl: bool,
    /// Whether the alt key is pressed.
    pub alt: bool,
    /// Whether the shift key is pressed.
    pub shift: bool,
}

/// Convert a [`web_sys::KeyboardEvent`] to a [`KeyEvent`].
impl From<web_sys::KeyboardEvent> for KeyEvent {
    fn from(event: web_sys::KeyboardEvent) -> Self {
        let ctrl = event.ctrl_key();
        let alt = event.alt_key();
        let shift = event.shift_key();
        KeyEvent {
            code: event.into(),
            ctrl,
            alt,
            shift,
        }
    }
}

/// A key code.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum KeyCode {
    /// Normal letter key input.
    Char(char),
    /// F keys.
    F(u8),
    /// Backspace key
    Backspace,
    /// Enter or return key
    Enter,
    /// Left arrow key
    Left,
    /// Right arrow key
    Right,
    /// Up arrow key
    Up,
    /// Down arrow key
    Down,
    /// Tab key
    Tab,
    /// Delete key
    Delete,
    /// Home key
    Home,
    /// End key
    End,
    /// Page up key
    PageUp,
    /// Page down key
    PageDown,
    /// Escape key
    Esc,
    /// Unidentified.
    Unidentified,
}

/// Convert a [`web_sys::KeyboardEvent`] to a [`KeyCode`].
impl From<web_sys::KeyboardEvent> for KeyCode {
    fn from(event: web_sys::KeyboardEvent) -> Self {
        let key = event.key();
        if key.len() == 1 {
            let char = key.chars().next();
            if let Some(char) = char {
                return KeyCode::Char(char);
            } else {
                return KeyCode::Unidentified;
            }
        }
        match key.as_str() {
            "F1" => KeyCode::F(1),
            "F2" => KeyCode::F(2),
            "F3" => KeyCode::F(3),
            "F4" => KeyCode::F(4),
            "F5" => KeyCode::F(5),
            "F6" => KeyCode::F(6),
            "F7" => KeyCode::F(7),
            "F8" => KeyCode::F(8),
            "F9" => KeyCode::F(9),
            "F10" => KeyCode::F(10),
            "F11" => KeyCode::F(11),
            "F12" => KeyCode::F(12),
            "Backspace" => KeyCode::Backspace,
            "Enter" => KeyCode::Enter,
            "ArrowLeft" => KeyCode::Left,
            "ArrowRight" => KeyCode::Right,
            "ArrowUp" => KeyCode::Up,
            "ArrowDown" => KeyCode::Down,
            "Tab" => KeyCode::Tab,
            "Delete" => KeyCode::Delete,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "PageUp" => KeyCode::PageUp,
            "PageDown" => KeyCode::PageDown,
            "Escape" => KeyCode::Esc,
            _ => KeyCode::Unidentified,
        }
    }
}

/// A mouse button.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button
    Middle,
    /// Back mouse button
    Back,
    /// Forward mouse button
    Forward,
    /// Unidentified mouse button
    Unidentified,
}

/// Scroll delta with the original delta mode from the browser.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScrollDelta {
    /// Delta in pixels
    Pixels(i32),
    /// Delta in lines (typically represents wheel notches)
    Lines(i32),
    /// Delta in pages
    Pages(i32),
}

impl ScrollDelta {
    /// DOM_DELTA_PIXEL: The units of measurement for the delta are pixels.
    const DOM_DELTA_PIXEL: u32 = 0;
    /// DOM_DELTA_LINE: The units of measurement for the delta are individual lines of text.
    const DOM_DELTA_LINE: u32 = 1;
    /// DOM_DELTA_PAGE: The units of measurement for the delta are pages.
    const DOM_DELTA_PAGE: u32 = 2;

    /// Normalize the scroll delta to discrete wheel steps/clicks.
    ///
    /// This converts the delta to an approximate number of wheel notches:
    /// - Lines: Already represent wheel notches, returned as-is
    /// - Pixels: Divided by ~100 pixels per notch
    /// - Pages: Multiplied by ~10 notches per page
    pub fn to_steps(self) -> i32 {
        match self {
            ScrollDelta::Pixels(px) => px / 100,
            ScrollDelta::Lines(lines) => lines,
            ScrollDelta::Pages(pages) => pages * 10,
        }
    }
}

/// A mouse event.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MouseEventKind {
    /// Mouse moved
    Moved,
    /// Mouse button pressed
    Pressed,
    /// Mouse button released
    Released,
    /// Mouse scrolled vertically (positive = down, negative = up)
    ScrolledVertical(ScrollDelta),
    /// Mouse scrolled horizontally (positive = right, negative = left)
    ScrolledHorizontal(ScrollDelta),
    /// Unidentified mouse event
    Unidentified,
}

/// Convert a [`web_sys::MouseEvent`] to a [`MouseEvent`].
impl From<web_sys::MouseEvent> for MouseEvent {
    fn from(event: web_sys::MouseEvent) -> Self {
        let ctrl = event.ctrl_key();
        let alt = event.alt_key();
        let shift = event.shift_key();
        let event_type = event.type_().into();
        MouseEvent {
            // Button is only valid if it is a mousedown or mouseup event.
            button: if event_type == MouseEventKind::Moved {
                MouseButton::Unidentified
            } else {
                event.button().into()
            },
            event: event_type,
            x: event.client_x() as u32,
            y: event.client_y() as u32,
            ctrl,
            alt,
            shift,
        }
    }
}

/// Convert a [`web_sys::MouseEvent`] to a [`MouseButton`].
impl From<i16> for MouseButton {
    fn from(button: i16) -> Self {
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            3 => MouseButton::Back,
            4 => MouseButton::Forward,
            _ => MouseButton::Unidentified,
        }
    }
}

/// Convert a [`web_sys::MouseEvent`] to a [`MouseEventKind`].
impl From<String> for MouseEventKind {
    fn from(event: String) -> Self {
        let event = event.as_str();
        match event {
            "mousemove" => MouseEventKind::Moved,
            "mousedown" => MouseEventKind::Pressed,
            "mouseup" => MouseEventKind::Released,
            _ => MouseEventKind::Unidentified,
        }
    }
}

/// Convert a [`web_sys::WheelEvent`] to a [`MouseEvent`].
impl From<web_sys::WheelEvent> for MouseEvent {
    fn from(event: web_sys::WheelEvent) -> Self {
        let ctrl = event.ctrl_key();
        let alt = event.alt_key();
        let shift = event.shift_key();
        let delta_mode = event.delta_mode();
        let delta_x = event.delta_x();
        let delta_y = event.delta_y();

        // Create ScrollDelta based on the browser's delta mode
        let to_scroll_delta = |delta: f64| -> ScrollDelta {
            let delta_int = delta as i32;
            match delta_mode {
                ScrollDelta::DOM_DELTA_PIXEL => ScrollDelta::Pixels(delta_int),
                ScrollDelta::DOM_DELTA_LINE => ScrollDelta::Lines(delta_int),
                ScrollDelta::DOM_DELTA_PAGE => ScrollDelta::Pages(delta_int),
                _ => ScrollDelta::Pixels(delta_int), // fallback to pixels
            }
        };

        let scroll_x = to_scroll_delta(delta_x);
        let scroll_y = to_scroll_delta(delta_y);

        // Determine the event kind based on which delta is larger
        // Compare normalized steps to determine primary scroll direction
        let event_kind = if scroll_x.to_steps().abs() > scroll_y.to_steps().abs() {
            MouseEventKind::ScrolledHorizontal(scroll_x)
        } else {
            MouseEventKind::ScrolledVertical(scroll_y)
        };

        MouseEvent {
            button: MouseButton::Unidentified,
            event: event_kind,
            x: event.client_x() as u32,
            y: event.client_y() as u32,
            ctrl,
            alt,
            shift,
        }
    }
}
