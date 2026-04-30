# Proposal: Error Handling Overhaul

## Status

Draft. Targets standout 8.0. Builds on the 7.7 hotfix in #143 (closes #141), which added `RunResult::Error` and made `run()` exit non-zero on failure. This proposal addresses the underlying design holes that made the bug possible.

## Motivation

The 7.7 hotfix is necessary but not sufficient. The bug it fixed — handler errors surfacing as `RunResult::Handled` and the binary exiting 0 — was the visible symptom of a deeper pattern: standout's error handling was designed *against the happy path*, with failure plumbing bolted on. Concretely:

- **Errors are flattened to `String` at the API boundary.** `dispatch.rs:98` turns an `anyhow::Error` into `format!("Error: {}", e)`, throwing away the context chain, the source, and any typed downcast info. From that point forward, an error is just a display string.
- **`run()` returns `bool`.** It cannot represent failure to the OS. The 7.7 hotfix patches this by calling `std::process::exit(1)` from inside `run()` — pragmatic, but a library calling `process::exit` is a footgun.
- **`RunResult::Handled(String)` was overloaded across seven semantic states** in `execution.rs` (rendered text, handler error, three flavors of hook error, two flavors of write error, empty-string success). Tests baked in the swallowing as the contract.
- **No exit-code policy.** Even after the hotfix, every error exits 1. There is no way for a handler to say "this is a 'not found' — exit 2" without bypassing the framework.
- **Hooks have no error-policy lever.** Every hook error becomes `"Hook error: {e}"` stuffed into one variant. There's no concept of "log and continue" vs. "fatal."

For a CLI framework — whose entire job is shepherding `main()` to a correct exit code, message, and stream — these are core features, not niceties. The goal of this proposal is to move standout from *"errors don't break things"* to *"errors are a value-add."*

## Goals

1. **Errors are first-class data.** They flow through the framework as `anyhow::Error` (or a wrapper carrying it), not as pre-formatted strings.
2. **Default behavior is correct shell semantics.** Stderr for messages, non-zero exit codes, no `process::exit` from inside library code.
3. **Opt-in structure when it pays off.** Handlers that don't care get the default; handlers that need exit-code control get a one-line API.
4. **No new mandatory layer.** Today's handler signature still works. The simple case stays simple.
5. **Testing is a first-class concern.** If the framework can produce an outcome, the test harness can assert on it.

## Non-goals

- Replacing `anyhow` or introducing a parallel error type system inside handlers.
- Forcing every handler to use a framework-supplied error type.
- Designing an error-presentation framework beyond what the existing render path can absorb.
- Adding macros that hide what's happening.

## What 7.7 already ships (recap)

- `RunResult::Error(String)` variant + `is_error()` / `error()` accessors.
- `RunResult` is `#[non_exhaustive]`.
- `run()` writes errors to stderr and `process::exit(1)`s.
- `standout-test::TestRun` gains `is_error()` / `error()` / `assert_error()` / `assert_error_contains()`.

The design choices in 7.7 are deliberately conservative — the variant carries `String`, `run()` keeps `bool`, exit codes are not policy-driven. 8.0 is where we earn the right to break things.

## Design

The design is layered: each layer is independently useful and can be adopted (or skipped) without affecting the others.

### Layer 0 — Default behavior (always on, replaces 7.7 patch)

#### `RunResult::Error` carries `anyhow::Error`, not `String`

```rust
#[non_exhaustive]
pub enum RunResult {
    Handled(String),
    Binary(Vec<u8>, String),
    Silent,
    Error(DispatchError),
    NoMatch(ArgMatches),
}

pub struct DispatchError {
    pub source: anyhow::Error,
    pub kind: ErrorKind,
    pub exit_code: u8,
}

pub enum ErrorKind {
    Handler,
    Hook(HookPhase),     // pre/post-dispatch, post-output
    Output,              // file write, pipe, etc.
}
```

Why a struct, not just `anyhow::Error`? Because the framework already knows *where* the error came from (handler vs. pre-dispatch hook vs. file write), and that information should travel with the error. It's free to compute and lossless to carry.

