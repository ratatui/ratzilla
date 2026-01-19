use crate::{
    backend::color::ansi_to_rgb,
    error::Error,
    utils::{get_screen_size, get_window_size, is_mobile},
};
use compact_str::{format_compact, CompactString};
use ratatui::{
    buffer::Cell,
    layout::Size,
    style::{Color, Modifier},
};
use unicode_width::UnicodeWidthStr;
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    window, Document, Element, HtmlCanvasElement, Window,
};

pub struct CssAttribute {
    pub field: &'static str,
    pub value: Option<&'static str>,
}

/// Creates a new `<span>` element with the given cell.
pub(crate) fn create_span(document: &Document, cell: &Cell) -> Result<Element, Error> {
    let span = document.create_element("span")?;
    span.set_inner_html(cell.symbol());

    let style = get_cell_style_as_css(cell);
    span.set_attribute("style", &style)?;
    Ok(span)
}

/// Creates a new `<a>` element with the given cells.
pub(crate) fn create_anchor(document: &Document, cells: &[Cell]) -> Result<Element, Error> {
    let anchor = document.create_element("a")?;
    anchor.set_attribute(
        "href",
        &cells.iter().map(|c| c.symbol()).collect::<String>(),
    )?;
    anchor.set_attribute("style", &get_cell_style_as_css(&cells[0]))?;
    Ok(anchor)
}

/// Converts a cell to a CSS style.
pub(crate) fn get_cell_style_as_css(cell: &Cell) -> String {
    let mut fg = ansi_to_rgb(cell.fg);
    let mut bg = ansi_to_rgb(cell.bg);

    if cell.modifier.contains(Modifier::REVERSED) {
        std::mem::swap(&mut fg, &mut bg);
    }

    let fg_style = match fg {
        Some(color) => format!("color: rgb({}, {}, {});", color.0, color.1, color.2),
        None => "color: rgb(255, 255, 255);".to_string(),
    };

    let bg_style = match bg {
        Some(color) => format!(
            "background-color: rgb({}, {}, {});",
            color.0, color.1, color.2
        ),
        None => {
            // If the cell needs to be reversed but we don't have a valid background,
            // then default the background to white.
            if cell.modifier.contains(Modifier::REVERSED) {
                "background-color: rgb(255, 255, 255);".to_string()
            } else {
                "background-color: transparent;".to_string()
            }
        }
    };

    let mut modifier_style = String::new();
    if cell.modifier.contains(Modifier::BOLD) {
        modifier_style.push_str("font-weight: bold; ");
    }
    if cell.modifier.contains(Modifier::DIM) {
        modifier_style.push_str("opacity: 0.5; ");
    }
    if cell.modifier.contains(Modifier::ITALIC) {
        modifier_style.push_str("font-style: italic; ");
    }
    if cell.modifier.contains(Modifier::UNDERLINED) {
        modifier_style.push_str("text-decoration: underline; ");
    }
    if cell.modifier.contains(Modifier::HIDDEN) {
        modifier_style.push_str("visibility: hidden; ");
    }
    if cell.modifier.contains(Modifier::CROSSED_OUT) {
        modifier_style.push_str("text-decoration: line-through; ");
    }

    // ensure consistent width for braille characters
    let braille_style = if contains_braille(cell) {
        "font-variant-numeric: tabular-nums; "
    } else {
        ""
    };

    let sizing = format!("display: inline-block; width: {}ch;", cell.symbol().width());

    format!("{fg_style} {bg_style} {modifier_style} {braille_style} {sizing}")
}

/// Parse an inline CSS style string into a Vec of (property, value) pairs.
fn parse_inline_style(css: &str) -> Vec<(String, String)> {
    css.split(';')
        .filter_map(|decl| {
            let decl = decl.trim();
            if decl.is_empty() {
                return None;
            }
            let mut parts = decl.splitn(2, ':');
            let key = parts.next()?.trim();
            let val = parts.next()?.trim();
            if key.is_empty() || val.is_empty() {
                None
            } else {
                Some((key.to_string(), val.to_string()))
            }
        })
        .collect()
}

/// Build a css string from an array of (field, value) css style attributes.
fn build_inline_style(styles: &[(String, String)]) -> String {
    let mut s = String::new();
    for (k, v) in styles {
        s.push_str(format!("{k}: {v};").as_str());
    }
    s
}

/// Replace the `style` attribute by the given css string in the given Element.
///
/// If the css string is empty, removes completly the `style` attribute.
fn set_or_remove_style_attribute(elem: &Element, css: String) -> Result<(), JsValue> {
    if css.is_empty() {
        elem.remove_attribute("style")
    } else {
        elem.set_attribute("style", &css)
    }
}

/// Update or remove a CSS field in the inline `style` attribute.
///
/// - If `attribute.value` is `Some(v)`: sets/updates `attribute.field: v`.
/// - If `attribute.value` is `None`: removes `attribute.field`.
/// - If the final style is empty: removes the `style` attribute entirely.
pub(crate) fn update_css_field(attribute: CssAttribute, elem: &Element) -> Result<(), JsValue> {
    let field = attribute.field;
    let value = attribute.value;

    let css = elem.get_attribute("style").unwrap_or_default();
    let mut styles = parse_inline_style(&css);
    let target = field.trim().to_string();

    // Either update/add or remove the field
    match value {
        Some(new_val) => {
            let new_val = new_val.trim().to_string();
            let mut found = false;
            for (k, v) in styles.iter_mut() {
                if k.eq_ignore_ascii_case(&target) {
                    *v = new_val.clone();
                    found = true;
                    break;
                }
            }
            if !found {
                styles.push((target, new_val));
            }
        }
        None => {
            styles.retain(|(k, _)| !k.eq_ignore_ascii_case(&target));
        }
    }

    // Rebuild CSS string
    let updated_css = build_inline_style(&styles);
    set_or_remove_style_attribute(elem, updated_css)
}

