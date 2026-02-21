use std::{cell::RefCell, io, rc::Rc};

use ratatui::{
    layout::Alignment,
    style::{Color, Stylize},
    widgets::{Block, BorderType, Paragraph},
    Frame, Terminal,
};

use ratzilla::{event::{KeyCode, KeyEvent}, SelectionMode, WebRenderer};
use examples_shared::backend::{BackendType, MultiBackendBuilder};
use ratzilla::backend::webgl2::WebGl2BackendOptions;

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let webgl2_options = WebGl2BackendOptions::new()
        .enable_mouse_selection_with_mode(SelectionMode::Linear)
        .measure_performance(true);

    let terminal = MultiBackendBuilder::with_fallback(BackendType::Dom)
        .webgl2_options(webgl2_options)
        .build_terminal()?;

    let state = Rc::new(App::default());
    let event_state = Rc::clone(&state);
    terminal.on_key_event(move |key_event| {
        let event_state = event_state.clone();
        wasm_bindgen_futures::spawn_local(
            async move { event_state.handle_events(key_event).await },
        );
    });

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |frame| {
        render_state.render(frame);
    });

    Ok(())
}

struct App {
    text: RefCell<String>,
}

impl Default for App {
    fn default() -> Self {
        let text = format!(
            "Press Ctrl+C to copy.\n\
            Press Ctrl+V to paste."
        );
        Self {
            text: RefCell::new(text),
        }
    }
}

impl App {
    fn render(&self, frame: &mut Frame) {
        let block = Block::bordered()
            .title("Clipboard Example")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        if let Ok(text) = self.text.try_borrow() {
            let paragraph = Paragraph::new(text.to_string())
                .block(block)
                .fg(Color::White)
                .bg(Color::Black)
                .centered();

            frame.render_widget(paragraph, frame.area());
        }
    }

    async fn handle_events(&self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('c') if key_event.ctrl => {
                self.set_clipboard("i like rats").await;
            }
            KeyCode::Char('v') if key_event.ctrl => {
                if let Ok(mut text) = self.text.try_borrow_mut() {
                    let clipboard_text = self.get_clipboard().await;
                    *text = clipboard_text;
                }
            }
            _ => {}
        }
    }

    async fn set_clipboard(&self, text: &str) {
        let window = web_sys::window().unwrap();
        let nav = window.navigator().clipboard();
        let promise = nav.write_text(text);
        wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    }

    async fn get_clipboard(&self) -> String {
        let window = web_sys::window().unwrap();
        let nav = window.navigator().clipboard();
        let promise = nav.read_text();
        let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
        result.as_string().unwrap_or_default()
    }
}
