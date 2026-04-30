# Proposal: Error Handling Overhaul

## Status

Draft. Targets standout 8.0 (with a clean-up in 9.0). Builds on the 7.6.2 hotfix in #143 (closes #141), which added `RunResult::Error` and made `run()` exit non-zero on failure. This proposal addresses the underlying design holes that made the bug possible.

The guiding principle, set explicitly at the start of this work: **the old non-error-bearing API is deprecated**. Consumers that want to ignore errors must do so by explicit opt-in (`unwrap`, `silence`, drop). The framework will not offer a one-liner that silently throws errors away.

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

#[non_exhaustive]
pub struct DispatchError {
    pub source: anyhow::Error,
    pub kind: ErrorKind,
    pub exit_code: u8,
}

#[non_exhaustive]
pub enum ErrorKind {
    Handler,
    Hook(HookPhase),     // pre/post-dispatch, post-output
    Output,              // file write, pipe, etc.
}
```

Why a struct, not just `anyhow::Error`? Because the framework already knows *where* the error came from (handler vs. pre-dispatch hook vs. file write), and that information should travel with the error. It's free to compute and lossless to carry. Both `DispatchError` and `ErrorKind` are `#[non_exhaustive]` so future kinds (`Render`, `Validation`, etc.) are additive — consumers must always carry a wildcard arm when matching.

The `String` form is recoverable via `format!("{}", err.source)` for code that just wants a message. The deprecated `error()` accessor's return type **changes** in 8.0:

```rust
// 7.x:
pub fn error(&self) -> Option<&str>

// 8.0 (deprecated, breaking signature):
#[deprecated(since = "8.0.0", note = "use `error_source()` for typed access \
                                       or `error_message()` for an owned String")]
pub fn error(&self) -> Option<std::borrow::Cow<'_, str>>
```

This is a hard break for any 7.x test code that pattern-matches the return type. The migration is mechanical: `result.error().unwrap().contains("X")` becomes `result.error().as_deref().unwrap().contains("X")`. We accept this break (the alternative — caching a precomputed `String` inside `DispatchError` — duplicates state for a method we want users to migrate away from). The `#[deprecated]` attribute steers users toward `error_source()` / `error_message()` regardless.

#### `run() -> bool` is deprecated; new `run_to_exit() -> ExitCode` takes its place in 8.0

The 7.x `run() -> bool` is the canonical *non-error-bearing* surface in the framework: it can't represent failure to the OS, so 7.x patches around that with an internal `process::exit(1)` call. 8.0 deprecates it.

The deprecation policy:

```rust
// 8.0 — both methods exist, old one is a soft break.
#[deprecated(
    since = "8.0.0",
    note = "use `run_to_exit()` and propagate from main(); \
            see docs/proposals/error-handling-overhaul.md"
)]
pub fn run<I, T>(&self, cmd: Command, args: I) -> bool { ... }

pub fn run_to_exit<I, T>(&self, cmd: Command, args: I) -> std::process::ExitCode { ... }
```

Callers wire the new method as:

```rust
fn main() -> std::process::ExitCode {
    let app = build_app();
    app.run_to_exit(cmd, std::env::args())
}
```

This pulls `process::exit` out of the library-level execution path entirely: the new method *returns* an `ExitCode` rather than terminating the process, and `main()` is responsible for propagating it. Drop, destructors, and finalizers all run normally.

**9.0 cleanup**: `run() -> bool` is removed and `run_to_exit()` is renamed to `run()`. Deprecation warnings during 8.x give consumers a window to migrate without taking a build-break.

For consumers who want the old `bool` semantics ("did anything match"), `run_or_unmatched(cmd, args) -> Result<ExitCode, ArgMatches>` returns the unmatched matches in the `Err` arm. This is a third method, not a replacement for `run_to_exit`, since the two needs are distinct.

