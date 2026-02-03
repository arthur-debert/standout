//! CLI argument input sources.

use clap::ArgMatches;

use crate::collector::{InputCollector, InputSourceKind, ResolvedInput};
use crate::InputError;

/// Collect input from a CLI argument.
///
/// This source reads a string value from a clap argument. It is available
/// when the argument was provided by the user.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, ArgSource};
///
/// // For: myapp --message "hello"
/// let chain = InputChain::<String>::new()
///     .try_source(ArgSource::new("message"));
/// ```
#[derive(Debug, Clone)]
pub struct ArgSource {
    name: String,
}

impl ArgSource {
    /// Create a new argument source.
    ///
    /// The `name` should match the argument name defined in clap.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Get the argument name.
    pub fn arg_name(&self) -> &str {
        &self.name
    }
}

impl InputCollector<String> for ArgSource {
    fn name(&self) -> &'static str {
        "argument"
    }

    fn is_available(&self, matches: &ArgMatches) -> bool {
        matches.contains_id(&self.name) && matches.get_one::<String>(&self.name).is_some()
    }

    fn collect(&self, matches: &ArgMatches) -> Result<Option<String>, InputError> {
        Ok(matches.get_one::<String>(&self.name).cloned())
    }
}

/// Collect input from a CLI flag.
///
/// This source reads a boolean flag value. It is always available since
/// flags have a default value of `false`.
///
/// # Example
///
/// ```ignore
/// use standout_input::{InputChain, FlagSource};
///
/// // For: myapp --verbose
/// let chain = InputChain::<bool>::new()
///     .try_source(FlagSource::new("verbose"));
/// ```
#[derive(Debug, Clone)]
pub struct FlagSource {
    name: String,
    invert: bool,
}

impl FlagSource {
    /// Create a new flag source.
    ///
    /// The `name` should match the flag name defined in clap.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            invert: false,
        }
    }

    /// Invert the flag value.
    ///
    /// Useful for patterns like `--no-editor` where the flag being set
    /// means "don't use editor" (i.e., `false` for "use editor").
    pub fn inverted(mut self) -> Self {
        self.invert = true;
        self
    }

    /// Get the flag name.
    pub fn flag_name(&self) -> &str {
        &self.name
    }
}

impl InputCollector<bool> for FlagSource {
    fn name(&self) -> &'static str {
        "flag"
    }

    fn is_available(&self, matches: &ArgMatches) -> bool {
        // Flags are always "available" - they default to false
        matches.contains_id(&self.name)
    }

    fn collect(&self, matches: &ArgMatches) -> Result<Option<bool>, InputError> {
        let value = matches.get_flag(&self.name);
        let result = if self.invert { !value } else { value };

        // Only return Some if the flag was explicitly set (true)
        // This allows the chain to continue if the flag wasn't provided
        if matches.get_flag(&self.name) {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
}

/// Resolve a flag source to a [`ResolvedInput`].
impl FlagSource {
    /// Resolve the flag, returning metadata about the source.
    pub fn resolve(&self, matches: &ArgMatches) -> Result<ResolvedInput<bool>, InputError> {
        let value = matches.get_flag(&self.name);
        let result = if self.invert { !value } else { value };

        Ok(ResolvedInput {
            value: result,
            source: InputSourceKind::Flag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    fn make_matches(args: &[&str]) -> ArgMatches {
        Command::new("test")
            .arg(Arg::new("message").long("message").short('m'))
            .arg(
                Arg::new("verbose")
                    .long("verbose")
                    .short('v')
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("no-editor")
                    .long("no-editor")
                    .action(clap::ArgAction::SetTrue),
            )
            .try_get_matches_from(args)
            .unwrap()
    }

    #[test]
    fn arg_source_available_when_provided() {
        let matches = make_matches(&["test", "--message", "hello"]);
        let source = ArgSource::new("message");

        assert!(source.is_available(&matches));
        assert_eq!(source.collect(&matches).unwrap(), Some("hello".to_string()));
    }

    #[test]
    fn arg_source_unavailable_when_missing() {
        let matches = make_matches(&["test"]);
        let source = ArgSource::new("message");

        assert!(!source.is_available(&matches));
        assert_eq!(source.collect(&matches).unwrap(), None);
    }

    #[test]
    fn flag_source_returns_some_when_set() {
        let matches = make_matches(&["test", "--verbose"]);
        let source = FlagSource::new("verbose");

        assert!(source.is_available(&matches));
        assert_eq!(source.collect(&matches).unwrap(), Some(true));
    }

    #[test]
    fn flag_source_returns_none_when_not_set() {
        let matches = make_matches(&["test"]);
        let source = FlagSource::new("verbose");

        // Flag is "available" (defined) but returns None if not explicitly set
        assert!(source.is_available(&matches));
        assert_eq!(source.collect(&matches).unwrap(), None);
    }

    #[test]
    fn flag_source_inverted() {
        let matches = make_matches(&["test", "--no-editor"]);
        let source = FlagSource::new("no-editor").inverted();

        // --no-editor is set (true), but inverted means "use editor = false"
        assert_eq!(source.collect(&matches).unwrap(), Some(false));
    }

    #[test]
    fn flag_source_inverted_not_set() {
        let matches = make_matches(&["test"]);
        let source = FlagSource::new("no-editor").inverted();

        // Flag not set, so returns None (not inverted false)
        assert_eq!(source.collect(&matches).unwrap(), None);
    }
}
