use std::io;

use ratzilla::ratatui::{
    layout::Alignment,
    style::Color,
    widgets::{Block, Paragraph},
};

use ratzilla::WebRenderer;

use examples_shared::backend::{BackendType, MultiBackendBuilder};

fn main() -> io::Result<()> {
    let terminal = MultiBackendBuilder::with_fallback(BackendType::Dom).build_terminal()?;

    terminal.draw_web(move |f| {
        f.render_widget(
            Paragraph::new(
                [
                    "Hello, world!",
                    "你好，世界！",
                    "世界、こんにちは。",
                    "헬로우 월드！",
                    "👨💻👋🌐",
                ]
                .join("\n"),
            )
            .alignment(Alignment::Center)
            .block(
                Block::bordered()
                    .title("Ratzilla")
                    .title_alignment(Alignment::Center)
                    .border_style(Color::Yellow),
            ),
            f.area(),
        );
    });

    Ok(())
}
