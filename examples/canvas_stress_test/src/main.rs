//! # Ratzilla Canvas Rendering Stress Test
//!
//! This example demonstrates a stress test for the foreground text rendering
//! capabilities of the `WebGl2Backend` in Ratzilla. It displays large amounts
//! of lorem ipsum text with different coloring strategies while monitoring
//! the frames per second (FPS).
//!
//! There are four different text rendering strategies, declared in descending
//! order of performance.

use ratzilla::{ratatui::{
    layout::Size,
    style::{Color, Styled},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
}, WebRenderer};
use examples_shared::backend::{BackendType, MultiBackendBuilder};
use ratzilla::backend::webgl2::WebGl2BackendOptions;
use std::{cell::RefCell, rc::Rc};

fn main() -> std::io::Result<()> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let mut terminal = MultiBackendBuilder::with_fallback(BackendType::WebGl2)
        .webgl2_options(WebGl2BackendOptions::new().measure_performance(true))
        .build_terminal()?;

    let mut rendered_frames = 0; // used for screen cycling

    // style index for the text
    let text_stye = Rc::new(RefCell::new(0usize));

    // any key event changes the text style
    let text_style_key_event = text_stye.clone();
    terminal.on_key_event(move |_| {
        let current = text_style_key_event.as_ref();
        let next = current.borrow().clone() + 1;
        *current.borrow_mut() = next % WidgetCache::SCREEN_TYPES;
    })?;

    // Pre-generate widgets for better performance; in particular,
    // this avoids excessive GC pressure in the JS heap.
    let widget_cache = WidgetCache::new(terminal.size().unwrap());

    terminal.draw_web(move |frame| {
        // retrieve and render cached paragraph widget
        let p = widget_cache.get(*text_stye.as_ref().borrow(), rendered_frames);
        frame.render_widget(p, frame.area());
        rendered_frames += 1;
    });

    Ok(())
}

/// Caches pre-generated widgets for different text rendering strategies.
///
/// A number of paragraphs are pregenerated per style type to avoid
/// measuring the performance of the widget generation overhead.
struct WidgetCache {
    /// Widgets that render text in white
    white: Vec<Paragraph<'static>>,

    /// Widgets that color words starting with 'e'
    colorize_e_words: Vec<Paragraph<'static>>,

    /// Widgets that color words based on first character
    colorize_some: Vec<Paragraph<'static>>,

    /// Widgets that color words based on their hash
    colorize_words: Vec<Paragraph<'static>>,
}

impl WidgetCache {
    /// Number of different text rendering strategies
    const SCREEN_TYPES: usize = 4;

    /// Number of pre-generated cycled screens per strategy
    const CACHED_SCREENS: usize = 10;

    fn new(area: Size) -> Self {
        fn white(_: &'static str, span: Span<'static>) -> Span<'static> {
            let style = span.style.fg(Color::White);
            span.set_style(style)
        }

        fn colorize_words(word: &'static str, span: Span<'static>) -> Span<'static> {
            let hash: usize = word.chars().map(|c| c as usize).sum();
            let color = COLORS[hash % COLORS.len()];
            let style = span.style.fg(color);
            span.set_style(style)
        }

        fn colorize_some(word: &'static str, span: Span<'static>) -> Span<'static> {
            let hash: usize = word.chars().take(1).map(|c| c as usize / 10).sum();
            let color = COLORS[hash % COLORS.len()];
            let style = span.style.fg(color);
            span.set_style(style)
        }

        fn colorize_e_words(word: &'static str, span: Span<'static>) -> Span<'static> {
            let c = if word.starts_with("e") {
                COLORS[7]
            } else {
                COLORS[0]
            };
            let style = span.style.fg(c);
            span.set_style(style)
        }

        fn prepare_walls_of_text(
            cells: u32,
            f: fn(&'static str, Span<'static>) -> Span<'static>
        ) -> Vec<Paragraph<'static>> {
            (0..WidgetCache::CACHED_SCREENS)
                .into_iter()
                .map(|i| lorem_ipsum_paragraph(cells, i * WidgetCache::CACHED_SCREENS, f))
                .collect::<Vec<_>>()
        }

        let cell_count = (area.width * area.height) as u32;
        Self {
            white: prepare_walls_of_text(cell_count, white),
            colorize_e_words: prepare_walls_of_text(cell_count, colorize_e_words),
            colorize_some: prepare_walls_of_text(cell_count, colorize_some),
            colorize_words: prepare_walls_of_text(cell_count, colorize_words),
        }
    }

    /// Retrieves a pre-generated paragraph widget based on the style type and index.
    fn get(&self, style_type: usize, index: usize) -> &Paragraph<'static> {
        debug_assert!(style_type < Self::SCREEN_TYPES);

        let index = index % Self::CACHED_SCREENS;
        match style_type {
            0 => &self.white[index],
            1 => &self.colorize_e_words[index],
            2 => &self.colorize_some[index],
            _ => &self.colorize_words[index],
        }
    }
}

/// Generates a paragraph of lorem ipsum text with a specified length and word offset.
fn lorem_ipsum_paragraph(
    text_len: u32,
    word_offset: usize,
    span_op: impl Fn(&'static str, Span<'static>) -> Span<'static>,
) -> Paragraph<'static> {
    let spans = lorem_ipsum(text_len as _, word_offset).map(|w| span_op(w, Span::raw(w)));

    Paragraph::new(Line::from_iter(spans)).wrap(Wrap { trim: true })
}

/// Generates an iterator of lorem ipsum words, with a specified length and word offset.
fn lorem_ipsum(len: usize, word_offset: usize) -> impl Iterator<Item = &'static str> {
    let mut acc = 0;

    LOREM_IPSUM
        .iter()
        .copied()
        .cycle()
        .skip(word_offset * 2) // *2 to account for the space
        .flat_map(|w| [w, " "].into_iter())
        .take_while(move |w| {
            let is_within_screen = acc <= len;
            acc += w.len();
            is_within_screen
        })
}

const COLORS: [Color; 22] = [
    Color::from_u32(0xfbf1c7),
    Color::from_u32(0xfb4934),
    Color::from_u32(0xb8bb26),
    Color::from_u32(0xfabd2f),
    Color::from_u32(0x83a598),
    Color::from_u32(0xd3869b),
    Color::from_u32(0x8ec07c),
    Color::from_u32(0xfe8019),
    Color::from_u32(0xcc241d),
    Color::from_u32(0x98971a),
    Color::from_u32(0xd79921),
    Color::from_u32(0x458588),
    Color::from_u32(0xb16286),
    Color::from_u32(0x689d6a),
    Color::from_u32(0xd65d0e),
    Color::from_u32(0x9d0006),
    Color::from_u32(0x79740e),
    Color::from_u32(0xb57614),
    Color::from_u32(0x076678),
    Color::from_u32(0x8f3f71),
    Color::from_u32(0x427b58),
    Color::from_u32(0xaf3a03),
];

const LOREM_IPSUM: [&str; 69] = [
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "ut",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "ut",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "dolor",
    "in",
    "reprehenderit",
    "in",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "dolore",
    "eu",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "in",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
];
