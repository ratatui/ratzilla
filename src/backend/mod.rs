//! ## Backends
//!
//! **Ratzilla** provides three backends for rendering terminal UIs in the browser,
//! each with different performance characteristics and trade-offs:
//!
//! - [`WebGl2Backend`]: GPU-accelerated rendering powered by [beamterm][beamterm]. Uses prebuilt
//!   or runtime generated font atlases. Best performance, capable of 60fps on large terminals.
//!
//! - [`CanvasBackend`]: Canvas 2D API with full Unicode support via browser font rendering.
//!   Good fallback when WebGL2. Does not support hyperlinks or text selection, but can render
//!   dynamic Unicode/emoji.
//!
//! - [`DomBackend`]: Renders cells as HTML elements. Most compatible and accessible,
//!   supports hyperlinks, but slowest for large terminals.
//!
//! [beamterm]: https://github.com/junkdog/beamterm
//!
//! ## Backend Comparison
//!
//! | Feature                      | DomBackend | CanvasBackend | WebGl2Backend  |
//! |------------------------------|------------|---------------|----------------|
//! | **60fps on large terminals** | ✗          | ✗             | ✓              |
//! | **Memory Usage**             | Highest    | Medium        | Lowest         |
//! | **Hyperlinks**               | ✓          | ✗             | ✓              |
//! | **Text Selection**           | ✓          | ✗             | ✓              |
//! | **Accessibility**            | ✓          | Limited       | Limited        |
//! | **Unicode/Emoji Support**    | Full       | Full          | Full¹          |
//! | **Dynamic Characters**       | ✓          | ✓             | ✓¹             |
//! | **Font Variants**            | ✓          | Regular only  | ✓              |
//! | **Underline**                | ✓          | ✗             | ✓              |
//! | **Strikethrough**            | ✓          | ✗             | ✓              |
//! | **Browser Support**          | All        | All           | Modern (2017+) |
//!
//! ¹: The [dynamic font atlas](webgl2::WebGl2BackendOptions::dynamic_font_atlas) rasterizes
//!    glyphs on demand with full Unicode/emoji and font variant support. The
//!    [static font atlas](webgl2::WebGl2BackendOptions::font_atlas) is limited to glyphs
//!    included at atlas build time.
//!
//! ## Choosing a Backend
//!
//! - **WebGl2Backend**: Preferred for most applications - consumes the least amount of resources
//! - **CanvasBackend**: When you must support non-WebGL2 browsers
//! - **DomBackend**: When you need better accessibility or CSS styling

/// Canvas backend.
pub mod canvas;

/// DOM backend.
pub mod dom;

/// WebGL2 backend.
pub mod webgl2;

/// Color handling.
mod color;
/// Backend utilities.
pub(crate) mod utils;

/// Cursor shapes.
pub mod cursor;