The `String` form is recoverable via `format!("{}", err.source)` for code that just wants a message. Existing call sites that did `result.error()` keep working with a thin shim that returns `Option<&str>` (formats source on demand), or upgrade to `result.error_source()` for the typed access.

#### `run()` returns `ExitCode`

```rust
pub fn run<I, T>(&self, cmd: Command, args: I) -> std::process::ExitCode
where I: IntoIterator<Item = T>, T: Into<OsString> + Clone
```

Callers wire it as:

```rust
fn main() -> std::process::ExitCode {
    let app = build_app();
    app.run(cmd, std::env::args())
}
```

This removes the `process::exit` call from inside `run()`. Library code no longer terminates the process; the application's `main` does. Drop, destructors, and finalizers all run normally.

For consumers who want the old `bool` semantics ("did anything match"), `run_or_unmatched(cmd, args) -> Result<ExitCode, ArgMatches>` returns the unmatched matches in the `Err` arm.

#### Errors go to stderr; success to stdout

This is already true after 7.7 for `run()`. 8.0 also routes binary-write failure messages to stderr, and any framework-emitted messages (warnings, hook errors) consistently use stderr. Stdout is reserved for command output that downstream tools might pipe.

### Layer 1 — Opt-in: exit-code-aware errors

For handlers that need specific exit codes (e.g., `2` for "not found", `13` for "permission denied" — common conventions), introduce a thin newtype:

```rust
pub struct ExitError {
    pub code: u8,
    pub source: anyhow::Error,
}

impl ExitError {
    pub fn new(code: u8, source: impl Into<anyhow::Error>) -> Self { ... }
}
```

Handlers wrap their error before returning:

```rust
fn find(matches: &ArgMatches, ctx: &CommandContext) -> HandlerResult<Item> {
    let path = matches.get_one::<String>("path").unwrap();
    let item = store.find(path)
        .ok_or_else(|| ExitError::new(2, anyhow!("not found: {}", path)))?;
    Ok(Output::Render(item))
}
```

The framework downcasts the `anyhow::Error` to `ExitError` when computing exit code; absent that, exit code defaults to 1.

Why a newtype and not a trait? Because `anyhow::Error` already supports downcasting, and a single concrete type keeps the API surface boring. Handlers that don't need exit codes pay nothing.

### Layer 2 — Opt-in: type-to-exit-code mappings

For apps with a stable error vocabulary, wrapping every `Err` in an `ExitError` is boilerplate. Layer 2 is a registry on the builder:

```rust
App::builder()
    .exit_code_for::<std::io::Error>(|e| match e.kind() {
        ErrorKind::NotFound => 2,
        ErrorKind::PermissionDenied => 13,
        _ => 1,
    })
    .exit_code_for::<MyAppError>(|e| match e {
        MyAppError::Conflict(_) => 17,
        _ => 1,
    })
```

The framework, when computing exit code, walks: `ExitError.code` first, then registered mappers in registration order, then default 1. Mappers receive a borrowed reference; first non-1 wins (or first match if you prefer — open question).

This is the "shell error subclass with a code" pattern from the Python parallel, expressed Rust-idiomatically as downcast-based dispatch.

### Layer 3 — Themed error rendering

Errors are user-facing output. Today they're `eprintln!("{}", msg)` — plain text, no theme. There's no reason a `[error]Error:[/error] {message}` template can't apply.

```rust
App::builder()
    .error_template("[error]Error:[/error] {{ message }}\n")
```

For typed errors, optional per-type templates:

```rust
App::builder()
    .error_template_for::<std::io::Error>(io_error_template)
```

This is purely additive — apps that don't configure a template get the current `Error: {message}` formatting.

### Layer 4 — Testing

Build on the 7.7 `assert_error_contains`:

```rust
result.assert_exit_code(2);
result.assert_error_kind(ErrorKind::Handler);
result.assert_error_downcast::<std::io::Error>(|e| matches!(e.kind(), ErrorKind::NotFound));
```

Plus a "simulated end-to-end" assertion that captures what the OS would observe:

```rust
let observed = TestRun::observe(app, args);
assert_eq!(observed.exit_code(), 2);
assert_eq!(observed.stderr_contains("not found"));
assert!(observed.stdout().is_empty());
```

