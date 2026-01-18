//! Output mode control for dispatch.
//!
//! [`OutputMode`] determines how handler output is formatted, from terminal
//! styling to structured data serialization.
//!
//! [`TextMode`] is passed to render functions to control style tag processing.

use std::io::Write;
use std::path::PathBuf;

/// Controls how output is rendered.
///
/// This is the user-facing enum for the `--output` CLI flag.
///
/// # Variants
///
/// - `Auto` - Detect terminal capabilities (TTY → Term, pipe → Text)
/// - `Term` - Always apply terminal styling
/// - `Text` - Never apply styling (strip style tags)
/// - `TermDebug` - Keep style tags visible as `[name]text[/name]`
/// - `Json`, `Yaml`, `Xml`, `Csv` - Serialize data directly (skip templates)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Auto-detect: TTY gets Term, pipe gets Text
    #[default]
    Auto,
    /// Always use terminal styling
    Term,
    /// Never use styling (plain text)
    Text,
    /// Debug mode: render style names as bracket tags
    TermDebug,
    /// Serialize data as JSON (skips template rendering)
    Json,
    /// Serialize data as YAML (skips template rendering)
    Yaml,
    /// Serialize data as XML (skips template rendering)
    Xml,
    /// Serialize flattened data as CSV (skips template rendering)
    Csv,
}

impl OutputMode {
    /// Returns true if this is a structured output mode (JSON, YAML, XML, CSV).
    ///
    /// Structured modes serialize handler data directly, bypassing templates.
    pub fn is_structured(&self) -> bool {
        matches!(
            self,
            OutputMode::Json | OutputMode::Yaml | OutputMode::Xml | OutputMode::Csv
        )
    }

    /// Returns true if this is debug mode.
    pub fn is_debug(&self) -> bool {
        matches!(self, OutputMode::TermDebug)
    }

    /// Returns true if this is a text mode (requires a render function).
    pub fn is_text_mode(&self) -> bool {
        !self.is_structured()
    }

    /// Resolves Auto mode to concrete Term or Text based on TTY detection.
    ///
    /// For non-Auto modes, returns self unchanged.
    pub fn resolve_auto(&self) -> OutputMode {
        match self {
            OutputMode::Auto => {
                if atty::is(atty::Stream::Stdout) {
                    OutputMode::Term
                } else {
                    OutputMode::Text
                }
            }
            other => *other,
        }
    }

    /// Converts this output mode to a TextMode for render functions.
    ///
    /// Returns None for structured modes (which bypass rendering).
    pub fn to_text_mode(&self) -> Option<TextMode> {
        match self.resolve_auto() {
            OutputMode::Term => Some(TextMode::Styled),
            OutputMode::Text => Some(TextMode::Plain),
            OutputMode::TermDebug => Some(TextMode::Debug),
            OutputMode::Auto => unreachable!("resolve_auto should have resolved Auto"),
            _ => None, // Structured modes
        }
    }
}

/// How style tags should be processed by render functions.
///
/// This is passed from dispatch to render functions to control styling behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    /// Apply styles (generate ANSI escape codes)
    Styled,
    /// Strip style tags (plain text output)
    Plain,
    /// Keep style tags visible as `[name]text[/name]`
    Debug,
}

/// Destination for rendered output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputDestination {
    /// Write to standard output
    Stdout,
    /// Write to a specific file
    File(PathBuf),
}

impl OutputDestination {
    /// Writes text content to this destination.
    pub fn write_text(&self, content: &str) -> std::io::Result<()> {
        match self {
            OutputDestination::Stdout => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "{}", content)
            }
            OutputDestination::File(path) => {
                validate_path(path)?;
                std::fs::write(path, content)
            }
        }
    }

    /// Writes binary content to this destination.
    pub fn write_binary(&self, content: &[u8]) -> std::io::Result<()> {
        match self {
            OutputDestination::Stdout => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                handle.write_all(content)
            }
            OutputDestination::File(path) => {
                validate_path(path)?;
                std::fs::write(path, content)
            }
        }
    }
}

/// Validates that a file path's parent directory exists.
fn validate_path(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Parent directory does not exist: {}", parent.display()),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_mode_default_is_auto() {
        assert_eq!(OutputMode::default(), OutputMode::Auto);
    }

    #[test]
    fn test_output_mode_is_structured() {
        assert!(OutputMode::Json.is_structured());
        assert!(OutputMode::Yaml.is_structured());
        assert!(OutputMode::Xml.is_structured());
        assert!(OutputMode::Csv.is_structured());
        assert!(!OutputMode::Auto.is_structured());
        assert!(!OutputMode::Term.is_structured());
        assert!(!OutputMode::Text.is_structured());
        assert!(!OutputMode::TermDebug.is_structured());
    }

    #[test]
    fn test_output_mode_is_debug() {
        assert!(OutputMode::TermDebug.is_debug());
        assert!(!OutputMode::Auto.is_debug());
        assert!(!OutputMode::Term.is_debug());
        assert!(!OutputMode::Json.is_debug());
    }

    #[test]
    fn test_output_mode_to_text_mode() {
        assert_eq!(OutputMode::Term.to_text_mode(), Some(TextMode::Styled));
        assert_eq!(OutputMode::Text.to_text_mode(), Some(TextMode::Plain));
        assert_eq!(OutputMode::TermDebug.to_text_mode(), Some(TextMode::Debug));
        assert_eq!(OutputMode::Json.to_text_mode(), None);
        assert_eq!(OutputMode::Yaml.to_text_mode(), None);
    }

    #[test]
    fn test_resolve_auto_non_auto_unchanged() {
        assert_eq!(OutputMode::Term.resolve_auto(), OutputMode::Term);
        assert_eq!(OutputMode::Text.resolve_auto(), OutputMode::Text);
        assert_eq!(OutputMode::Json.resolve_auto(), OutputMode::Json);
    }

    #[test]
    fn test_write_text_to_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("output.txt");
        let dest = OutputDestination::File(file_path.clone());

        dest.write_text("hello").unwrap();

        let content = std::fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn test_write_binary_to_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("output.bin");
        let dest = OutputDestination::File(file_path.clone());

        dest.write_binary(&[1, 2, 3]).unwrap();

        let content = std::fs::read(&file_path).unwrap();
        assert_eq!(content, vec![1, 2, 3]);
    }

    #[test]
    fn test_write_to_invalid_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("missing").join("output.txt");
        let dest = OutputDestination::File(file_path);

        let result = dest.write_text("hello");
        assert!(result.is_err());
    }
}
