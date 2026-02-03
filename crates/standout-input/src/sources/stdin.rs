//! Stdin input source.

use std::sync::Arc;

use clap::ArgMatches;

use crate::collector::InputCollector;
use crate::env::{RealStdin, StdinReader};
use crate::InputError;

/// Collect input from piped stdin.
///
/// This source reads from stdin only when it is piped (not a terminal).
/// If stdin is a TTY, the source returns `None` to allow the chain to
/// continue to the next source.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, ArgSource, StdinSource};
///
/// // For: echo "hello" | myapp
/// let chain = InputChain::<String>::new()
///     .try_source(ArgSource::new("message"))
///     .try_source(StdinSource::new());
/// ```
///
/// # Testing
///
/// Use [`StdinSource::with_reader`] to inject a mock for testing:
///
/// ```ignore
/// use standout_input::{StdinSource, MockStdin};
///
/// let source = StdinSource::with_reader(MockStdin::piped("test input"));
/// ```
#[derive(Clone)]
pub struct StdinSource<R: StdinReader = RealStdin> {
    reader: Arc<R>,
    trim: bool,
}

impl StdinSource<RealStdin> {
    /// Create a new stdin source using real stdin.
    pub fn new() -> Self {
        Self {
            reader: Arc::new(RealStdin),
            trim: true,
        }
    }
}

impl Default for StdinSource<RealStdin> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: StdinReader> StdinSource<R> {
    /// Create a stdin source with a custom reader.
    ///
    /// This is primarily used for testing to inject mock stdin.
    pub fn with_reader(reader: R) -> Self {
        Self {
            reader: Arc::new(reader),
            trim: true,
        }
    }

    /// Control whether to trim whitespace from the input.
    ///
    /// Default is `true`.
    pub fn trim(mut self, trim: bool) -> Self {
        self.trim = trim;
        self
    }
}

impl<R: StdinReader + 'static> InputCollector<String> for StdinSource<R> {
    fn name(&self) -> &'static str {
        "stdin"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        // Stdin is available if it's piped (not a terminal)
        !self.reader.is_terminal()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<String>, InputError> {
        if self.reader.is_terminal() {
            return Ok(None);
        }

        let content = self
            .reader
            .read_to_string()
            .map_err(InputError::StdinFailed)?;

        if content.is_empty() {
            return Ok(None);
        }

        let result = if self.trim {
            content.trim().to_string()
        } else {
            content
        };

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }
}

/// Convenience function to read stdin if piped.
///
/// Returns `Ok(Some(content))` if stdin is piped and has content,
/// `Ok(None)` if stdin is a terminal or empty.
pub fn read_if_piped() -> Result<Option<String>, InputError> {
    let reader = RealStdin;
    if reader.is_terminal() {
        return Ok(None);
    }

    let content = reader.read_to_string().map_err(InputError::StdinFailed)?;

    if content.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(content.trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::MockStdin;
    use clap::Command;

    fn empty_matches() -> ArgMatches {
        Command::new("test").try_get_matches_from(["test"]).unwrap()
    }

    #[test]
    fn stdin_available_when_piped() {
        let source = StdinSource::with_reader(MockStdin::piped("content"));
        assert!(source.is_available(&empty_matches()));
    }

    #[test]
    fn stdin_unavailable_when_terminal() {
        let source = StdinSource::with_reader(MockStdin::terminal());
        assert!(!source.is_available(&empty_matches()));
    }

    #[test]
    fn stdin_reads_piped_content() {
        let source = StdinSource::with_reader(MockStdin::piped("hello world"));
        let result = source.collect(&empty_matches()).unwrap();
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn stdin_trims_whitespace() {
        let source = StdinSource::with_reader(MockStdin::piped("  hello  \n"));
        let result = source.collect(&empty_matches()).unwrap();
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn stdin_no_trim() {
        let source = StdinSource::with_reader(MockStdin::piped("  hello  \n")).trim(false);
        let result = source.collect(&empty_matches()).unwrap();
        assert_eq!(result, Some("  hello  \n".to_string()));
    }

    #[test]
    fn stdin_returns_none_for_empty() {
        let source = StdinSource::with_reader(MockStdin::piped_empty());
        let result = source.collect(&empty_matches()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn stdin_returns_none_for_whitespace_only() {
        let source = StdinSource::with_reader(MockStdin::piped("   \n\t  "));
        let result = source.collect(&empty_matches()).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn stdin_returns_none_when_terminal() {
        let source = StdinSource::with_reader(MockStdin::terminal());
        let result = source.collect(&empty_matches()).unwrap();
        assert_eq!(result, None);
    }
}
