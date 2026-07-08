use std::io;

use ratzilla::{
    ratatui::{
        layout::{Alignment, Constraint, Layout, Rect},
        prelude::{Color, Stylize, Terminal},
        widgets::{Block, BorderType, Clear, Paragraph},
    },
    widgets::Hyperlink,
    DomBackend, WebRenderer,
};

fn main() -> io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let terminal = Terminal::new(DomBackend::new()?)?;
    terminal.draw_web(|frame| {
        frame.render_widget(Clear, frame.area());

        let [card] = Layout::vertical([Constraint::Length(9)])
            .flex(ratzilla::ratatui::layout::Flex::Center)
            .areas(frame.area());
        let [card] = Layout::horizontal([Constraint::Length(44)])
            .flex(ratzilla::ratatui::layout::Flex::Center)
            .areas(card);

        frame.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(" Aliasable Hyperlinks ".bold())
                .border_style(Color::LightGreen),
            card,
        );

        let inner = card.inner(ratzilla::ratatui::layout::Margin {
            vertical: 1,
            horizontal: 2,
        });
        let [intro, docs, repo, plain] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .spacing(0)
        .areas(inner);

        frame.render_widget(
            Paragraph::new("DOM links use native browser anchors.")
                .alignment(Alignment::Left),
            intro,
        );
        render_link(
            frame,
            docs,
            "Docs: ",
            Hyperlink::with_label(
                "Ratatui rendering guide".black().on_cyan().bold(),
                "https://ratatui.rs/concepts/rendering/under-the-hood/",
            ),
        );
        render_link(
            frame,
            repo,
            "Repo: ",
            Hyperlink::with_label(
                "ratzilla on GitHub".black().on_yellow().italic(),
                "https://github.com/ratatui/ratzilla",
            ),
        );
        render_link(
            frame,
            plain,
            "Website: ",
            Hyperlink::new("https://ratatui.rs".light_cyan().underlined()),
        );
    });
    Ok(())
}

fn render_link(
    frame: &mut ratzilla::ratatui::Frame<'_>,
    area: Rect,
    label: &str,
    link: Hyperlink<'_>,
) {
    let [prefix, suffix] =
        Layout::horizontal([Constraint::Length(label.len() as u16), Constraint::Min(0)])
            .areas(area);
    frame.render_widget(Paragraph::new(label), prefix);
    frame.render_widget(link, suffix);
}
