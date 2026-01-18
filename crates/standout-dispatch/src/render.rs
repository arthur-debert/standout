//! Render function abstraction.
//!
//! Defines the contract between dispatch and renderers.
//! Dispatch doesn't know about templates - it just knows that
//! for each command there's a function that turns data into a string.

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