**`NoMatch` exit code.** When `run_to_exit` encounters `RunResult::NoMatch` (no handler matched, and no fallback dispatch), it produces exit code **`2`** by convention — matching clap's behavior for argument errors. Apps using standout in partial-adoption mode (where `NoMatch` means "fall through to legacy dispatch", not user error) override this:

```rust
App::builder()
    .no_match_exit_code(0)   // NoMatch is not a user error in this app
```

A value of `0` means `run_to_exit` returns `ExitCode::SUCCESS` on `NoMatch`, letting the caller's fallback path handle the matches without polluting the exit code.

#### Ignoring errors must be explicit

The point of deprecating the non-error-bearing API is that *swallowing failure should require an act of will*. The 7.x default — quiet `bool` returns, errors disappearing into stdout — is exactly the pattern that produced #141. In 8.0, consumers have three honest options:

1. **Propagate.** `fn main() -> ExitCode { app.run_to_exit(cmd, args) }` — the OS sees the right code. This is the recommended default.
2. **Panic on error.** `let code = app.run_to_exit(cmd, args); assert!(code == ExitCode::SUCCESS);` — or a convenience `app.run_or_panic(cmd, args)` that panics on `RunResult::Error`. This is "I expect this to never fail; if it does, abort loud."
3. **Silence.** `let _ = app.run_to_exit(cmd, args);` — explicitly drop the `ExitCode`. Discarding the value discards its effect: `ExitCode` only reaches the OS when returned from `main`, so a dropped one is invisible and the process exits with whatever `main` itself returns (typically `0`). If a caller wants a non-zero process exit, they have to set it explicitly (`std::process::exit(...)` or a hand-built `ExitCode`). We may add `.silenced()` as a clarity helper that makes the discard self-documenting.

The framework does *not* offer a one-liner that silently throws errors away. If a user wants "old `run()`-style fire and forget," they call the deprecated method and accept the warning.

#### Same deprecation policy on the `RunResult` accessor

`RunResult::Error(String)` becomes `RunResult::Error(DispatchError)` in 8.0. To smooth that:

```rust
#[deprecated(since = "8.0.0", note = "use `error_source()` for typed access")]
pub fn error(&self) -> Option<&str> { ... }   // formats DispatchError.source on demand

pub fn error_source(&self) -> Option<&anyhow::Error> { ... }
pub fn error_kind(&self) -> Option<&ErrorKind> { ... }
pub fn exit_code(&self) -> u8 { ... }   // 0 for success, computed via Layer 2 mappers for Error
```

Tests and callers that only need the message keep working with a deprecation warning. Tests that want typed access opt in to `error_source()`.

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

For apps with a stable error vocabulary, wrapping every `Err` in an `ExitError` is boilerplate. Layer 2 is a `TypeId`-keyed registry on the builder — **one mapper per concrete type, no order**:

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
    .default_exit_code(1)   // for any error not matching above (1 is default; pass to override)
```

Exit-code computation walks: `ExitError.code` (Layer 1) first → `downcast_ref::<T>()` against each registered mapper → `default_exit_code` (defaults to 1) → falls through. Re-registering `.exit_code_for::<T>` for the same `T` replaces the previous mapper; there is no "first wins" / "last wins" question because there is no chain.

This sidesteps the "Python exception-hierarchy" model that doesn't translate to Rust's flat error types. Apps that genuinely need conditional dispatch over their own error types should wrap them in an enum and pattern-match inside the closure — that's where the conditional logic belongs.

This is the "shell error subclass with a code" pattern from the Python parallel, expressed Rust-idiomatically as `TypeId`-keyed downcast dispatch.

### Layer 3 — Themed error rendering

Errors are user-facing output. Today they're `eprintln!("{}", msg)` — plain text, no theme. The behavior splits by output mode:

#### Term/Text mode: themed template

A BBCode-tagged template is applied to the error message and written to stderr:

```rust
App::builder()
    .error_template("[error]Error:[/error] {{ message }}\n")
