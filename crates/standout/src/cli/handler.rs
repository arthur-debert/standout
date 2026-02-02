//! Command handler types for the declarative API.
//!
//! This module re-exports handler types from `standout-dispatch` and provides
//! the core types for building command handlers:
//!
//! - [`CommandContext`]: Environment information passed to handlers (command path only)
//! - [`Output`]: What a handler produces (render data, silent, or binary)
//! - [`HandlerResult`]: The result type for handlers (`Result<Output<T>, Error>`)
//! - [`RunResult`]: The result of running the CLI dispatcher
//! - [`Handler`]: Trait for command handlers (`&mut self`)
//!
//! # Design Note
//!
//! Handler types are defined in `standout-dispatch` because dispatch orchestrates
//! handler execution. The types are render-agnostic - handlers produce serializable
//! data, and the render layer (configured by standout) handles formatting.
//!
//! Output format (JSON, YAML, terminal, etc.) is NOT passed to handlers via
//! CommandContext. This is intentional: handlers should focus on business logic
//! and produce data, not make format decisions. If a handler truly needs to know
//! the output format, it can check the `--output` flag in ArgMatches directly.
//!
//! # Single-Threaded Design
//!
//! CLI applications are single-threaded: parse args → run one handler → output → exit.
//! Handlers use `&mut self` and `FnMut`, allowing natural Rust patterns without
//! forcing interior mutability wrappers (`Arc<Mutex<_>>`).
//!
//! ```rust,ignore
//! use standout::cli::{App, Handler, Output, HandlerResult, CommandContext};
//!
//! // Closure handler (most common)
//! App::builder()
//!     .command("list", |matches, ctx| {
//!         Ok(Output::Render(get_items()?))
//!     }, "{{ items }}")
//!
//! // Struct handler with mutable state
//! struct Database {
//!     connection: Connection,
//! }
//!
//! impl Database {
//!     fn query(&mut self) -> Vec<Row> { ... }
//! }
//!
//! impl Handler for Database {
//!     type Output = Vec<Row>;
//!     fn handle(&mut self, m: &ArgMatches, ctx: &CommandContext) -> HandlerResult<Vec<Row>> {
//!         Ok(Output::Render(self.query()))
//!     }
//! }
//! ```

// Re-export all handler types from standout-dispatch.
// These types are render-agnostic and focus on handler execution.
pub use standout_dispatch::{
    CommandContext, Extensions, FnHandler, Handler, HandlerResult, Output, RunResult,
};

// Tests for these types are in the standout-dispatch crate.
