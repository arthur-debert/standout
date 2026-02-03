//! Error types for input collection.

use std::io;

/// Errors that can occur during input collection.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    /// No editor found in environment.
    #[error("No editor found. Set VISUAL or EDITOR environment variable.")]
    NoEditor,

    /// User cancelled the editor without saving.
    #[error("Editor cancelled without saving.")]
    EditorCancelled,

    /// Editor process failed.
    #[error("Editor failed: {0}")]
    EditorFailed(#[source] io::Error),

    /// Failed to read from stdin.
    #[error("Failed to read stdin: {0}")]
    StdinFailed(#[source] io::Error),

    /// Failed to read from clipboard.
    #[error("Failed to read clipboard: {0}")]
    ClipboardFailed(String),

    /// User cancelled an interactive prompt.
    #[error("Prompt cancelled by user.")]
    PromptCancelled,

    /// Interactive prompt failed.
    #[error("Prompt failed: {0}")]
    PromptFailed(String),

    /// Input validation failed.
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// No input was provided and no default is available.
    #[error("No input provided and no default available.")]
    NoInput,

    /// Required CLI argument was not provided.
    #[error("Required argument '{0}' not provided.")]
    MissingArgument(String),

    /// Failed to parse argument value.
    #[error("Failed to parse argument '{name}': {reason}")]
    ParseError { name: String, reason: String },
}

impl InputError {
    /// Create a validation error.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::ValidationFailed(msg.into())
    }

    /// Create a parse error.
    pub fn parse(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ParseError {
            name: name.into(),
            reason: reason.into(),
        }
    }
}
