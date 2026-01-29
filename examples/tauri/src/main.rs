use std::io;

use ratzilla::{
    ratatui::{
        prelude::*,
        widgets::{Block, Clear},
    },
    WebRenderer,
};
use examples_shared::backend::{BackendType, MultiBackendBuilder};
use tachyonfx::{
    fx, CenteredShrink, Duration, Effect, EffectRenderer, EffectTimer, Interpolation, Motion,
};

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let terminal = MultiBackendBuilder::with_fallback(BackendType::Canvas)
        .build_terminal()?;
    let mut effect = fx::sequence(&[
        // first we "sweep in" the text from the left, before reversing the effect
        fx::ping_pong(fx::sweep_in(
            Motion::LeftToRight,
            10,
            0,
            Color::DarkGray,
            EffectTimer::from_ms(2000, Interpolation::QuadIn),
        )),
        // then we coalesce the text back to its original state
        // (note that EffectTimers can be constructed from a tuple of duration and interpolation)
        fx::coalesce((800, Interpolation::SineOut)),
    ]);

    terminal.draw_web(move |f| ui(f, &mut effect));

    Ok(())
}

fn ui(f: &mut Frame<'_>, effect: &mut Effect) {
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(Color::Black))
        .render(f.area(), f.buffer_mut());
    let area = f.area().inner_centered(25, 2);
    let main_text = Text::from(vec![
        Line::from("Hello, Ratzilla!"),
        Line::from("Are you rendering, son?"),
    ]);
    f.render_widget(main_text.light_magenta().centered(), area);
    if effect.running() {
        f.render_effect(effect, area, Duration::from_millis(100));
    }
}
