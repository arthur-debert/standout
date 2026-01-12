# Declarative Command Dispatch

**Status:** Proposal
**Author:** Claude
**Date:** 2026-01-12

## Motivation

CLI applications built with clap typically follow a pattern:

1. Define argument structures using clap's derive macros
2. Parse input with `clap::Parser`
3. Write a manual dispatch tree matching commands to handler functions

Step 3 is repetitive boilerplate. Worse, handler functions end up with CLI-aware signatures that manually extract arguments from `ArgMatches`:

```rust
fn migrate(matches: &ArgMatches, ctx: &CommandContext) -> CommandResult<Value> {
    let db = matches.get_one::<String>("database").unwrap();
    let host = matches.get_one::<String>("host").unwrap_or(&"localhost".into());
    let dry_run = matches.get_flag("dry-run");

    // Finally, actual business logic...
}
```

This is error-prone (typos in string keys, missing unwraps), verbose, and couples business logic to CLI infrastructure.

**The ideal**: handlers with natural function signatures that receive typed, validated arguments:

```rust
fn migrate(args: MigrateArgs) -> Result<MigrateOutput, Error> {
    // Pure business logic - no CLI awareness
}
```

## Goals

1. **Natural handler signatures** - Handlers receive typed structs, not raw `ArgMatches`
2. **Leverage clap's derive macros** - Don't reinvent argument parsing; build on `#[derive(Args)]`
3. **Progressive disclosure** - Simple cases are simple; power users get escape hatches
4. **Type-safe dispatch** - Compile-time verification of command paths and handler types
5. **Composable** - Works with existing hook system (pre_dispatch, post_dispatch, post_output)
6. **Convention over configuration** - Templates resolve from command paths by default

## Assumptions

1. **Users have a working clap CLI** - We integrate with clap, not replace it
2. **Clap derive is the norm** - Most users define `#[derive(Args)]` structs for their commands
3. **Handlers are pure functions** - Business logic shouldn't depend on CLI infrastructure
4. **Serializable output** - All handler output must implement `Serialize` for template rendering
5. **Single dispatch target** - Each command maps to exactly one handler function

## Non-Goals

- Replacing clap's argument parsing
- Async handler support (can be added later)
- Runtime command discovery (compile-time only)

---

## Design Overview

### The Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Layer 3: Attribute Macro (Future)                               │
│   #[outstanding::handler]                                        │
│   Auto-generates Args struct from function signature             │
├─────────────────────────────────────────────────────────────────┤
│ Layer 2: Derive Macro Integration (Future)                      │
│   #[derive(Dispatch)] on clap enums                              │
│   Generates routing from subcommand tree to handlers            │
├─────────────────────────────────────────────────────────────────┤
│ Layer 1: Args-Aware Handler Trait (Foundation) ← START HERE     │
│   IntoHandler trait + typed builder methods                      │
│   Enables typed argument extraction                             │
├─────────────────────────────────────────────────────────────────┤
│ Layer 0: Current System (Unchanged)                             │
│   Handler trait + dispatch! macro + hooks                        │
│   For power users who need raw ArgMatches access                │
└─────────────────────────────────────────────────────────────────┘
```

Each layer builds on the one below. Macros (Layer 2, 3) generate code that uses the builder API (Layer 1).

---

## Layer 1: Handler Shapes

We support multiple handler "shapes" (function signatures), all converging to the same internal machinery:

### Supported Signatures

```rust
// Shape A: Full control (current behavior, escape hatch)
fn(&ArgMatches, &CommandContext) -> CommandResult<T>

// Shape B: Typed args with context
fn(Args, &CommandContext) -> CommandResult<T>

// Shape C: Typed args, no context (most common)
fn(Args) -> CommandResult<T>

// Shape D: No args, no context (e.g., version command)
fn() -> CommandResult<T>
```

Additionally, for simpler cases, we support `Result<T, E>` return types that auto-convert:

```rust
// Shape C with Result (auto-converts to CommandResult)
fn(Args) -> Result<T, E>  where E: Into<anyhow::Error>

