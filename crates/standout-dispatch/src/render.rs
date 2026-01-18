//! Render function abstraction.
//!
//! This module defines the contract between dispatch and renderers. The key design
//! principle is that **dispatch is render-agnostic**: it doesn't know about templates,
//! themes, output formats, or any rendering implementation details.
//!
//! # Design Rationale
//!
//! The render handler is a **pluggable callback** that the consuming framework provides.
//! This separation exists because:
//!
//! 1. **Flexibility**: Different applications may use different renderers (or none at all)
//! 2. **Separation of concerns**: Business logic (handlers) shouldn't know about presentation
//! 3. **Runtime configuration**: Format/theme decisions happen at runtime (from CLI args),
//!    not at compile time
//!
//! # The Closure Pattern
//!
//! Render handlers capture their context (format, theme, etc.) in a closure:
//!
//! ```rust,ignore
//! // At runtime, after parsing --output=json:
//! let format = OutputMode::Json;
//! let theme = &app.theme;
//! let templates = &app.templates;
//!
//! // Create render handler with context baked in
//! let render_handler = from_fn(move |data, _mode| {
//!     render_with_format(templates, theme, format, data)
//! });
//! ```
//!
//! Dispatch calls `render_handler(data, mode)` without knowing what's inside the closure.
//! All format/theme/template logic lives in the closure, created by the framework layer.
//!
//! # TextMode Parameter
//!
//! The [`TextMode`] parameter exists for simple use cases where the render handler
//! wants dispatch to pass through a hint about text styling. For full-featured
//! frameworks like `standout`, the format is typically captured in the closure instead,
//! and TextMode can be ignored.
//!
//! # Thread Safety
//!
//! Two variants are provided:
//! - [`RenderFn`]: Thread-safe (`Send + Sync`), uses `Arc`
//! - [`LocalRenderFn`]: Single-threaded, uses `Rc<RefCell>`, allows `FnMut`

use crate::TextMode;
use std::sync::Arc;

/// The render function signature.
///
/// Takes handler data (as JSON) and a text mode, returns formatted output.
/// For dispatch-only users, this can be a simple closure that ignores TextMode.
/// For standout users, this wraps template rendering with style processing.
pub type RenderFn =
    Arc<dyn Fn(&serde_json::Value, TextMode) -> Result<String, RenderError> + Send + Sync>;

/// A local (non-Send) render function for LocalApp.
pub type LocalRenderFn = std::rc::Rc<
    std::cell::RefCell<dyn FnMut(&serde_json::Value, TextMode) -> Result<String, RenderError>>,
>;

/// Errors that can occur during rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Template rendering failed
    #[error("render error: {0}")]
    Render(String),

    /// Data serialization failed
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Other error
    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for RenderError {
    fn from(e: serde_json::Error) -> Self {
        RenderError::Serialization(e.to_string())
    }
}

/// Creates a simple identity render function that converts data to string.
///
/// This is useful for dispatch-only users who don't need template rendering.
/// The TextMode is ignored since there's no style processing.
pub fn identity_render() -> RenderFn {
    Arc::new(|data, _mode| Ok(data.to_string()))
}

/// Creates a render function that formats data as pretty JSON.
///
/// Useful for debugging or simple output needs.
pub fn json_render() -> RenderFn {
    Arc::new(|data, _mode| {
        serde_json::to_string_pretty(data).map_err(|e| RenderError::Serialization(e.to_string()))
    })
}

/// Creates a render function from a closure.
///
/// This is the primary way for dispatch-only users to provide custom rendering.
pub fn from_fn<F>(f: F) -> RenderFn
where
    F: Fn(&serde_json::Value, TextMode) -> Result<String, RenderError> + Send + Sync + 'static,
{
    Arc::new(f)
}

/// Creates a local render function from a FnMut closure.
pub fn from_fn_mut<F>(f: F) -> LocalRenderFn
where
    F: FnMut(&serde_json::Value, TextMode) -> Result<String, RenderError> + 'static,
{
    std::rc::Rc::new(std::cell::RefCell::new(f))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_identity_render() {
        let render = identity_render();
        let data = json!({"key": "value"});
        let result = render(&data, TextMode::Plain).unwrap();
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_json_render() {
        let render = json_render();
        let data = json!({"name": "test"});
        let result = render(&data, TextMode::Styled).unwrap();
        assert!(result.contains("\"name\": \"test\""));
    }

    #[test]
    fn test_from_fn() {
        let render = from_fn(|data, mode| {
            let prefix = match mode {
                TextMode::Styled => "[STYLED] ",
                TextMode::Plain => "[PLAIN] ",
                TextMode::Debug => "[DEBUG] ",
            };
            Ok(format!("{}{}", prefix, data))
        });

        let data = json!("hello");
        assert!(render(&data, TextMode::Styled)
            .unwrap()
            .starts_with("[STYLED]"));
        assert!(render(&data, TextMode::Plain)
            .unwrap()
            .starts_with("[PLAIN]"));
    }
}
