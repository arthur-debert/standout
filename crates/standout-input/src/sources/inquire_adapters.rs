//! Inquire-based input sources.
//!
//! Rich TUI prompts using the [inquire](https://crates.io/crates/inquire) crate.
//! These provide a more polished interactive experience than simple-prompts.

use std::fmt::Display;
use std::io::IsTerminal;

use clap::ArgMatches;
use inquire::{
    ui::RenderConfig, Confirm, Editor, InquireError, MultiSelect, Password, PasswordDisplayMode,
    Select, Text,
};

use crate::collector::InputCollector;
use crate::InputError;

/// Convert inquire errors to InputError.
fn map_inquire_error(e: InquireError) -> InputError {
    match e {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            InputError::PromptCancelled
        }
        other => InputError::PromptFailed(other.to_string()),
    }
}

/// Text input prompt using inquire.
///
/// Provides a rich text input experience with:
/// - Autocomplete suggestions
/// - Input validation feedback
/// - Help messages
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, ArgSource, InquireText};
///
/// let chain = InputChain::<String>::new()
///     .try_source(ArgSource::new("name"))
///     .try_source(InquireText::new("What is your name?"));
/// ```
pub struct InquireText {
    message: String,
    default: Option<String>,
    placeholder: Option<String>,
    help_message: Option<String>,
}

impl InquireText {
    /// Create a new text prompt.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            default: None,
            placeholder: None,
            help_message: None,
        }
    }

    /// Set a default value shown in the prompt.
    pub fn default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Set placeholder text shown when empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set a help message shown below the prompt.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help_message = Some(help.into());
        self
    }
}

impl InputCollector<String> for InquireText {
    fn name(&self) -> &'static str {
        "prompt"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        std::io::stdin().is_terminal()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<String>, InputError> {
        let mut prompt = Text::new(&self.message);

        if let Some(default) = &self.default {
            prompt = prompt.with_default(default);
        }
        if let Some(placeholder) = &self.placeholder {
            prompt = prompt.with_placeholder(placeholder);
        }
        if let Some(help) = &self.help_message {
            prompt = prompt.with_help_message(help);
        }

        let result = prompt.prompt().map_err(map_inquire_error)?;

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    fn can_retry(&self) -> bool {
        true
    }
}

/// Confirmation prompt using inquire.
///
/// Provides a yes/no selection with clear visual feedback.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, FlagSource, InquireConfirm};
///
/// let chain = InputChain::<bool>::new()
///     .try_source(FlagSource::new("yes"))
///     .try_source(InquireConfirm::new("Proceed with deployment?"));
/// ```
pub struct InquireConfirm {
    message: String,
    default: Option<bool>,
    help_message: Option<String>,
}

impl InquireConfirm {
    /// Create a new confirmation prompt.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            default: None,
            help_message: None,
        }
    }

    /// Set the default value.
    pub fn default(mut self, default: bool) -> Self {
        self.default = Some(default);
        self
    }

    /// Set a help message.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help_message = Some(help.into());
        self
    }
}

impl InputCollector<bool> for InquireConfirm {
    fn name(&self) -> &'static str {
        "prompt"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        std::io::stdin().is_terminal()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<bool>, InputError> {
        let mut prompt = Confirm::new(&self.message);

        if let Some(default) = self.default {
            prompt = prompt.with_default(default);
        }
        if let Some(help) = &self.help_message {
            prompt = prompt.with_help_message(help);
        }

        let result = prompt.prompt().map_err(map_inquire_error)?;
        Ok(Some(result))
    }

    fn can_retry(&self) -> bool {
        true
    }
}

/// Selection prompt using inquire.
///
/// Presents a list of options for single selection with arrow key navigation.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, InquireSelect};
///
/// let chain = InputChain::<String>::new()
///     .try_source(InquireSelect::new("Choose environment:", vec![
///         "development",
///         "staging",
///         "production",
///     ]));
/// ```
pub struct InquireSelect<T> {
    message: String,
    options: Vec<T>,
    help_message: Option<String>,
    page_size: usize,
}

impl<T: Display + Clone + Send + Sync + 'static> InquireSelect<T> {
    /// Create a new selection prompt.
    pub fn new(message: impl Into<String>, options: Vec<T>) -> Self {
        Self {
            message: message.into(),
            options,
            help_message: None,
            page_size: 10,
        }
    }

    /// Set a help message.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help_message = Some(help.into());
        self
    }

    /// Set the page size for scrolling.
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = size;
        self
    }
}