/// Converts a Color to a CSS style.
pub(crate) fn get_canvas_color(color: Color, fallback_color: Color) -> CompactString {
    let color = ansi_to_rgb(color).unwrap_or_else(|| ansi_to_rgb(fallback_color).unwrap());

    format_compact!("rgb({}, {}, {})", color.0, color.1, color.2)
}

/// Calculates the number of pixels that can fit in the window.
pub(crate) fn get_raw_window_size() -> (u16, u16) {
    fn js_val_to_int<I: TryFrom<usize>>(val: JsValue) -> Option<I> {
        val.as_f64().and_then(|i| I::try_from(i as usize).ok())
    }

    web_sys::window()
        .and_then(|s| {
            s.inner_width()
                .ok()
                .and_then(js_val_to_int::<u16>)
                .zip(s.inner_height().ok().and_then(js_val_to_int::<u16>))
        })
        .unwrap_or((120, 120))
}

/// Returns the number of pixels that can fit in the window.
pub(crate) fn get_raw_screen_size() -> (i32, i32) {
    let s = web_sys::window().unwrap().screen().unwrap();
    (s.width().unwrap(), s.height().unwrap())
}

/// Returns a buffer based on the screen size.
pub(crate) fn get_sized_buffer() -> Vec<Vec<Cell>> {
    let size = get_size();
    vec![vec![Cell::default(); size.width as usize]; size.height as usize]
}

/// Returns a buffer size based on the screen size.
pub(crate) fn get_size() -> Size {
    if is_mobile() {
        get_screen_size()
    } else {
        get_window_size()
    }
}

/// Returns a buffer based on the canvas size.
pub(crate) fn get_sized_buffer_from_canvas(canvas: &HtmlCanvasElement) -> Vec<Vec<Cell>> {
    let width = canvas.client_width() as u16 / 10_u16;
    let height = canvas.client_height() as u16 / 19_u16;
    vec![vec![Cell::default(); width as usize]; height as usize]
}

/// Returns the document object from the window.
pub(crate) fn get_document() -> Result<Document, Error> {
    get_window()?
        .document()
        .ok_or(Error::UnableToRetrieveDocument)
}

/// Returns the window object.
pub(crate) fn get_window() -> Result<Window, Error> {
    window().ok_or(Error::UnableToRetrieveWindow)
}

/// Returns an element by its ID or the body element if no ID is provided.
pub(crate) fn get_element_by_id_or_body(id: Option<&String>) -> Result<web_sys::Element, Error> {
    match id {
        Some(id) => get_document()?
            .get_element_by_id(id)
            .ok_or_else(|| Error::UnableToRetrieveElementById(id.to_string())),
        None => get_document()?
            .body()
            .ok_or(Error::UnableToRetrieveBody)
            .map(|body| body.into()),
    }
}

/// Returns the performance object from the window.
pub(crate) fn performance() -> Result<web_sys::Performance, Error> {
    Ok(get_window()?
        .performance()
        .ok_or(Error::UnableToRetrieveComponent("Performance"))?)
}

/// Creates a new canvas element in the specified parent element with the
/// given width and height.
pub(crate) fn create_canvas_in_element(
    parent: &Element,
    width: u32,
    height: u32,
) -> Result<HtmlCanvasElement, Error> {
    let element = get_document()?.create_element("canvas")?;

    let canvas = element
        .clone()
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| ())
        .expect("Unable to cast canvas element");
    canvas.set_width(width);
    canvas.set_height(height);

    parent.append_child(&element)?;

    Ok(canvas)
}

/// Checks if the given cell contains a braille character.
fn contains_braille(cell: &Cell) -> bool {
    cell.symbol()
        .chars()
        .next()
        .is_some_and(|c| ('\u{2800}'..='\u{28FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use web_sys::window;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    fn create_elem_with_style(s: &str) -> Element {
        let doc = window().unwrap().document().unwrap();
        let el = doc.create_element("div").unwrap();
        if !s.is_empty() {
            el.set_attribute("style", s).unwrap();
        }
        el
    }

    #[wasm_bindgen_test]
    fn test_add_new_field() {
        let el = create_elem_with_style("color: red;");
        let attr = CssAttribute {
            field: "background-color",
            value: Some("blue"),
        };
        update_css_field(attr, &el).unwrap();
        let got = el.get_attribute("style").unwrap();
        assert!(got.contains("color: red;"));
        assert!(got.contains("background-color: blue;"));
    }

    #[wasm_bindgen_test]
    fn test_update_existing_field() {
        let el = create_elem_with_style("color: red;");
        let attr = CssAttribute {
            field: "color",
            value: Some("green"),
        };
        update_css_field(attr, &el).unwrap();
        assert_eq!(el.get_attribute("style").unwrap(), "color: green;");
    }

    #[wasm_bindgen_test]
    fn test_remove_field() {
        let el = create_elem_with_style("color: red; background-color: blue;");
        let attr = CssAttribute {
            field: "color",
            value: None,
        };
        update_css_field(attr, &el).unwrap();
        let got = el.get_attribute("style").unwrap();
        assert_eq!(got, "background-color: blue;");
    }

    #[wasm_bindgen_test]
    fn test_remove_last_field_removes_attribute() {
        let el = create_elem_with_style("color: red;");
        let attr = CssAttribute {
            field: "color",
            value: None,
        };
        update_css_field(attr, &el).unwrap();
        assert!(el.get_attribute("style").is_none());
    }
}