// Shape D with Result
fn() -> Result<T, E>
```

### The `IntoHandler` Trait

A single trait converts any handler shape into our internal dispatch machinery:

```rust
/// Marker types for handler shapes (enables multiple blanket impls)
pub mod handler_shape {
    pub struct Raw;           // (&ArgMatches, &CommandContext)
    pub struct ArgsCtx<A>(std::marker::PhantomData<A>);  // (A, &CommandContext)
    pub struct ArgsOnly<A>(std::marker::PhantomData<A>); // (A,)
    pub struct NoArgs;        // ()
}

/// Trait for converting any handler shape into a dispatch function.
pub trait IntoHandler<Shape>: Send + Sync + 'static {
    /// Convert this handler into a type-erased dispatch function.
    fn into_dispatch_fn(self, template_resolver: TemplateResolver) -> DispatchFn;
}
```

### Blanket Implementations

```rust
// Shape A: fn(&ArgMatches, &CommandContext) -> CommandResult<T>
impl<F, T> IntoHandler<handler_shape::Raw> for F
where
    F: Fn(&ArgMatches, &CommandContext) -> CommandResult<T> + Send + Sync + 'static,
    T: Serialize + 'static,
{
    fn into_dispatch_fn(self, resolver: TemplateResolver) -> DispatchFn {
        Arc::new(move |matches, ctx, hooks| {
            let result = (self)(matches, ctx);
            resolver.render(result, hooks)
        })
    }
}

// Shape B: fn(A, &CommandContext) -> CommandResult<T>
impl<F, A, T> IntoHandler<handler_shape::ArgsCtx<A>> for F
where
    F: Fn(A, &CommandContext) -> CommandResult<T> + Send + Sync + 'static,
    A: clap::FromArgMatches + Clone + Send + Sync + 'static,
    T: Serialize + 'static,
{
    fn into_dispatch_fn(self, resolver: TemplateResolver) -> DispatchFn {
        Arc::new(move |matches, ctx, hooks| {
            let args = A::from_arg_matches(matches)
                .map_err(|e| ExtractError::args(ctx.command_path.join("."), e))?;
            let result = (self)(args, ctx);
            resolver.render(result, hooks)
        })
    }
}

// Shape C: fn(A) -> CommandResult<T>  (context-free)
impl<F, A, T> IntoHandler<handler_shape::ArgsOnly<A>> for F
where
    F: Fn(A) -> CommandResult<T> + Send + Sync + 'static,
    A: clap::FromArgMatches + Clone + Send + Sync + 'static,
    T: Serialize + 'static,
{
    fn into_dispatch_fn(self, resolver: TemplateResolver) -> DispatchFn {
        Arc::new(move |matches, ctx, hooks| {
            let args = A::from_arg_matches(matches)
                .map_err(|e| ExtractError::args(ctx.command_path.join("."), e))?;
            let result = (self)(args);
            resolver.render(result, hooks)
        })
    }
}

// Shape D: fn() -> CommandResult<T>  (no args)
impl<F, T> IntoHandler<handler_shape::NoArgs> for F
where
    F: Fn() -> CommandResult<T> + Send + Sync + 'static,
    T: Serialize + 'static,
{
    fn into_dispatch_fn(self, resolver: TemplateResolver) -> DispatchFn {
        Arc::new(move |_matches, _ctx, hooks| {
            let result = (self)();
            resolver.render(result, hooks)
        })
    }
}
```

---

## Return Type Flexibility

### The `IntoCommandResult` Trait

Support both explicit `CommandResult<T>` and simple `Result<T, E>`:

```rust
/// Trait for types that can be converted to CommandResult.
pub trait IntoCommandResult<T: Serialize> {
    fn into_command_result(self) -> CommandResult<T>;
}

