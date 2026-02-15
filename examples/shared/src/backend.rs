use crate::{fps, utils::inject_backend_footer};
use ratzilla::{
    backend::{canvas::CanvasBackendOptions, dom::DomBackendOptions, webgl2::WebGl2BackendOptions},
    error::Error,
    event::{KeyEvent, MouseEvent},
    ratatui::{backend::Backend, prelude::backend::ClearType, Terminal, TerminalOptions},
    CanvasBackend, DomBackend, WebEventHandler, WebGl2Backend,
};
use std::{convert::TryFrom, fmt, io};
use web_sys::{window, Url};

/// Available backend types
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum BackendType {
    #[default]
    Dom,
    Canvas,
    WebGl2,
}

impl BackendType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendType::Dom => "dom",
            BackendType::Canvas => "canvas",
            BackendType::WebGl2 => "webgl2",
        }
    }
}

impl TryFrom<String> for BackendType {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "dom" => Ok(BackendType::Dom),
            "canvas" => Ok(BackendType::Canvas),
            "webgl2" => Ok(BackendType::WebGl2),
            _ => Err(format!(
                "Invalid backend type: '{s}'. Valid options are: dom, canvas, webgl2"
            )),
        }
    }
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Enum wrapper for different Ratzilla backends that implements the Ratatui Backend trait.
///
/// This enum allows switching between different rendering backends at runtime while
/// providing a unified interface. All backend operations are delegated to the wrapped
/// backend implementation.
///
/// # Backends
///
/// - `Dom`: HTML DOM-based rendering with accessibility features
/// - `Canvas`: Canvas 2D API rendering with full Unicode support  
/// - `WebGl2`: GPU-accelerated rendering using WebGL2 and beamterm-renderer
pub enum RatzillaBackend {
    Dom(DomBackend),
    Canvas(CanvasBackend),
    WebGl2(WebGl2Backend),
}

impl RatzillaBackend {
    /// Get the backend type for this backend instance.
    pub fn backend_type(&self) -> BackendType {
        match self {
            RatzillaBackend::Dom(_) => BackendType::Dom,
            RatzillaBackend::Canvas(_) => BackendType::Canvas,
            RatzillaBackend::WebGl2(_) => BackendType::WebGl2,
        }
    }
}

impl Backend for RatzillaBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a ratzilla::ratatui::buffer::Cell)>,
    {
        match self {
            RatzillaBackend::Dom(backend) => backend.draw(content),
            RatzillaBackend::Canvas(backend) => backend.draw(content),
            RatzillaBackend::WebGl2(backend) => backend.draw(content),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            RatzillaBackend::Dom(backend) => backend.flush(),
            RatzillaBackend::Canvas(backend) => backend.flush(),
            RatzillaBackend::WebGl2(backend) => backend.flush(),
        }
    }

    fn size(&self) -> io::Result<ratzilla::ratatui::layout::Size> {
        match self {
            RatzillaBackend::Dom(backend) => backend.size(),
            RatzillaBackend::Canvas(backend) => backend.size(),
            RatzillaBackend::WebGl2(backend) => backend.size(),
        }
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        match self {
            RatzillaBackend::Dom(backend) => backend.hide_cursor(),
            RatzillaBackend::Canvas(backend) => backend.hide_cursor(),
            RatzillaBackend::WebGl2(backend) => backend.hide_cursor(),
        }
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        match self {
            RatzillaBackend::Dom(backend) => backend.show_cursor(),
            RatzillaBackend::Canvas(backend) => backend.show_cursor(),
            RatzillaBackend::WebGl2(backend) => backend.show_cursor(),
        }
    }

    fn get_cursor_position(&mut self) -> io::Result<ratzilla::ratatui::layout::Position> {
        match self {
            RatzillaBackend::Dom(backend) => backend.get_cursor_position(),
            RatzillaBackend::Canvas(backend) => backend.get_cursor_position(),
            RatzillaBackend::WebGl2(backend) => backend.get_cursor_position(),
        }
    }

    fn set_cursor_position<P: Into<ratzilla::ratatui::layout::Position>>(
        &mut self,
        position: P,
    ) -> io::Result<()> {
        match self {
            RatzillaBackend::Dom(backend) => backend.set_cursor_position(position),
            RatzillaBackend::Canvas(backend) => backend.set_cursor_position(position),
            RatzillaBackend::WebGl2(backend) => backend.set_cursor_position(position),
        }
    }

    fn clear(&mut self) -> io::Result<()> {
        match self {
            RatzillaBackend::Dom(backend) => backend.clear(),
            RatzillaBackend::Canvas(backend) => backend.clear(),
            RatzillaBackend::WebGl2(backend) => backend.clear(),
        }
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        match self {
            RatzillaBackend::Dom(backend) => backend.append_lines(n),
            RatzillaBackend::Canvas(backend) => backend.append_lines(n),
            RatzillaBackend::WebGl2(backend) => backend.append_lines(n),
        }
    }

    fn window_size(&mut self) -> io::Result<ratzilla::ratatui::backend::WindowSize> {
        match self {
            RatzillaBackend::Dom(backend) => backend.window_size(),
            RatzillaBackend::Canvas(backend) => backend.window_size(),
            RatzillaBackend::WebGl2(backend) => backend.window_size(),
        }
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(io::Error::other("unimplemented")),
        }
    }
}

