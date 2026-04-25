//! Storage for resolved inputs, keyed by name.
//!
//! [`Inputs`] is a name-keyed, type-safe container for values produced by
//! [`InputChain`](crate::InputChain) resolution. It exists because
//! `TypeId`-keyed containers (the common pattern for request-scoped state in
//! frameworks) cannot disambiguate two inputs of the same type — for example,
//! a command with both a `body: String` and a `title: String` input.
//!
//! Framework integrations resolve each registered chain and stash the result
//! here under the user-chosen name. Handlers retrieve by `(name, type)`.
//!
//! # Example
//!
//! ```
//! use standout_input::{InputSourceKind, Inputs, ResolvedInput};
//!
//! let mut inputs = Inputs::new();
//! inputs.insert(
//!     "body",
//!     ResolvedInput { value: "hello".to_string(), source: InputSourceKind::Arg },
//! );
//!
//! let body: &String = inputs.get("body").unwrap();
//! assert_eq!(body, "hello");
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;

use crate::collector::{InputSourceKind, ResolvedInput};

/// Name-keyed storage for resolved inputs.
///
/// Each entry stores the resolved `T` value boxed as `dyn Any + Send + Sync`,
/// while its [`InputSourceKind`] metadata is tracked separately on the
/// internal entry. Lookups are by `(name, T)` — wrong-type lookups return
/// `None` rather than panicking.
#[derive(Default)]
pub struct Inputs {
    entries: HashMap<&'static str, Entry>,
}

struct Entry {
    type_id: TypeId,
    type_name: &'static str,
    source: InputSourceKind,
    value: Box<dyn Any + Send + Sync>,
}

impl Inputs {
    /// Create an empty `Inputs` bag.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert a resolved input under `name`.
    ///
    /// Returns the previous entry's source kind if `name` was already present.
    pub fn insert<T>(
        &mut self,
        name: &'static str,
        resolved: ResolvedInput<T>,
    ) -> Option<InputSourceKind>
    where
        T: Send + Sync + 'static,
    {
        let prev = self.entries.insert(
            name,
            Entry {
                type_id: TypeId::of::<T>(),
                type_name: std::any::type_name::<T>(),
                source: resolved.source,
                value: Box::new(resolved.value),
            },
        );
        prev.map(|e| e.source)
    }

    /// Get a reference to the value stored under `name`, if it exists and has
    /// type `T`.
    ///
    /// Returns `None` if no entry exists or the stored type does not match.
    pub fn get<T: 'static>(&self, name: &str) -> Option<&T> {
        let entry = self.entries.get(name)?;
        if entry.type_id != TypeId::of::<T>() {
            return None;
        }
        entry.value.downcast_ref::<T>()
    }

    /// Get the value stored under `name`, returning a descriptive error if
    /// missing or of the wrong type.
    pub fn get_required<T: 'static>(&self, name: &str) -> Result<&T, MissingInput> {
        let Some(entry) = self.entries.get(name) else {
            return Err(MissingInput::NotRegistered {
                name: name.to_string(),
            });
        };
        if entry.type_id != TypeId::of::<T>() {
            return Err(MissingInput::TypeMismatch {
                name: name.to_string(),
                expected: std::any::type_name::<T>(),
                actual: entry.type_name,
            });
        }
        entry
            .value
            .downcast_ref::<T>()
            .ok_or_else(|| MissingInput::TypeMismatch {
                name: name.to_string(),
                expected: std::any::type_name::<T>(),
                actual: entry.type_name,
            })
    }

    /// Get the [`InputSourceKind`] that provided `name`, if it exists.
    pub fn source_of(&self, name: &str) -> Option<InputSourceKind> {
        self.entries.get(name).map(|e| e.source)
    }

    /// Returns true if `name` has been resolved.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Number of resolved inputs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no inputs have been resolved.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over `(name, source)` pairs.
    pub fn iter_sources(&self) -> impl Iterator<Item = (&'static str, InputSourceKind)> + '_ {
        self.entries
            .iter()
            .map(|(name, entry)| (*name, entry.source))
    }
}

impl fmt::Debug for Inputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Inputs");
        for (name, entry) in &self.entries {
            s.field(
                name,
                &format_args!("{} from {}", entry.type_name, entry.source),
            );
        }
        s.finish()
    }
}

