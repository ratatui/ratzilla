use crate::backend::utils::CssAttribute;
use ratatui::style::Style;

/// Supported cursor shapes.
#[derive(Debug, Default)]
pub enum CursorShape {
    /// A non blinking block cursor shape (█).
    #[default]
    SteadyBlock,
    /// A non blinking underscore cursor shape (_).
    SteadyUnderScore,
    /// This variant is only used to clear cursor.
    None,
}

impl CursorShape {
    /// Transforms the given style to hide the cursor.
    pub fn hide(&self, style: Style) -> Style {
        match self {
            CursorShape::SteadyBlock => style.not_reversed(),
            CursorShape::SteadyUnderScore => style.not_underlined(),
            CursorShape::None => style,
        }
    }

    /// Transforms the given style to show the cursor.
    pub fn show(&self, style: Style) -> Style {
        match self {
            CursorShape::SteadyBlock => style.reversed(),
            CursorShape::SteadyUnderScore => style.underlined(),
            CursorShape::None => style,
        }
    }

    /// Returns a list of css fields and their values for this cursor shape.
    pub fn get_css_attribute(&self) -> CssAttribute {
        match self {
            CursorShape::SteadyBlock => CssAttribute {
                field: "text-decoration",
                value: Some("none"),
            },
            CursorShape::SteadyUnderScore => CssAttribute {
                field: "text-decoration",
                value: Some("underline"),
            },
            CursorShape::None => CssAttribute {
                field: "text-decoration",
                value: None,
            },
        }
    }
}