impl WebEventHandler for RatzillaBackend {
    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        match self {
            RatzillaBackend::Dom(backend) => backend.on_mouse_event(callback),
            RatzillaBackend::Canvas(backend) => backend.on_mouse_event(callback),
            RatzillaBackend::WebGl2(backend) => backend.on_mouse_event(callback),
        }
    }

    fn clear_mouse_events(&mut self) {
        match self {
            RatzillaBackend::Dom(backend) => backend.clear_mouse_events(),
            RatzillaBackend::Canvas(backend) => backend.clear_mouse_events(),
            RatzillaBackend::WebGl2(backend) => backend.clear_mouse_events(),
        }
    }

    fn on_key_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        match self {
            RatzillaBackend::Dom(backend) => backend.on_key_event(callback),
            RatzillaBackend::Canvas(backend) => backend.on_key_event(callback),
            RatzillaBackend::WebGl2(backend) => backend.on_key_event(callback),
        }
    }

    fn clear_key_events(&mut self) {
        match self {
            RatzillaBackend::Dom(backend) => backend.clear_key_events(),
            RatzillaBackend::Canvas(backend) => backend.clear_key_events(),
            RatzillaBackend::WebGl2(backend) => backend.clear_key_events(),
        }
    }
}

/// Backend wrapper that automatically tracks FPS by recording frames on each flush.
///
/// This wrapper delegates all Backend trait methods to the inner RatzillaBackend
/// while recording frame timing information when `flush()` is called successfully.
/// The FPS data can be accessed through the `fps` module functions.
pub struct FpsTrackingBackend {
    inner: RatzillaBackend,
}

impl FpsTrackingBackend {
    /// Create a new FPS tracking backend that wraps the given backend.
    ///
    /// Frame timing will be recorded automatically on each successful flush operation.
    pub fn new(backend: RatzillaBackend) -> Self {
        Self { inner: backend }
    }

    /// Get the backend type for the wrapped backend.
    pub fn backend_type(&self) -> BackendType {
        self.inner.backend_type()
    }
}

impl From<RatzillaBackend> for FpsTrackingBackend {
    fn from(backend: RatzillaBackend) -> Self {
        Self::new(backend)
    }
}