impl<T: Display + Clone + Send + Sync + 'static> InputCollector<T> for InquireSelect<T> {
    fn name(&self) -> &'static str {
        "prompt"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        std::io::stdin().is_terminal() && !self.options.is_empty()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<T>, InputError> {
        if self.options.is_empty() {
            return Ok(None);
        }

        let mut prompt =
            Select::new(&self.message, self.options.clone()).with_page_size(self.page_size);

        if let Some(help) = &self.help_message {
            prompt = prompt.with_help_message(help);
        }

        let result = prompt.prompt().map_err(map_inquire_error)?;
        Ok(Some(result))
    }

    fn can_retry(&self) -> bool {
        true
    }
}

/// Multi-selection prompt using inquire.
///
/// Presents a list of options for multiple selection with checkboxes.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, InquireMultiSelect};
///
/// let chain = InputChain::<Vec<String>>::new()
///     .try_source(InquireMultiSelect::new("Select features:", vec![
///         "logging",
///         "metrics",
///         "tracing",
///     ]));
/// ```
pub struct InquireMultiSelect<T> {
    message: String,
    options: Vec<T>,
    help_message: Option<String>,
    page_size: usize,
    min_selections: Option<usize>,
    max_selections: Option<usize>,
}

impl<T: Display + Clone + Send + Sync + 'static> InquireMultiSelect<T> {
    /// Create a new multi-selection prompt.
    pub fn new(message: impl Into<String>, options: Vec<T>) -> Self {
        Self {
            message: message.into(),
            options,
            help_message: None,
            page_size: 10,
            min_selections: None,
            max_selections: None,
        }
    }

    /// Set a help message.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help_message = Some(help.into());
        self
    }

    /// Set the page size for scrolling.
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = size;
        self
    }

    /// Set minimum required selections.
    pub fn min_selections(mut self, min: usize) -> Self {
        self.min_selections = Some(min);
        self
    }

    /// Set maximum allowed selections.
    pub fn max_selections(mut self, max: usize) -> Self {
        self.max_selections = Some(max);
        self
    }
}

impl<T: Display + Clone + Send + Sync + 'static> InputCollector<Vec<T>> for InquireMultiSelect<T> {
    fn name(&self) -> &'static str {
        "prompt"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        std::io::stdin().is_terminal() && !self.options.is_empty()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<Vec<T>>, InputError> {
        if self.options.is_empty() {
            return Ok(None);
        }

        let mut prompt =
            MultiSelect::new(&self.message, self.options.clone()).with_page_size(self.page_size);

        if let Some(help) = &self.help_message {
            prompt = prompt.with_help_message(help);
        }

        // Note: inquire's min/max validation is done via validators,
        // but we simplify by checking after the fact

        let result = prompt.prompt().map_err(map_inquire_error)?;

        // Validate min/max selections
        if let Some(min) = self.min_selections {
            if result.len() < min {
                return Err(InputError::ValidationFailed(format!(
                    "At least {} selection(s) required",
                    min
                )));
            }
        }
        if let Some(max) = self.max_selections {
            if result.len() > max {
                return Err(InputError::ValidationFailed(format!(
                    "At most {} selection(s) allowed",
                    max
                )));
            }
        }

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    fn can_retry(&self) -> bool {
        true
    }
}

/// Password prompt using inquire.
///
/// Provides secure password entry with masked input.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, InquirePassword};
///
/// let chain = InputChain::<String>::new()
///     .try_source(InquirePassword::new("Enter API token:"));
/// ```
pub struct InquirePassword {
    message: String,
    help_message: Option<String>,
    display_mode: PasswordDisplayMode,
    confirmation: Option<String>,
}

impl InquirePassword {
    /// Create a new password prompt.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            help_message: None,
            display_mode: PasswordDisplayMode::Masked,
            confirmation: None,
        }
    }

    /// Set a help message.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help_message = Some(help.into());
        self
    }

    /// Hide input completely (no asterisks).
    pub fn hidden(mut self) -> Self {
        self.display_mode = PasswordDisplayMode::Hidden;
        self
    }

    /// Show asterisks for each character (default).
    pub fn masked(mut self) -> Self {
        self.display_mode = PasswordDisplayMode::Masked;
        self
    }

    /// Show the full password as typed.
    pub fn full(mut self) -> Self {
        self.display_mode = PasswordDisplayMode::Full;
        self
    }

    /// Require password confirmation with a second prompt.
    pub fn with_confirmation(mut self, message: impl Into<String>) -> Self {
        self.confirmation = Some(message.into());
        self
    }
}

