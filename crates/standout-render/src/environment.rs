//! Injectable environment detection.
//!
//! This module centralizes process-global detection of terminal properties
//! — width, TTY status, and ANSI color capability — behind overridable
//! function pointers so tests can force specific values without touching
//! real environment state.
//!
//! It follows the same pattern used by
//! [`set_theme_detector`](crate::set_theme_detector) and
//! [`set_icon_detector`](crate::set_icon_detector).
//!
//! # Usage
//!
//! In application code, call the `detect_*` functions. They resolve to real
//! terminal queries by default:
//!
//! ```rust
//! use standout_render::{detect_terminal_width, detect_is_tty, detect_color_capability};
//!
//! let _width = detect_terminal_width();
//! let _tty = detect_is_tty();
//! let _color = detect_color_capability();
//! ```
//!
//! In tests, override any of them with a closure:
//!
//! ```rust
//! use standout_render::{set_terminal_width_detector, detect_terminal_width};
//!
//! set_terminal_width_detector(|| Some(80));
//! assert_eq!(detect_terminal_width(), Some(80));
//! ```
//!
//! Overrides are process-global, so tests that set them should be annotated
//! with `#[serial]` (via the `serial_test` crate).

use console::Term;
use once_cell::sync::Lazy;
use std::sync::Mutex;

type WidthDetector = fn() -> Option<usize>;
type TtyDetector = fn() -> bool;
type ColorDetector = fn() -> bool;

static WIDTH_DETECTOR: Lazy<Mutex<WidthDetector>> =
    Lazy::new(|| Mutex::new(default_width_detector));
static TTY_DETECTOR: Lazy<Mutex<TtyDetector>> = Lazy::new(|| Mutex::new(default_tty_detector));
static COLOR_DETECTOR: Lazy<Mutex<ColorDetector>> =
    Lazy::new(|| Mutex::new(default_color_detector));

/// Overrides the detector used to query terminal width.
///
/// The detector returns `Some(cols)` when a width can be determined and
/// `None` when output is not a terminal. Useful to force a fixed width in
/// snapshot tests.
pub fn set_terminal_width_detector(detector: WidthDetector) {
    *WIDTH_DETECTOR.lock().unwrap() = detector;
}

/// Overrides the detector used to check whether stdout is a TTY.
pub fn set_tty_detector(detector: TtyDetector) {
    *TTY_DETECTOR.lock().unwrap() = detector;
}

/// Overrides the detector used to check whether ANSI color is supported on
/// stdout.
///
/// This is what [`OutputMode::Auto`](crate::OutputMode::Auto) consults to
/// decide between applying and stripping style tags.
pub fn set_color_capability_detector(detector: ColorDetector) {
    *COLOR_DETECTOR.lock().unwrap() = detector;
}

/// Returns the current terminal width in columns, or `None` when unavailable.
pub fn detect_terminal_width() -> Option<usize> {
    (*WIDTH_DETECTOR.lock().unwrap())()
}

/// Returns `true` when stdout is attached to a terminal.
pub fn detect_is_tty() -> bool {
    (*TTY_DETECTOR.lock().unwrap())()
}

/// Returns `true` when ANSI color output is supported on stdout.
pub fn detect_color_capability() -> bool {
    (*COLOR_DETECTOR.lock().unwrap())()
}

fn default_width_detector() -> Option<usize> {
    terminal_size::terminal_size().map(|(w, _)| w.0 as usize)
}

fn default_tty_detector() -> bool {
    Term::stdout().is_term()
}

fn default_color_detector() -> bool {
    Term::stdout().features().colors_supported()
}

/// Resets every detector to its default (real-terminal) implementation.
///
/// Tests that installed overrides should call this in teardown to avoid
/// leaking state into sibling tests.
pub fn reset_detectors() {
    set_terminal_width_detector(default_width_detector);
    set_tty_detector(default_tty_detector);
    set_color_capability_detector(default_color_detector);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn width_override_is_honored() {
        set_terminal_width_detector(|| Some(42));
        assert_eq!(detect_terminal_width(), Some(42));
        set_terminal_width_detector(|| None);
        assert_eq!(detect_terminal_width(), None);
        reset_detectors();
    }

    #[test]
    #[serial]
    fn tty_override_is_honored() {
        set_tty_detector(|| true);
        assert!(detect_is_tty());
        set_tty_detector(|| false);
        assert!(!detect_is_tty());
        reset_detectors();
    }

    #[test]
    #[serial]
    fn color_override_is_honored() {
        set_color_capability_detector(|| true);
        assert!(detect_color_capability());
        set_color_capability_detector(|| false);
        assert!(!detect_color_capability());
        reset_detectors();
    }

    #[test]
    #[serial]
    fn reset_restores_defaults() {
        set_terminal_width_detector(|| Some(1));
        set_tty_detector(|| true);
        set_color_capability_detector(|| true);

        reset_detectors();

        let _ = detect_terminal_width();
        let _ = detect_is_tty();
        let _ = detect_color_capability();
    }
}