```

Default template (when none is configured) is the current plain `Error: {message}\n`.

For typed errors, optional per-type templates:

```rust
App::builder()
    .error_template_for::<std::io::Error>(io_error_template)
```

#### JSON / YAML / structured modes: structured error object

When the user requested machine-readable output (`--output=json` or `--output=yaml`), errors emit a structured object in the *same* format the user asked for, written to **stderr**:

```bash
$ tdoo --output=json find missing
{"error": {"message": "not found: missing", "kind": "Handler", "exit_code": 2}}     # → stderr
                                                                                    # stdout empty, $? = 2
```

Why stderr (not stdout)? Convention. `gh`, `kubectl`, `aws` all keep errors on stderr regardless of output mode so that `cmd --output=json | jq` doesn't accidentally process error objects as data. Scripts that want to capture the structured error redirect with `2>err.json`. (Open: confirm this is the right call. The case for stdout is "the user asked for JSON, give them parseable failure on the same stream as success." The case for stderr is "stdout is for downstream consumers; failure must not pollute it." Going with stderr.)

The shape of the error object is stable: `{"error": {"message", "kind", "exit_code"}}` with `message` formatted from `DispatchError.source`, `kind` serialized from `ErrorKind`, `exit_code` from Layer 1/2 dispatch. Templates do NOT apply to JSON/YAML output — the schema is the contract there.

This split (template on Term/Text; structured on JSON/YAML) is purely additive — apps that don't configure anything get plain `Error: {message}` on stderr in Term/Text and the JSON object on stderr in JSON mode.

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

#### `ExitCodeExt::silenced()`

`std::process::ExitCode` is an std type, so we cannot add inherent methods to it. Instead, an extension trait makes silencing self-documenting at the call site:

```rust
pub trait ExitCodeExt {
    /// Drops the exit code without propagating it. Equivalent to `let _ = code;`
    /// but greppable: a search for `.silenced()` finds every place errors are
    /// thrown away in the codebase.
    fn silenced(self);
}

impl ExitCodeExt for std::process::ExitCode {
    fn silenced(self) { /* dropped */ }
}
```

Usage:

```rust
use standout::cli::ExitCodeExt;

fn main() {
    app.run_to_exit(cmd, std::env::args()).silenced();
}
```

Combined with `#[must_use]` on `ExitCode` (already present in std), `let _ = ...` would suppress the lint anyway — `.silenced()` adds nothing the compiler enforces but everything a code reviewer needs.

## Migration

### Soft breaks (`#[deprecated]` in 8.0, removed in 9.0)

1. `run() -> bool` → use `run_to_exit() -> ExitCode`. Old method still works in 8.x with a deprecation warning; gone in 9.0.

### Hard breaks (8.0)

1. `RunResult::Error(String)` → `RunResult::Error(DispatchError)`. Variants can't be soft-deprecated; this is a one-shot change at the major boundary.
2. `RunResult::error()` signature changes from `Option<&str>` to `Option<Cow<'_, str>>`. Marked `#[deprecated]` to steer users toward `error_source()` / `error_message()`, but the signature change is a hard break (no byte-compatibility shim). 7.x test code that did `result.error().unwrap().contains("X")` becomes `result.error().as_deref().unwrap().contains("X")` — one-line per call site.
3. Internal: `dispatch.rs:98`'s `format!("Error: {}", e)` collapse is removed; the `anyhow::Error` rides through to `RunResult::Error`.
4. `RunResult` and `DispatchError` and `ErrorKind` are all `#[non_exhaustive]`. Existing `RunResult` matchers already needed a wildcard after 7.6.2; new `DispatchError`/`ErrorKind` matchers will too.

### Migration paths