impl Backend for FpsTrackingBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a ratzilla::ratatui::buffer::Cell)>,
    {
        self.inner.draw(content)
    }

    fn flush(&mut self) -> io::Result<()> {
        let result = self.inner.flush();
        // Record frame after successful flush
        if result.is_ok() {
            fps::record_frame();
        }
        result
    }

    fn size(&self) -> io::Result<ratzilla::ratatui::layout::Size> {
        self.inner.size()
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<ratzilla::ratatui::layout::Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<ratzilla::ratatui::layout::Position>>(
        &mut self,
        position: P,
    ) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn window_size(&mut self) -> io::Result<ratzilla::ratatui::backend::WindowSize> {
        self.inner.window_size()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.clear(),
            _ => Err(io::Error::other("unimplemented")),
        }
    }
}

impl WebEventHandler for FpsTrackingBackend {
    fn on_mouse_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(MouseEvent) + 'static,
    {
        self.inner.on_mouse_event(callback)
    }

    fn clear_mouse_events(&mut self) {
        self.inner.clear_mouse_events()
    }

    fn on_key_event<F>(&mut self, callback: F) -> Result<(), Error>
    where
        F: FnMut(KeyEvent) + 'static,
    {
        self.inner.on_key_event(callback)
    }

    fn clear_key_events(&mut self) {
        self.inner.clear_key_events()
    }
}

/// Builder for creating terminals with different backend types and configuration options.
///
/// This builder provides a fluent API for configuring terminal and backend options
/// before creating a terminal instance. It supports automatic backend selection
/// from URL query parameters and includes FPS tracking by default.
///
/// # Backend Selection
///
/// The builder uses the following priority order for backend selection:
/// 1. `?backend=<type>` URL query parameter (dom, canvas, or webgl2)
/// 2. Fallback backend specified in `with_fallback()`
/// 3. Default backend (DOM)
///
/// # Example
///
/// ```rust
/// use examples_shared::backend::{BackendType, MultiBackendBuilder};
/// use ratzilla::backend::canvas::CanvasBackendOptions;
/// use ratzilla::backend::webgl2::WebGl2BackendOptions;
/// use ratzilla::ratatui::TerminalOptions;
///
/// let terminal = MultiBackendBuilder::with_fallback(BackendType::Dom)
///     .canvas_options(CanvasBackendOptions::new().grid_id("terminal-id"))
///     .webgl2_options(WebGl2BackendOptions::new().size((1200, 800)))
///     .build_terminal()?;
///
/// // Get backend type if needed
/// let backend_type = terminal.backend().backend_type();
/// ```
#[derive(Debug, Default)]
pub struct MultiBackendBuilder {
    default_backend: BackendType,

    terminal_options: TerminalOptions,
    canvas_options: CanvasBackendOptions,
    dom_options: DomBackendOptions,
    webgl2_options: WebGl2BackendOptions,
}

impl MultiBackendBuilder {
    /// Create a new builder with the specified fallback backend type.
    ///
    /// The fallback backend will be used if no backend is specified in the URL query parameters.
    pub fn with_fallback(default_backend: BackendType) -> Self {
        Self {
            default_backend,
            ..Self::default()
        }
    }

    /// Set terminal configuration options.
    ///
    /// These options control terminal behavior such as viewport behavior and drawing settings.
    pub fn terminal_options(mut self, options: TerminalOptions) -> Self {
        self.terminal_options = options;
        self
    }

    /// Set options for the Canvas backend.
    ///
    /// These options control Canvas 2D rendering behavior such as font settings,
    /// cursor appearance, and Unicode support.
    pub fn canvas_options(mut self, options: CanvasBackendOptions) -> Self {
        self.canvas_options = options;
        self
    }

    /// Set options for the DOM backend.
    ///
    /// These options control DOM rendering behavior such as accessibility features,
    /// element styling, and focus management.
    pub fn dom_options(mut self, options: DomBackendOptions) -> Self {
        self.dom_options = options;
        self
    }

    /// Set options for the WebGL2 backend.
    ///
    /// These options control WebGL2 rendering behavior such as shader configuration,
    /// GPU memory management, and performance settings.
    pub fn webgl2_options(mut self, options: WebGl2BackendOptions) -> Self {
        self.webgl2_options = options;
        self
    }

