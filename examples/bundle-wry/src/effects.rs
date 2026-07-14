use ratzilla::ratatui::{
    layout::{Constraint, Layout},
    prelude::Color,
    style::Style,
};
use tachyonfx::{
    fx::*, CellFilter, ColorSpace, Duration, Effect, EffectTimer, Interpolation::*, Motion,
};

pub fn startup() -> Effect {
    let timer = EffectTimer::from_ms(3000, QuadIn);

    parallel(&[
        parallel(&[
            sweep_in(Motion::LeftToRight, 100, 20, Color::Black, timer),
            sweep_in(Motion::UpToDown, 100, 20, Color::Black, timer),
        ]),
        prolong_start(500, coalesce((2500, SineOut))),
    ])
}

pub(super) fn pulsate_selected_tab() -> Effect {
    let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]);
    let highlighted_tab = CellFilter::AllOf(vec![
        CellFilter::Layout(layout, 0),
        CellFilter::FgColor(Color::LightYellow),
    ]);

    // never ends
    repeating(hsl_shift_fg([-170.0, 25.0, 30.0], (1000, SineInOut))).with_filter(highlighted_tab)
}

pub(super) fn change_tab() -> Effect {
    let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]);
    let dissolved = Style::default().fg(Color::White).bg(BG_COLOR);

    let flash_color = Color::from_u32(0x3232030);

    sequence(&[
        // close panel effect
        with_duration(
            Duration::from_millis(300),
            parallel(&[
                style_all_cells(),
                never_complete(fade_to(flash_color, flash_color, (30, ExpoInOut))),
                never_complete(dissolve_to(dissolved, (125, ExpoInOut))),
                never_complete(fade_to_fg(BG_COLOR, (125, BounceOut))),
            ])
            .with_color_space(ColorSpace::Rgb),
        ),
        // init pane, after having closed the (not) "old" one
        parallel(&[
            style_all_cells(),
            fade_from(BG_COLOR, BG_COLOR, (140, Linear)),
            sweep_in(Motion::UpToDown, 40, 0, BG_COLOR, (140, Linear))
                .with_color_space(ColorSpace::Hsl),
        ]),
    ])
    .with_filter(CellFilter::Layout(layout, 1))
}

/// Style all cells have so that they have non-reset foreground and background colors.
/// This ensures that color interpolation works correctly.
fn style_all_cells() -> Effect {
    never_complete(effect_fn((), 100_000, |_, _, cells| {
        for (_, cell) in cells {
            if cell.fg == Color::Reset {
                cell.set_fg(Color::White);
            }

            if cell.bg == Color::Reset {
                cell.set_bg(BG_COLOR);
            }
        }
    }))
}

const BG_COLOR: Color = Color::from_u32(0x121212);