/// Error returned when a named input is missing or stored under a different type.
#[derive(Debug, thiserror::Error)]
pub enum MissingInput {
    /// No input was registered for the given name.
    #[error("no input named `{name}` was registered for this command")]
    NotRegistered {
        /// The requested input name.
        name: String,
    },
    /// An input is registered but stored under a different type.
    #[error("input `{name}` is registered as `{actual}`, not `{expected}`")]
    TypeMismatch {
        /// The requested input name.
        name: String,
        /// The type the caller asked for.
        expected: &'static str,
        /// The type actually stored.
        actual: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg<T>(value: T) -> ResolvedInput<T> {
        ResolvedInput {
            value,
            source: InputSourceKind::Arg,
        }
    }

    #[test]
    fn insert_and_get() {
        let mut inputs = Inputs::new();
        inputs.insert("body", arg("hello".to_string()));

        let body: &String = inputs.get("body").unwrap();
        assert_eq!(body, "hello");
    }

    #[test]
    fn get_missing_returns_none() {
        let inputs = Inputs::new();
        assert!(inputs.get::<String>("missing").is_none());
    }

    #[test]
    fn get_wrong_type_returns_none() {
        let mut inputs = Inputs::new();
        inputs.insert("body", arg("hello".to_string()));
        assert!(inputs.get::<u32>("body").is_none());
    }

    #[test]
    fn get_required_reports_missing() {
        let inputs = Inputs::new();
        let err = inputs.get_required::<String>("body").unwrap_err();
        assert!(matches!(err, MissingInput::NotRegistered { .. }));
        assert!(err.to_string().contains("body"));
    }

    #[test]
    fn get_required_reports_type_mismatch() {
        let mut inputs = Inputs::new();
        inputs.insert("body", arg("hello".to_string()));
        let err = inputs.get_required::<u32>("body").unwrap_err();
        match err {
            MissingInput::TypeMismatch {
                ref name,
                expected,
                actual,
            } => {
                assert_eq!(name, "body");
                assert!(expected.contains("u32"));
                assert!(actual.contains("String"));
            }
            other => panic!("expected TypeMismatch, got {:?}", other),
        }
    }

    #[test]
    fn two_inputs_of_same_type_do_not_collide() {
        let mut inputs = Inputs::new();
        inputs.insert("body", arg("the body".to_string()));
        inputs.insert("title", arg("the title".to_string()));

        assert_eq!(inputs.get::<String>("body").unwrap(), "the body");
        assert_eq!(inputs.get::<String>("title").unwrap(), "the title");
    }

    #[test]
    fn insert_returns_previous_source() {
        let mut inputs = Inputs::new();
        assert!(inputs.insert("body", arg("first".to_string())).is_none());
        let prev = inputs.insert(
            "body",
            ResolvedInput {
                value: "second".to_string(),
                source: InputSourceKind::Stdin,
            },
        );
        assert_eq!(prev, Some(InputSourceKind::Arg));
        assert_eq!(inputs.source_of("body"), Some(InputSourceKind::Stdin));
    }

    #[test]
    fn source_of_and_contains() {
        let mut inputs = Inputs::new();
        assert!(!inputs.contains("body"));
        inputs.insert("body", arg("x".to_string()));
        assert!(inputs.contains("body"));
        assert_eq!(inputs.source_of("body"), Some(InputSourceKind::Arg));
        assert_eq!(inputs.source_of("missing"), None);
    }

    #[test]
    fn iter_sources_yields_all_entries() {
        let mut inputs = Inputs::new();
        inputs.insert("body", arg("x".to_string()));
        inputs.insert(
            "yes",
            ResolvedInput {
                value: true,
                source: InputSourceKind::Flag,
            },
        );

        let mut pairs: Vec<_> = inputs.iter_sources().collect();
        pairs.sort_by_key(|(name, _)| *name);
        assert_eq!(
            pairs,
            vec![
                ("body", InputSourceKind::Arg),
                ("yes", InputSourceKind::Flag)
            ]
        );
    }

    #[test]
    fn len_and_is_empty() {
        let mut inputs = Inputs::new();
        assert!(inputs.is_empty());
        assert_eq!(inputs.len(), 0);
        inputs.insert("body", arg("x".to_string()));
        assert!(!inputs.is_empty());
        assert_eq!(inputs.len(), 1);
    }
}
