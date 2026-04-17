use std::{cell::RefCell, io, rc::Rc};

use ratatui::{
    layout::Alignment,
    style::{Color, Stylize},
    widgets::{Block, BorderType, Paragraph},
    Frame, Terminal,
};

use ratzilla::{
    event::{KeyCode, KeyEvent},
    DomBackend, WebRenderer,
};

fn main() -> io::Result<()> {
    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    let state = Rc::new(App::default());

    let event_state = Rc::clone(&state);
    terminal.on_key_event(move |key_event| {
        event_state.handle_events(key_event);
    });

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |frame| {
        render_state.render(frame);
    });

    Ok(())
}

#[derive(Default)]
struct App {
    counter: RefCell<u8>,
}

impl App {
    fn render(&self, frame: &mut Frame) {
        let counter = self.counter.borrow();
        let block = Block::bordered()
            .title("{{project-name}}")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        let text = format!(
            "This is a Ratzilla template.\n\
             Press left and right to increment and decrement the counter respectively.\n\
             Counter: {counter}",
        );

        let paragraph = Paragraph::new(text)
            .block(block)
            .fg(Color::White)
            .bg(Color::Black)
            .centered();

        frame.render_widget(paragraph, frame.area());
    }

    fn handle_events(&self, key_event: KeyEvent) {
        let mut counter = self.counter.borrow_mut();
        match key_event.code {
            KeyCode::Left => *counter = counter.saturating_sub(1),
            KeyCode::Right => *counter = counter.saturating_add(1),
            _ => {}
        }
    }
}