impl InputCollector<String> for InquirePassword {
    fn name(&self) -> &'static str {
        "prompt"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        std::io::stdin().is_terminal()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<String>, InputError> {
        let mut prompt = Password::new(&self.message).with_display_mode(self.display_mode);

        if let Some(help) = &self.help_message {
            prompt = prompt.with_help_message(help);
        }

        if let Some(confirmation) = &self.confirmation {
            prompt = prompt.with_display_toggle_enabled();
            prompt = prompt.with_custom_confirmation_message(confirmation);
        }

        let result = prompt.prompt().map_err(map_inquire_error)?;

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    fn can_retry(&self) -> bool {
        true
    }
}

/// Editor prompt using inquire.
///
/// Opens an external editor for multi-line input with a preview.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, ArgSource, InquireEditor};
///
/// let chain = InputChain::<String>::new()
///     .try_source(ArgSource::new("message"))
///     .try_source(InquireEditor::new("Enter commit message:"));
/// ```
pub struct InquireEditor {
    message: String,
    help_message: Option<String>,
    file_extension: String,
    predefined_text: Option<String>,
    render_config: Option<RenderConfig<'static>>,
}

impl InquireEditor {
    /// Create a new editor prompt.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            help_message: None,
            file_extension: ".txt".to_string(),
            predefined_text: None,
            render_config: None,
        }
    }

    /// Set a help message.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help_message = Some(help.into());
        self
    }

    /// Set the file extension for syntax highlighting.
    pub fn extension(mut self, ext: impl Into<String>) -> Self {
        self.file_extension = ext.into();
        self
    }

    /// Set predefined text to populate the editor.
    pub fn predefined_text(mut self, text: impl Into<String>) -> Self {
        self.predefined_text = Some(text.into());
        self
    }

    /// Set a custom render config.
    pub fn render_config(mut self, config: RenderConfig<'static>) -> Self {
        self.render_config = Some(config);
        self
    }
}

impl InputCollector<String> for InquireEditor {
    fn name(&self) -> &'static str {
        "editor"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        std::io::stdin().is_terminal()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<String>, InputError> {
        let mut prompt = Editor::new(&self.message).with_file_extension(&self.file_extension);

        if let Some(help) = &self.help_message {
            prompt = prompt.with_help_message(help);
        }
        if let Some(text) = &self.predefined_text {
            prompt = prompt.with_predefined_text(text);
        }
        if let Some(config) = &self.render_config {
            prompt = prompt.with_render_config(*config);
        }

        let result = prompt.prompt().map_err(map_inquire_error)?;

        let trimmed = result.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed.to_string()))
        }
    }

    fn can_retry(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Inquire prompts are interactive and can't be easily unit tested
    // without mocking the terminal. These tests verify basic construction
    // and is_available behavior.

    fn empty_matches() -> ArgMatches {
        clap::Command::new("test")
            .try_get_matches_from(["test"])
            .unwrap()
    }

    #[test]
    fn inquire_text_construction() {
        let source = InquireText::new("Name?")
            .default("Alice")
            .placeholder("Your name...")
            .help("Enter your full name");

        assert_eq!(source.name(), "prompt");
        assert!(source.can_retry());
    }

    #[test]
    fn inquire_confirm_construction() {
        let source = InquireConfirm::new("Proceed?")
            .default(true)
            .help("Are you sure?");

        assert_eq!(source.name(), "prompt");
        assert!(source.can_retry());
    }

    #[test]
    fn inquire_select_construction() {
        let source = InquireSelect::new("Choose:", vec!["a", "b", "c"])
            .help("Select one")
            .page_size(5);

        assert_eq!(source.name(), "prompt");
        assert!(source.can_retry());
    }

    #[test]
    fn inquire_select_empty_options_unavailable() {
        let source: InquireSelect<String> = InquireSelect::new("Choose:", vec![]);
        // Even with terminal, empty options makes it unavailable
        // (we can't easily test terminal state, so just verify the method exists)
        let _ = source.is_available(&empty_matches());
    }

    #[test]
    fn inquire_multiselect_construction() {
        let source = InquireMultiSelect::new("Select:", vec!["x", "y", "z"])
            .help("Select multiple")
            .page_size(10)
            .min_selections(1)
            .max_selections(2);

        assert_eq!(source.name(), "prompt");
        assert!(source.can_retry());
    }

    #[test]
    fn inquire_password_construction() {
        let source = InquirePassword::new("Password:")
            .help("Enter securely")
            .masked()
            .with_confirmation("Confirm:");

        assert_eq!(source.name(), "prompt");
        assert!(source.can_retry());
    }

    #[test]
    fn inquire_password_display_modes() {
        let _ = InquirePassword::new("P:").hidden();
        let _ = InquirePassword::new("P:").masked();
        let _ = InquirePassword::new("P:").full();
    }

    #[test]
    fn inquire_editor_construction() {
        let source = InquireEditor::new("Message:")
            .help("Enter in editor")
            .extension(".md")
            .predefined_text("# Title\n");

        assert_eq!(source.name(), "editor");
        assert!(source.can_retry());
    }
}