// CommandResult passes through unchanged
impl<T: Serialize> IntoCommandResult<T> for CommandResult<T> {
    fn into_command_result(self) -> CommandResult<T> {
        self
    }
}

// Result<T, E> converts automatically
impl<T: Serialize, E: Into<anyhow::Error>> IntoCommandResult<T> for Result<T, E> {
    fn into_command_result(self) -> CommandResult<T> {
        match self {
            Ok(data) => CommandResult::Ok(data),
            Err(e) => CommandResult::Err(e.into()),
        }
    }
}

// Raw T for infallible handlers (optional - may be too magical)
impl<T: Serialize> IntoCommandResult<T> for T {
    fn into_command_result(self) -> CommandResult<T> {
        CommandResult::Ok(self)
    }
}
```

This allows handlers to use whichever return type is most natural:

```rust
// Explicit control (can return Silent, Archive, etc.)
fn export(args: ExportArgs) -> CommandResult<ExportOutput> {
    if args.format == "pdf" {
        CommandResult::Archive(pdf_bytes, "export.pdf".into())
    } else {
        CommandResult::Ok(ExportOutput { ... })
    }
}

// Simple Result (most common)
fn migrate(args: MigrateArgs) -> Result<MigrateOutput, MigrateError> {
    Ok(MigrateOutput { success: true })
}

// Infallible (optional)
fn version() -> VersionOutput {
    VersionOutput { version: "1.0.0".into() }
}
```

---

## Error Handling

### The `ExtractError` Type

Extraction failures include context about what failed and where:

```rust
use thiserror::Error;

/// Error during argument extraction.
#[derive(Debug, Error)]
pub enum ExtractError {
    /// Clap failed to parse/extract arguments
    #[error("Failed to extract arguments for '{command}': {message}")]
    Extraction {
        command: String,
        message: String,
        #[source]
        source: Option<clap::Error>,
    },
}

impl ExtractError {
    /// Create from a clap error with command context.
    pub fn args(command: impl Into<String>, error: clap::Error) -> Self {
        Self::Extraction {
            command: command.into(),
            message: error.to_string(),
            source: Some(error),
        }
    }
}
```

---

## Builder API

### Explicit Methods (Recommended for Phase 1)

```rust
impl OutstandingBuilder {
    /// Register handler with raw ArgMatches access (escape hatch)
    pub fn command<F, T>(self, path: &str, handler: F, template: &str) -> Self
    where
        F: Fn(&ArgMatches, &CommandContext) -> CommandResult<T> + Send + Sync + 'static,
        T: Serialize + 'static;

    /// Register handler with typed Args and context
    pub fn handler_with_context<A, F, R, T>(self, path: &str, handler: F) -> Self
    where
        A: FromArgMatches + Clone + Send + Sync + 'static,
        F: Fn(A, &CommandContext) -> R + Send + Sync + 'static,
        R: IntoCommandResult<T>,
        T: Serialize + 'static;

    /// Register context-free handler with typed Args (MOST COMMON)
    pub fn handler<A, F, R, T>(self, path: &str, handler: F) -> Self
    where
        A: FromArgMatches + Clone + Send + Sync + 'static,
        F: Fn(A) -> R + Send + Sync + 'static,
        R: IntoCommandResult<T>,
        T: Serialize + 'static;

