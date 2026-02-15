use std::io;

use ratzilla::ratatui::{
    layout::Alignment,
    style::Color,
    widgets::{Block, Paragraph},
};

use ratzilla::{FontAtlasConfig, SelectionMode, WebRenderer};

use examples_shared::backend::{BackendType, MultiBackendBuilder};
use ratzilla::backend::webgl2::WebGl2BackendOptions;

fn main() -> io::Result<()> {
    let webgl2_options = WebGl2BackendOptions::new()
        .enable_mouse_selection_with_mode(SelectionMode::Block)
        .enable_console_debug_api()
        .measure_performance(true)
        .font_atlas_config(FontAtlasConfig::dynamic(&["Maple Mono NF CN"], 15.0));

    let terminal = MultiBackendBuilder::with_fallback(BackendType::Dom)
        .webgl2_options(webgl2_options)
        .build_terminal()?;

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
