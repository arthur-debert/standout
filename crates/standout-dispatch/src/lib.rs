//! Command dispatch and routing for clap-based CLIs.
//!
//! `standout-dispatch` provides the command routing, handler execution, and
//! hook system for CLI applications. It's designed to work with any renderer
//! (or no renderer at all for simple imperative output).
//!
//! # Features
//!
//! - **Command routing**: Map command names to handler functions
//! - **Handler traits**: Thread-safe (`Handler`) and local (`LocalHandler`) variants
//! - **Hook system**: Pre/post dispatch and post-output hooks
//! - **Output modes**: Auto TTY detection, structured output (JSON/YAML/CSV/XML)
//! - **Clap integration**: `--output` flag injection, argument parsing
//!
//! # Output Mode Ownership
//!
//! Dispatch owns the `OutputMode` enum and handles:
//! - Auto mode (TTY detection to choose Term vs Text)
//! - Structured serialization (JSON, YAML, CSV, XML)
//! - Passing `TextMode` to render functions for text output
//!
//! Renderers receive `TextMode` and handle:
//! - Styled: Apply styles (ANSI escape codes)
//! - Plain: Strip style tags
//! - Debug: Keep tags visible
//!
//! # Usage Without Rendering
//!
//! For dispatch-only usage (no templates, no styles):
//!
//! ```rust,ignore
//! use standout_dispatch::{Dispatcher, Output, TextMode};
//!
//! Dispatcher::builder()
//!     .command("list", list_handler, |data, _mode| {
//!         // Imperative formatting - TextMode ignored
//!         Ok(format_list_output(&data))
//!     })
//!     .build()?
//!     .run(cmd, args);
//! ```
//!
//! # Integration with standout-render
//!
//! The `standout` crate provides integration that wires templates to render functions:
//!
//! ```rust,ignore
//! use standout::{App, embed_templates};
//!
//! App::builder()
//!     .templates(embed_templates!("src/templates"))
//!     .command("list", list_handler, "list")  // template name
//!     .build()?
//!     .run(cmd, args);
//! ```

// Core modules
mod dispatch;
mod handler;
mod hooks;
mod output;
mod render;
mod serialize;

// Re-export core types
pub use dispatch::{
    extract_command_path, get_deepest_matches, has_subcommand, insert_default_command,
    path_to_string, string_to_path,
};

pub use handler::{
    CommandContext, FnHandler, Handler, HandlerResult, LocalFnHandler, LocalHandler, Output,
    RunResult,
};

pub use hooks::{
    HookError, HookPhase, Hooks, PostDispatchFn, PostOutputFn, PreDispatchFn, RenderedOutput,
};

pub use output::{OutputDestination, OutputMode, TextMode};

pub use render::{
    from_fn, from_fn_mut, identity_render, json_render, LocalRenderFn, RenderError, RenderFn,
};

pub use serialize::{
    serialize_csv, serialize_structured, to_json, to_xml, to_yaml, SerializeError,
};
