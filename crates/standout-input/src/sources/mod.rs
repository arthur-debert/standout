//! Input source implementations.
//!
//! This module contains the built-in input sources:
//!
//! - [`ArgSource`] - Read from CLI arguments
//! - [`FlagSource`] - Read from CLI flags
//! - [`StdinSource`] - Read from piped stdin
//! - [`EnvSource`] - Read from environment variables
//! - [`ClipboardSource`] - Read from system clipboard
//! - [`DefaultSource`] - Provide a fallback value

mod arg;
mod clipboard;
mod default;
mod env;
mod stdin;

pub use arg::{ArgSource, FlagSource};
pub use clipboard::ClipboardSource;
pub use default::DefaultSource;
pub use env::EnvSource;
pub use stdin::{read_if_piped, StdinSource};
