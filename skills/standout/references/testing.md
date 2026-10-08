# Testing Standout applications

Choose the smallest boundary that covers the behavior:

| Level | Covers | Tool |
| --- | --- | --- |
| Core | Validation, filtering, state transitions, persistence | Library interface |
| Adapter | CLI-to-core mapping and returned view DTOs | Direct typed handler call |
| Integration | clap through handler, hooks, and rendering | `TestHarness::run` |
| Process | Real binary, env, exit code, TTY-dependent color | `TestHarness::run_process` / `run_pty` (Unix) |

Test filtering, validation, state transitions, and persistence directly through
the CLI-free library interface. With `#[handler]`, call the preserved typed
function to test flag/argument mapping and CLI-owned returned data rather than
constructing `ArgMatches` for the generated wrapper.

Add `standout-test` and `serial_test` as dev-dependencies only; the re-exported `serial` expands to `serial_test` paths, and `standout-test` enables standout's `test-support` feature itself.

Use `TestHarness::run` when command registration, input/environment seams, templates, or output modes matter:

```rust
use standout::Representation;
use standout_test::{serial, TestHarness};

#[test]
#[serial]
fn list_is_machine_readable() {
    let result = TestHarness::new()
        .fixture("todos.json", STORE)
        .env("TDOO__STORE", "todos.json")
        .output_mode(Representation::Json)
        .run(&app, cli::command(), ["tdoo", "list"]);

    result.assert_success();
    let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert_eq!(value["total"], 1);
}
```

`fixture` writes into a per-test tempdir that becomes the cwd. The harness also controls env vars, cwd, terminal width, the four per-stream terminal and color-capability facts, the `ColorPolicy` (`color`, or `rendering(repr, policy)` for both), `piped_stdin`/`interactive_stdin`, `clipboard`, and scripted `prompts`. Without `interactive_stdin()` the run inherits the test runner's stdin, which is not a terminal. `TestResult` exposes `outcome()`, `result()` (the value as data in any mode), `delivery()`, `exit_status()`, `warnings()`, and assertions for text, errors, no-match, binary, artifact, and silent outcomes.

Every `run` test needs `#[serial]`: env and cwd are process-global, restored when the `TestResult` drops. `run_process` tests set env on the child only and need no `#[serial]`.

`run_process(env!("CARGO_BIN_EXE_<bin>"), args)` reuses the harness's env, fixtures, cwd, and `output_mode`, and panics if given an in-process-only setting (width, color, terminal facts, stdin, clipboard, prompts). In-process runs have no TTY detection; test terminal-dependent behavior with `run_pty`. Signals, interactive PTY sessions, and subprocesses launched by application code need `expectrl`/`rexpect` or an application-owned trait with a fake.

Under a structured mode read a failure with `result.diagnostic()` (`kind`,
`summary`, `detail`, `range`), not from `stderr()`; under `Representation::Ndjson`,
`stdout()` is the whole stream and `diagnostic()` finds the error entry in it.
A status a handler declared with `with_exit_status` is a success:
`assert_success()` holds and `diagnostic()` is `None`; check it with `assert_exit_status`.
`assert_schema_snapshot("list.json")` pins a document's key names and value
types under `tests/schemas/`; `STANDOUT_UPDATE_SNAPSHOTS=1` rewrites it.

Use JSON to assert returned shape, `color(ColorPolicy::Never)` for rendered
strings, and `Representation::TermDebug` for style tags. See
`crates/standout-test/src/lib.rs`, `docs/topics/testing.md`,
`crates/todo-example/tdoo/src/app.rs`, and `crates/todo-example/tdoo/tests/`.
