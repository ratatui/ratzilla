use std::{cell::RefCell, rc::Rc};

use ratzilla::{event::KeyCode, utils::set_document_title, widgets::Hyperlink, CursorShape, WebRenderer};

use ratzilla::ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Circle},
        Block, Paragraph, Widget,
    },
};
use examples_shared::backend::{BackendType, MultiBackendBuilder};
use ratzilla::backend::canvas::CanvasBackendOptions;
use ratzilla::backend::dom::DomBackendOptions;
use ratzilla::backend::webgl2::{SelectionMode, WebGl2BackendOptions};

struct App {
    count: u64,
    pub ball: Circle,
    vx: f64,
    vy: f64,
}

impl App {
    const fn new() -> Self {
        Self {
            count: 0,
            ball: Circle {
                x: 20.0,
                y: 20.0,
                radius: 5.0,
                color: Color::Green,
            },
            vx: 1.0,
            vy: 1.0,
        }
    }

    fn pong_canvas(&self) -> impl Widget + '_ {
        Canvas::default()
            .marker(Marker::Dot)
            .block(Block::bordered().title("Pong"))
            .paint(|ctx| {
                ctx.draw(&self.ball);
            })
            .x_bounds([0.0, 50.0])
            .y_bounds([0.0, 100.0])
    }

    fn update(&mut self) {
        if self.ball.x < 10.0 || self.ball.x > 40.0 {
            self.vx = -self.vx;
        }
        if self.ball.y < 10.0 || self.ball.y > 100.0 {
            self.vy = -self.vy;
        }
        self.ball.x += self.vx;
        self.ball.y += self.vy;
    }
}

fn main() -> std::io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let app_state = Rc::new(RefCell::new(App::new()));

    let mut terminal = MultiBackendBuilder::with_fallback(BackendType::Dom)
        .webgl2_options(WebGl2BackendOptions::new()
            .grid_id("container")
            .enable_hyperlinks()
            .enable_mouse_selection_with_mode(SelectionMode::default())
        )
        .canvas_options(CanvasBackendOptions::new()
            .grid_id("container")
        )
        .dom_options(DomBackendOptions::new(Some("container".into()), CursorShape::SteadyBlock))
        .build_terminal()?;

    terminal.on_key_event({
        let app_state_cloned = app_state.clone();
        move |event| {
            let mut app_state = app_state_cloned.borrow_mut();
            match event.code {
                KeyCode::Char('t') => {
                    let _ = set_document_title("RATATUI");
                }
                KeyCode::Char(' ') => {
                    app_state.count = 0;
                    if app_state.ball.color == Color::Green {
                        app_state.ball.color = Color::White;
                    } else {
                        app_state.ball.color = Color::Green;
                    }
                }
                _ => {}
            }
        }
    })?;
    terminal.draw_web(move |f| {
        let mut app_state = app_state.borrow_mut();
        app_state.count += 1;
        app_state.update();
        let horizontal =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [left, right] = horizontal.areas(f.area());

        f.render_widget(
            Paragraph::new(format!("Count: {}", app_state.count))
                .alignment(Alignment::Center)
                .block(
                    Block::bordered()
                        .title_top("Ratzilla".bold())
                        .title_bottom("Press 't' to change title, enter to change color")
                        .border_style(Style::default().fg(Color::Yellow).bg(Color::Black)),
                ),
            left,
        );
        f.render_widget(app_state.pong_canvas(), right);

        let url = "https://orhun.dev";
        let link = Hyperlink::new(url);
        let area = Rect::new(right.x, right.y + right.height - 1, url.len() as u16, 1);
        f.render_widget(link, area);
    });

    Ok(())
}