This lets us write tests that pin shell behavior, not just framework internals.

## Migration

### Breaking changes (all in 8.0)

1. `RunResult::Error(String)` → `RunResult::Error(DispatchError)`.
2. `run() -> bool` → `run() -> ExitCode`.
3. `run_to_string` keeps its signature (consumers who want full control already use it).
4. Internal: `dispatch.rs:98`'s `format!("Error: {}", e)` collapse is removed; the `anyhow::Error` rides through to `RunResult::Error`.

### Migration paths

- **Apps using `run()`**: change `main()` to return `ExitCode`. One-line change.
- **Apps matching `RunResult::Error(s)`**: change `s` reads from `s.as_str()` to `format!("{}", err.source)` (or use the shim accessor).
- **Apps that want exit codes**: opt in to Layer 1 (`ExitError::new(code, ...)`) or Layer 2 (`.exit_code_for::<T>(...)`).
- **Tests using `assert_error_contains`**: keep working unchanged.

A migration guide page in the book walks through each diff with examples.

## Sample app updates

`todo-example` will demonstrate the layers:

- **Layer 1**: `done <id>` returns `ExitError::new(2, anyhow!("no todo with id {}", id))` when the id doesn't exist. Test: `assert_exit_code(2)`.
- **Layer 2**: register a mapping for `std::io::Error` so file-system failures (corrupt store, permission denied) get sensible codes.
- **Layer 4**: an integration test that runs `done 999` end-to-end and asserts `exit 2`, `stderr contains "no todo"`, `stdout empty`.

## Documentation

- **New book chapter: "Error handling"** — three sections matching the layers (default behavior, exit codes, mappings, testing). Each section has a runnable example pulled from `todo-example`.
- **README callout**: a short paragraph in the value-prop section ("standout binaries respect exit-code conventions out of the box") with a link to the chapter.
- **Migration guide**: 7.x → 8.0 page covering the four breaking changes with before/after snippets.
- **Updates to existing docs**: `cli/handler.rs` rustdoc gets an "Errors and exit codes" subsection. The dispatch flow diagram in `docs/command-flow-diagram.md` gains an explicit error path.

## Out-of-scope follow-ups

These come up naturally during discussion but are *not* part of this proposal:

- **Structured logging integration.** Routing errors to `tracing` instead of (or in addition to) stderr.
- **Recoverable hooks.** A "log and continue" policy for non-fatal hook failures. Layer 1 makes this easier later (a hook can return an error with `code: 0` and a future `HookPolicy::ContinueOnError` could honor it), but it's not in scope here.
- **Error rendering in JSON/structured modes.** Today `--output=json` on a failing command produces no JSON output. Should it produce `{"error": {...}}`? Worth a separate proposal.

## Open questions

1. **Mapper order: first-match vs. most-specific?** Registering a `Box<dyn Error>` mapper last and `io::Error` first — does first-match-wins make sense, or should we walk registrations in reverse? Most-specific is what Python's exception-hierarchy dispatch does; it requires knowing the type tree, which `anyhow` doesn't expose cleanly.
2. **Should `DispatchError::kind` be `#[non_exhaustive]`?** Yes, probably. We may want to add `Render`, `Validation`, etc.
3. **Default exit code for `NoMatch`.** Currently `run()` returns `false`. After this proposal, `run() -> ExitCode` — what code does no-match map to? Convention: `2` (clap's convention for argument errors), but consumers may want override.
4. **Does the error template apply in `--output=json`?** Probably no — JSON consumers want `stderr: <plaintext>, exit nonzero` so they can detect failure cheaply.

## Phasing

- **Now (this PR / proposal)**: ratify the design.
- **Next**: implement Layer 0 (the breaking changes) on a `feat/error-handling-8.0` branch. Keep it open while Layers 1–4 land incrementally.
- **Then**: 8.0 release once Layer 0 + Layer 1 + book chapter are in. Layers 2/3/4 can ship in 8.x without further breakage.

The fact that Layer 1 and Layer 4 are non-breaking means we don't have to ship everything at once.