    /// Register handler with no arguments
    pub fn handler_no_args<F, R, T>(self, path: &str, handler: F) -> Self
    where
        F: Fn() -> R + Send + Sync + 'static,
        R: IntoCommandResult<T>,
        T: Serialize + 'static;
}
```

---

## Macro Syntax Enhancement

The `dispatch!` macro extends to support typed handlers:

```rust
dispatch! {
    db: {
        // Explicit type annotation
        migrate => migrate::<MigrateArgs>,

        // Or with config block
        backup => {
            handler: backup,
            args: BackupArgs,       // Explicit Args type
            template: "backup.j2",
            pre_dispatch: validate_auth,
        },
    },

    // Simple handler (no args)
    version => version,
}
```

The macro expands to builder method calls:

```rust
|builder: GroupBuilder| {
    builder
        .handler::<MigrateArgs, _, _, _>("migrate", migrate)
        .handler_with_config::<BackupArgs, _, _, _>("backup", backup, |cfg| {
            cfg.template("backup.j2").pre_dispatch(validate_auth)
        })
        .handler_no_args("version", version)
}
```

---

## Complete Example

### Before (Current System)

```rust
use clap::{Command, Arg, ArgMatches};
use outstanding_clap::{Outstanding, CommandResult, CommandContext, dispatch};
use serde::Serialize;
use serde_json::json;

fn migrate(matches: &ArgMatches, _ctx: &CommandContext) -> CommandResult<serde_json::Value> {
    // Manual extraction - verbose and error-prone
    let db = matches.get_one::<String>("database").unwrap();
    let host = matches.get_one::<String>("host").unwrap_or(&"localhost".to_string());
    let dry_run = matches.get_flag("dry-run");

    if dry_run {
        return CommandResult::Ok(json!({ "success": true, "tables": 0 }));
    }

    CommandResult::Ok(json!({ "success": true, "tables": 42 }))
}

fn main() {
    let cmd = Command::new("myapp")
        .subcommand(Command::new("db")
            .subcommand(Command::new("migrate")
                .arg(Arg::new("database").required(true))
                .arg(Arg::new("host").long("host").default_value("localhost"))
                .arg(Arg::new("dry-run").long("dry-run").action(clap::ArgAction::SetTrue))));

    Outstanding::builder()
        .template_dir("templates")
        .commands(dispatch! {
            db: {
                migrate => migrate,
            },
        })
        .run_and_print(cmd, std::env::args());
}
```

### After (Layer 1 - Typed Args)

```rust
use clap::{Args, Parser, Subcommand};
use outstanding_clap::{Outstanding, dispatch};
use serde::Serialize;

// Clap handles extraction - type-safe and self-documenting
#[derive(Args, Clone)]
struct MigrateArgs {
    /// Database name
    database: String,

    /// Host address
    #[arg(long, default_value = "localhost")]
    host: String,

    /// Perform dry run without changes
    #[arg(long)]
    dry_run: bool,
}

#[derive(Serialize)]
struct MigrateOutput {
    success: bool,
    tables: usize,
}

// Clean handler - pure business logic!
fn migrate(args: MigrateArgs) -> Result<MigrateOutput, anyhow::Error> {
    if args.dry_run {
        return Ok(MigrateOutput { success: true, tables: 0 });
    }

    // Use args.database, args.host naturally
    Ok(MigrateOutput { success: true, tables: 42 })
}

fn main() {
    Outstanding::builder()
        .template_dir("templates")
        .handler::<MigrateArgs, _, _, _>("db.migrate", migrate)
        .run_and_print(cmd, std::env::args());
}
```

### With Dispatch Macro

```rust
Outstanding::builder()
    .template_dir("templates")
    .commands(dispatch! {
        db: {
            migrate => migrate::<MigrateArgs>,
        },
    })
    .run_and_print(cmd, std::env::args());
```

---

## Layer 2: Derive Macro Integration (Future)

For users who use clap's derive pattern with enum-based subcommands:

```rust
use clap::{Parser, Subcommand, Args};
use outstanding_clap::Dispatch;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Dispatch)]  // Add Dispatch derive
enum Commands {
    /// Database operations
    Db(DbCommands),
}

#[derive(Subcommand, Dispatch)]
enum DbCommands {
    /// Run migrations
    #[dispatch(handler = db::migrate)]
    Migrate(MigrateArgs),

