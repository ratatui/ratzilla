//! ## Backends
//!
//! **Ratzilla** provides three backends for rendering terminal UIs in the browser,
//! each with different performance characteristics and trade-offs:
//!
//! - [`WebGl2Backend`]: GPU-accelerated rendering powered by [beamterm][beamterm]. Uses prebuilt
//!   or runtime generated font atlases. Very low overhead, typically under 1ms CPU time per frame,
//!   even for fullscreen terminals with all cells changing.
//!
//! - [`CanvasBackend`]: Canvas 2D API with Unicode support via browser font rendering.
//!   Good fallback for the `WebGl2Backend`, if WebGL2 isn't available. Does not support hyperlinks
//!   or text selection, but can render dynamic Unicode and single-cell emoji.
//!
//! - [`DomBackend`]: Renders cells as HTML elements. Most compatible, but slowest for large
//!   terminals.
//!
//! [beamterm]: https://github.com/junkdog/beamterm
//!
//! ## Backend Comparison
//!
//! | Feature                      | DomBackend | CanvasBackend | WebGl2Backend  |
//! |------------------------------|------------|---------------|----------------|
//! | **60fps on large terminals** | ✗          | ✗             | ✓              |
//! | **Memory Usage**             | Highest    | Medium        | Lowest         |
//! | **Hyperlinks**               | ✗          | ✗             | ✓              |
//! | **Text Selection**           | Linear     | ✗             | Linear/Block   |
//! | **Unicode/Emoji Support**    | Full       | Limited²      | Full¹          |
//! | **Dynamic Characters**       | ✓          | ✓             | ✓¹             |
//! | **Font Variants**            | ✓          | Regular only  | ✓              |
//! | **Underline**                | ✓          | ✗             | ✓              |
//! | **Strikethrough**            | ✓          | ✗             | ✓              |
//! | **Browser Support**          | All        | All           | Modern (2017+) |
//!
//! ¹: The [dynamic font atlas](webgl2::FontAtlasConfig::Dynamic) rasterizes
//!    glyphs on demand with full Unicode/emoji and font variant support. The
//!    [static font atlas](webgl2::FontAtlasConfig::Static) is limited to glyphs
//!    compiled into the `.atlas` file.
//! ²: Unicode is supported, but emoji only render correctly when it spans one cell.
//!    Most emoji occupy two cells.
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