    /// Build the terminal with the configured options and backend selection.
    ///
    /// This method:
    /// 1. Determines the backend type from URL query parameters or fallback
    /// 2. Creates the appropriate backend with the configured options
    /// 3. Wraps the backend with FPS tracking
    /// 4. Creates and returns the terminal with the selected backend
    /// 5. Injects a backend footer into the DOM (best effort)
    ///
    /// # Returns
    ///
    /// The configured terminal instance. You can get the backend type using
    /// `terminal.backend().backend_type()` if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if backend creation or terminal initialization fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use examples_shared::backend::{BackendType, MultiBackendBuilder};
    /// let terminal = MultiBackendBuilder::with_fallback(BackendType::Canvas)
    ///     .build_terminal()?;
    ///
    /// // Get backend type if needed
    /// let backend_type = terminal.backend().backend_type();
    /// println!("Using {backend_type} backend");
    /// ```
    pub fn build_terminal(self) -> io::Result<Terminal<FpsTrackingBackend>> {
        let backend_type = parse_backend_from_url(self.default_backend);
        let backend = create_backend_with_options(
            backend_type,
            Some(self.dom_options),
            Some(self.canvas_options),
            Some(self.webgl2_options),
        )?;

        // Initialize FPS recorder
        fps::init_fps_recorder();

        // Wrap backend with FPS tracking
        let fps_backend: FpsTrackingBackend = backend.into();
        let terminal = Terminal::with_options(fps_backend, self.terminal_options)?;

        // Inject footer (ignore errors)
        let _ = inject_backend_footer(backend_type);

        Ok(terminal)
    }
}

impl From<BackendType> for MultiBackendBuilder {
    fn from(backend_type: BackendType) -> Self {
        MultiBackendBuilder::with_fallback(backend_type)
    }
}

/// Parse the backend type from URL query parameters, with fallback to default.
///
/// Checks for a `?backend=<type>` query parameter in the current page URL.
/// Valid backend types are "dom", "canvas", and "webgl2" (case-insensitive).
/// If no valid backend is found in the URL, returns the provided default.
fn parse_backend_from_url(default: BackendType) -> BackendType {
    window()
        .and_then(|w| w.location().href().ok())
        .and_then(|url| Url::new(url.as_str()).ok())
        .and_then(|url| url.search_params().get("backend"))
        .and_then(|backend| BackendType::try_from(backend).ok())
        .unwrap_or(default)
}

/// Create a backend instance with the specified type and options.
///
/// Creates the appropriate backend variant (DOM, Canvas, or WebGL2) using the provided
/// configuration options. Options default to `Default::default()` if `None` is provided.
///
/// # Arguments
///
/// * `backend_type` - The type of backend to create
/// * `dom_options` - Configuration options for DOM backend (if applicable)
/// * `canvas_options` - Configuration options for Canvas backend (if applicable)  
/// * `webgl2_options` - Configuration options for WebGL2 backend (if applicable)
///
/// # Returns
///
/// The created backend wrapped in a `RatzillaBackend` enum.
///
/// # Errors
///
/// Returns an error if the backend creation fails (e.g., WebGL2 not supported).
fn create_backend_with_options(
    backend_type: BackendType,
    dom_options: Option<DomBackendOptions>,
    canvas_options: Option<CanvasBackendOptions>,
    webgl2_options: Option<WebGl2BackendOptions>,
) -> io::Result<RatzillaBackend> {
    use RatzillaBackend::*;

    match backend_type {
        BackendType::Dom => Ok(Dom(DomBackend::new_with_options(
            dom_options.unwrap_or_default(),
        )?)),
        BackendType::Canvas => Ok(Canvas(CanvasBackend::new_with_options(
            canvas_options.unwrap_or_default(),
        )?)),
        BackendType::WebGl2 => Ok(WebGl2(WebGl2Backend::new_with_options(
            webgl2_options.unwrap_or_default(),
        )?)),
    }
}