    /// Backup database
    #[dispatch(handler = db::backup, template = "backup.j2")]
    Backup(BackupArgs),
}
```

The derive macro generates a `dispatch()` method that routes to handlers:

```rust
impl Commands {
    pub fn dispatch(
        &self,
        renderer: &Outstanding,
    ) -> Result<RunResult, DispatchError> {
        match self {
            Commands::Db(sub) => sub.dispatch(renderer),
        }
    }
}
```

---

## Layer 3: Attribute Macro on Handlers (Future)

Maximum convenience - generates Args struct from function signature:

```rust
#[outstanding::handler(command = "db.migrate")]
fn migrate(
    database: String,              // Positional arg
    #[opt] host: String,           // --host (Option inferred)
    #[opt(short = 'd')] dry_run: bool,  // -d / --dry-run
) -> Result<MigrateOutput, Error> {
    // Pure business logic
}
```

Generates:

```rust
#[derive(clap::Args, Clone)]
pub struct MigrateArgs {
    database: String,
    #[arg(long)]
    host: Option<String>,
    #[arg(short = 'd', long)]
    dry_run: Option<bool>,
}

fn migrate(args: MigrateArgs) -> Result<MigrateOutput, Error> {
    let database = args.database;
    let host = args.host.unwrap_or_else(|| "localhost".into());
    let dry_run = args.dry_run.unwrap_or(false);
    // User's original function body
}
```

---

## Implementation Plan

### Phase 1: Core Traits (Foundation)

1. Add `ExtractError` type to `extract.rs`
2. Add `IntoCommandResult` trait to `handler.rs`
3. Add `IntoHandler` trait with shape markers to `extract.rs`
4. Add builder methods: `.handler()`, `.handler_with_context()`, `.handler_no_args()`
5. Comprehensive tests for all handler shapes
6. Update documentation

### Phase 2: Macro Integration

1. Extend `dispatch!` macro to support `handler::<Args>` syntax
2. Add `args:` option to config blocks in macro
3. Update `GroupBuilder` to use new handler methods internally

### Phase 3: Derive Macro (Separate Crate)

1. Create `outstanding-macros` proc-macro crate
2. Implement `#[derive(Dispatch)]` for clap enums
3. Implement `#[handler]` attribute macro (optional)

---

## File Structure

```
crates/outstanding-clap/src/
├── lib.rs              # Re-exports
├── handler.rs          # CommandResult, Handler, IntoCommandResult
├── extract.rs          # NEW: ExtractError, IntoHandler, handler_shape
├── dispatch.rs         # Internal dispatch types
├── group.rs            # GroupBuilder with typed handler support
├── macros.rs           # dispatch! macro with args syntax
├── hooks.rs            # Hook system
└── outstanding.rs      # Builder with new handler methods

crates/outstanding-macros/    # Future: proc-macro crate
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── dispatch.rs     # #[derive(Dispatch)]
    └── handler.rs      # #[handler] attribute
```

---

## Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Handler shapes | 4 shapes (Raw, ArgsCtx, ArgsOnly, NoArgs) | Cover all use cases from escape hatch to simplest |
| Context parameter | Optional | Most handlers don't need it; keep signatures clean |
| Return type | `CommandResult<T>` or `Result<T, E>` | Flexibility via `IntoCommandResult` trait |
| Error handling | `ExtractError` with command context | Clear, actionable error messages |
| Implementation order | Layer 1 (traits/builder) first | Macros build on solid foundation |
| Builder API | Explicit methods | Clear error messages; inference can be added later |

---

## Open Questions

1. **Should `IntoCommandResult` accept raw `T`?** - Enables `fn() -> Output` but may be too magical
2. **Naming**: `handler()` vs `command_handler()` vs `register()`?
3. **Clone requirement on Args**: Required for type erasure; acceptable tradeoff?

---

## References

- [Clap Derive Tutorial](https://docs.rs/clap/latest/clap/_derive/)
- [Axum Handler Trait](https://docs.rs/axum/latest/axum/handler/trait.Handler.html)
- [Current outstanding-clap implementation](../crates/outstanding-clap/src/)