- **Apps using `run()`**: rename to `run_to_exit()` and have `main()` return `ExitCode`. The deprecation warning lands you in the right place.
- **Apps that *want* the old "fire and forget" behavior**: the deprecated `run()` still works for one minor cycle. Long-term, the framework will not offer a one-liner that silences errors — silencing must be explicit at the call site (`let _ = app.run_to_exit(...);`).
- **Apps matching `RunResult::Error(s)`**: the variant payload changes from `String` to `DispatchError`, and the deprecated `result.error()` accessor's return type changes from `Option<&str>` to `Option<Cow<'_, str>>`. Code like `result.error().unwrap().contains("X")` becomes `result.error().as_deref().unwrap().contains("X")` — one-line edit per call site. New code skips the deprecated shim and uses `result.error_source()` / `result.error_kind()` / `result.exit_code()`.
- **Apps that want exit codes**: opt in to Layer 1 (`ExitError::new(code, ...)`) or Layer 2 (`.exit_code_for::<T>(...)`).
- **Tests using `assert_error_contains`**: keep working unchanged. New helpers (`assert_exit_code`, `assert_error_kind`) are additive.

A migration guide page in the book walks through each diff with examples. The aim is that a typical 7.x app updates with three find/replace edits and a `#[deprecated]` warning to chase.

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

## Resolved decisions

These were the open questions on the original draft. Resolved through design review on PR #144; rationale captured here so the implementation doesn't relitigate.

1. **Mapper order in Layer 2: one-mapper-per-type, no order.** Registry is keyed by `TypeId`; re-registering the same type replaces. Catchall via `.default_exit_code(u8)`. The "first-match vs. most-specific" framing was borrowed from Python's exception hierarchy and doesn't translate to Rust's flat error types — there is no type lattice to walk.
2. **`DispatchError` and `ErrorKind` are both `#[non_exhaustive]`.** Future kinds (`Render`, `Validation`, etc.) are additive. Cost: mandatory wildcard arm forever in matchers. Worth it to keep the major-version churn to a minimum.
3. **`NoMatch` default exit code is `2`.** Matches clap's convention for argument errors. Apps in partial-adoption mode override via `.no_match_exit_code(u8)` on the builder; setting `0` means `NoMatch` is not treated as a user error.
4. **Hybrid rendering for errors.** Term/Text modes apply the configured error template (or default `Error: {message}\n`) to stderr. JSON/YAML modes emit a structured `{"error": {message, kind, exit_code}}` object in the requested format, *also* on stderr — keeps stdout empty on failure so pipe consumers (`cmd --output=json | jq`) don't process error objects as data. The structured-error schema is stable; templates do not apply in machine-readable modes.
5. **`silenced()` is a real method.** Implemented as an extension trait `ExitCodeExt` on `std::process::ExitCode`. Self-documenting and greppable: `.silenced()` calls flag every place errors are intentionally thrown away. `let _ = ...` would be functionally equivalent but invisible to code review.
6. **Shim signature: `Option<Cow<'_, str>>` (hard break).** The 7.x `Option<&str>` shape is not preserved. Migration is a one-line edit per call site: `result.error().unwrap()` → `result.error().as_deref().unwrap()`. Rejected the alternative of caching a precomputed `String` inside `DispatchError` — that duplicates state for a method users are being steered away from. The `#[deprecated]` attribute steers toward `error_source()` / `error_message()` regardless.

## Phasing

- **Now (this PR)**: ratify the design.
- **Next**: implement Layer 0 on `feat/error-handling-8.0`. The deprecation pieces (`#[deprecated]` attrs, `run_to_exit`, `error_source`) ship in 8.0 alongside the hard breaks (`DispatchError` payload, `dispatch.rs:98` rewrite). Keep the branch open while Layers 1–4 land incrementally.
- **Then**: 8.0 release once Layer 0 + Layer 1 + book chapter are in. Layers 2/3/4 can ship in 8.x without further breakage.
- **9.0**: remove the deprecated `run() -> bool` and `error() -> Option<&str>`; rename `run_to_exit` → `run`; drop the message-format shim.

The fact that Layer 1 and Layer 4 are non-breaking means we don't have to ship everything at once. The `#[deprecated]` cycle gives consumers time to migrate without a hard build-break in 8.0 itself.
