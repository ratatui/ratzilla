use ratzilla::web_sys::window;
use std::{cell::RefCell, thread_local};
use wasm_bindgen::JsValue;
use web_time::Instant;

thread_local! {
    /// Thread-local FPS recorder instance for shared use across examples
    static FPS_RECORDER: RefCell<Option<FpsRecorder>> = RefCell::new(None);
}

/// Records and calculates frames per second.
///
/// `FpsRecorder` keeps track of frame timings in a ring buffer and
/// provides methods to calculate the current frames per second.
pub struct FpsRecorder {
    /// Current position in the ring buffer
    tail: usize,
    /// Ring buffer of frame timestamps. Length is a power of 2 for
    /// fast modulus operations.
    recorded_frame: [Instant; 16],
}

impl FpsRecorder {
    /// Creates a new FPS recorder.
    pub fn new() -> Self {
        let recorder = Self {
            tail: 0,
            recorded_frame: [Instant::now(); 16],
        };

        debug_assert!(
            recorder.recorded_frame.len().is_power_of_two(),
            "recorded_frame length must be a power of two"
        );

        recorder
    }

    /// Records a new frame timestamp.
    pub fn record(&mut self) {
        self.recorded_frame[self.tail] = Instant::now();
        self.tail = (self.tail + 1) & (self.recorded_frame.len() - 1);
    }

    /// Calculates the current frames per second.
    pub fn fps(&self) -> f32 {
        // Find the newest recorded timestamp (the one just before tail)
        let newest_idx = if self.tail == 0 {
            self.recorded_frame.len() - 1
        } else {
            self.tail - 1
        };

        let elapsed = self.recorded_frame[newest_idx]
            .duration_since(self.recorded_frame[self.tail])
            .as_secs_f32()
            .max(0.001); // avoid division by zero

        // We have 16 frames, so there are 15 intervals between them
        (self.recorded_frame.len() - 1) as f32 / elapsed
    }
}

/// Initialize the global FPS recorder
pub fn init_fps_recorder() {
    FPS_RECORDER.with(|recorder| {
        *recorder.borrow_mut() = Some(FpsRecorder::new());
    });
}

/// Record a frame for FPS calculation
pub fn record_frame() {
    FPS_RECORDER.with(|recorder| {
        if let Some(ref mut fps_recorder) = *recorder.borrow_mut() {
            fps_recorder.record();
            // Update the footer FPS display
            let fps = fps_recorder.fps();
            update_fps_display(fps);
        }
    });
}

/// Get the current FPS value
pub fn get_current_fps() -> f32 {
    FPS_RECORDER.with(|recorder| {
        if let Some(ref fps_recorder) = *recorder.borrow() {
            fps_recorder.fps()
        } else {
            0.0
        }
    })
}

/// Update the FPS display in the footer
fn update_fps_display(fps: f32) {
    let _ = (|| -> Result<(), JsValue> {
        let fps_element = window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("ratzilla-fps"));

        if let Some(element) = fps_element {
            element.set_text_content(Some(&format!("{:.1}", fps)));
        }

        Ok(())
    })();
}
