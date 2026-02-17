//! Icon mode detection for adaptive icon rendering.
//!
//! This module provides icon mode detection for icons that adapt between
//! classic Unicode characters and Nerd Font glyphs.
//!
//! # Usage
//!
//! Icon mode detection is typically handled automatically by the render
//! functions. Use [`set_icon_detector`] to override detection for testing.
//!
//! ```rust
//! use standout_render::{IconMode, set_icon_detector};
//!
//! // Force Nerd Font mode for testing
//! set_icon_detector(|| IconMode::NerdFont);
//!
//! // Force classic mode
//! set_icon_detector(|| IconMode::Classic);
//! ```
//!
//! # Auto Detection
//!
//! In [`IconMode::Auto`] (the default), the icon mode is resolved by checking
//! the `NERD_FONT` environment variable. If set to `1` or `true`, Nerd Font
//! mode is used; otherwise classic mode is used.
//!
//! There is no reliable way to automatically detect Nerd Font availability
//! in a terminal. The environment variable approach is the community standard
//! used by tools like Starship and Oh My Posh.

use once_cell::sync::Lazy;
use std::sync::Mutex;

/// The icon rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconMode {
    /// Use classic Unicode characters (works in all terminals).
    Classic,
    /// Use Nerd Font glyphs (requires a Nerd Font to be installed).
    NerdFont,
    /// Auto-detect: check `NERD_FONT` env var, fall back to Classic.
    Auto,
}

type IconDetector = fn() -> IconMode;

static ICON_DETECTOR: Lazy<Mutex<IconDetector>> = Lazy::new(|| Mutex::new(default_icon_detector));

/// Overrides the detector used to determine icon mode.
///
/// This is useful for testing or when you want to force a specific icon mode.
///
/// # Example
///
/// ```rust
/// use standout_render::{IconMode, set_icon_detector};
///
/// // Force Nerd Font mode for testing
/// set_icon_detector(|| IconMode::NerdFont);
/// ```
pub fn set_icon_detector(detector: IconDetector) {
    let mut guard = ICON_DETECTOR.lock().unwrap();
    *guard = detector;
}

/// Detects the current icon mode.
///
/// Uses the configured detector (default: auto-detect via `NERD_FONT` env var).
/// Always returns a resolved mode (`Classic` or `NerdFont`), never `Auto`.
///
/// The detector can be overridden via [`set_icon_detector`] for testing.
///
/// # Returns
///
/// - [`IconMode::NerdFont`] if Nerd Font is detected/configured
/// - [`IconMode::Classic`] otherwise
pub fn detect_icon_mode() -> IconMode {
    let detector = ICON_DETECTOR.lock().unwrap();
    let mode = (*detector)();
    match mode {
        IconMode::Auto => resolve_auto(),
        other => other,
    }
}

/// Resolves Auto mode by checking the `NERD_FONT` environment variable.
fn resolve_auto() -> IconMode {
    match std::env::var("NERD_FONT") {
        Ok(val)
            if val == "1"
                || val.eq_ignore_ascii_case("true")
                || val.eq_ignore_ascii_case("yes") =>
        {
            IconMode::NerdFont
        }
        _ => IconMode::Classic,
    }
}

fn default_icon_detector() -> IconMode {
    IconMode::Auto
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_detect_icon_mode_default_is_classic() {
        // Reset to default detector
        set_icon_detector(default_icon_detector);
        // Without NERD_FONT env var, should resolve to Classic
        std::env::remove_var("NERD_FONT");
        let mode = detect_icon_mode();
        assert_eq!(mode, IconMode::Classic);
    }

    #[test]
    #[serial]
    fn test_detect_icon_mode_with_env_var() {
        set_icon_detector(default_icon_detector);
        std::env::set_var("NERD_FONT", "1");
        let mode = detect_icon_mode();
        assert_eq!(mode, IconMode::NerdFont);
        std::env::remove_var("NERD_FONT");
    }

    #[test]
    #[serial]
    fn test_detect_icon_mode_with_env_var_true() {
        set_icon_detector(default_icon_detector);
        std::env::set_var("NERD_FONT", "true");
        let mode = detect_icon_mode();
        assert_eq!(mode, IconMode::NerdFont);
        std::env::remove_var("NERD_FONT");
    }

    #[test]
    #[serial]
    fn test_detect_icon_mode_with_env_var_yes() {
        set_icon_detector(default_icon_detector);
        std::env::set_var("NERD_FONT", "YES");
        let mode = detect_icon_mode();
        assert_eq!(mode, IconMode::NerdFont);
        std::env::remove_var("NERD_FONT");
    }

    #[test]
    #[serial]
    fn test_detect_icon_mode_with_env_var_false() {
        set_icon_detector(default_icon_detector);
        std::env::set_var("NERD_FONT", "0");
        let mode = detect_icon_mode();
        assert_eq!(mode, IconMode::Classic);
        std::env::remove_var("NERD_FONT");
    }

    #[test]
    #[serial]
    fn test_set_icon_detector_override() {
        set_icon_detector(|| IconMode::NerdFont);
        assert_eq!(detect_icon_mode(), IconMode::NerdFont);

        set_icon_detector(|| IconMode::Classic);
        assert_eq!(detect_icon_mode(), IconMode::Classic);

        // Reset
        set_icon_detector(default_icon_detector);
    }

    #[test]
    #[serial]
    fn test_detect_never_returns_auto() {
        set_icon_detector(|| IconMode::Auto);
        let mode = detect_icon_mode();
        assert_ne!(mode, IconMode::Auto);
        // Reset
        set_icon_detector(default_icon_detector);
    }
}
