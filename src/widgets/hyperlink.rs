use std::{borrow::Cow, rc::Rc};

use ratatui::{buffer::Buffer, layout::Rect, text::Span, widgets::Widget};

use crate::widgets::hyperlink_state::{register, HyperlinkRegion};

/// A widget that can be used to render hyperlinks.
///
/// ```rust no_run
/// use ratzilla::widgets::Hyperlink;
///
/// let link = Hyperlink::new("https://ratatui.rs");
/// let docs = Hyperlink::with_label("Ratatui", "https://ratatui.rs");
///
/// // Then you can render it as usual:
/// // frame.render_widget(link, frame.area());
/// ```
pub struct Hyperlink<'a> {
    label: Span<'a>,
    url: Rc<str>,
}

impl<'a> Hyperlink<'a> {
    /// Constructs a new [`Hyperlink`] widget.
    pub fn new<T>(url: T) -> Self
    where
        T: Into<Span<'a>>,
    {
        let label = url.into();
        Self {
            url: Rc::from(label.content.clone().into_owned()),
            label,
        }
    }

    /// Constructs a new [`Hyperlink`] widget with a separate label and target URL.
    pub fn with_label<T, U>(label: T, url: U) -> Self
    where
        T: Into<Span<'a>>,
        U: Into<Cow<'a, str>>,
    {
        Self {
            label: label.into(),
            url: Rc::from(url.into().into_owned()),
        }
    }
}

impl Widget for Hyperlink<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let width = self.label.width().min(area.width as usize) as u16;
        self.label.render(area, buf);
        if width > 0 && area.height > 0 {
            register(HyperlinkRegion {
                x: area.x,
                y: area.y,
                width,
                url: self.url,
            });
        }
    }
}
