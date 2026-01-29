//! # [Ratatui] Demo2 example
//!
//! The latest version of this example is available in the [examples] folder in the repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui/ratatui
//! [examples]: https://github.com/ratatui/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui/ratatui/blob/main/examples/README.md

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

mod app;
mod colors;
mod destroy;
mod tabs;
mod theme;

use std::{cell::RefCell, rc::Rc};

use app::App;
use ratzilla::{
    backend::webgl2::{SelectionMode, WebGl2BackendOptions},
    ratatui::{layout::Rect, TerminalOptions, Viewport},
    WebRenderer,
};
use examples_shared::backend::{BackendType, MultiBackendBuilder};

pub use self::{
    colors::{color_from_oklab, RgbSwatch},
    theme::THEME,
};

fn main() -> std::io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    
    // this size is to match the size of the terminal when running the demo
    // using vhs in a 1280x640 sized window (github social preview size)
    let viewport = Viewport::Fixed(Rect::new(0, 0, 81, 18));
    
    let mut terminal = MultiBackendBuilder::with_fallback(BackendType::Canvas)
        .webgl2_options(WebGl2BackendOptions::new()
            .measure_performance(true)
            .enable_mouse_selection_with_mode(SelectionMode::default())
            .enable_console_debug_api()
        )
        .terminal_options(TerminalOptions { viewport })
        .build_terminal()?;
    
    let app = Rc::new(RefCell::new(App::default()));
    terminal.on_key_event({
        let app = app.clone();
        move |key_event| {
            app.borrow_mut().handle_key_press(key_event);
        }
    })?;
    terminal.draw_web(move |f| {
        let app = app.borrow_mut();
        app.draw(f);
    });
    Ok(())
}
