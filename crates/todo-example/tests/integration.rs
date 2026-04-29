//! End-to-end tests that drive the full pipeline (clap → dispatch →
//! handler → template → output) in-process via `standout-test`.
//!
//! `TestHarness` mutates process-global state (env, cwd, default stdin
//! reader, …) so every test is `#[serial]`.

use serial_test::serial;
use standout::cli::App;
use standout_test::TestHarness;
use tempfile::TempDir;
use todo_example::{build_app, cli_command, TodoStore};

/// Builds a fresh app pointed at a tempdir-backed store. The TempDir is
/// returned alongside so the caller can keep it alive for the duration of
/// the test.
fn fresh_app() -> (App, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = TodoStore::load(dir.path().join("todos.json")).unwrap();
    (build_app(store).unwrap(), dir)
}

#[test]
#[serial]
fn list_on_empty_store_shows_empty_state() {
    let (app, _dir) = fresh_app();
    let result = TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "list"]);

    result.assert_success();
    result.assert_stdout_contains("Nothing here yet");
}

#[test]
#[serial]
fn add_then_list_round_trips_through_store() {
    let (app, _dir) = fresh_app();

    TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "add", "--title", "buy milk"])
        .assert_success();

    let listed = TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "list"]);
    listed.assert_success();
    listed.assert_stdout_contains("buy milk");
    listed.assert_stdout_contains("#1");
}

#[test]
#[serial]
fn add_reads_title_from_piped_stdin_when_arg_is_absent() {
    let (app, _dir) = fresh_app();

    let added = TestHarness::new()
        .no_color()
        .piped_stdin("ship the docs\n")
        .run(&app, cli_command(), ["tdoo", "add"]);
    added.assert_success();
    added.assert_stdout_contains("ship the docs");
}

#[test]
#[serial]
fn add_rejects_empty_title_via_chain_validator() {
    let (app, _dir) = fresh_app();

    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cli_command(), ["tdoo", "add", "--title", "   "]);
    // The chain validator runs in pre-dispatch and aborts before the
    // handler. The framework surfaces this as RunResult::Error which
    // would be written to stderr and produce a non-zero exit code in `run()`.
    result.assert_error_contains("title cannot be empty");
}

#[test]
#[serial]
fn done_flips_status_and_list_all_shows_both_states() {
    let (app, _dir) = fresh_app();

    TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "add", "--title", "first"])
        .assert_success();
    TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "add", "--title", "second"])
        .assert_success();
    TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "done", "1"])
        .assert_success();

    // Default `list` filters out done todos.
    let pending_only = TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "list"]);
    pending_only.assert_success();
    pending_only.assert_stdout_contains("second");
    assert!(
        !pending_only.stdout().contains("first"),
        "completed todo should be hidden without --all; got:\n{}",
        pending_only.stdout()
    );

    // `--all` shows everything.
    let everything =
        TestHarness::new()
            .no_color()
            .run(&app, cli_command(), ["tdoo", "list", "--all"]);
    everything.assert_success();
    everything.assert_stdout_contains("first");
    everything.assert_stdout_contains("second");
}

#[test]
#[serial]
fn json_output_mode_serializes_handler_data_directly() {
    let (app, _dir) = fresh_app();

    TestHarness::new()
        .no_color()
        .run(&app, cli_command(), ["tdoo", "add", "--title", "buy milk"])
        .assert_success();

    let result = TestHarness::new().no_color().run(
        &app,
        cli_command(),
        ["tdoo", "list", "--output", "json"],
    );
    result.assert_success();
    let stdout = result.stdout();
    assert!(stdout.contains("\"title\""));
    assert!(stdout.contains("buy milk"));
    assert!(stdout.contains("\"total\""));
}

#[test]
#[serial]
fn audit_hook_writes_a_log_line_when_env_is_set() {
    let (app, dir) = fresh_app();
    let log_path = dir.path().join("audit.log");

    TestHarness::new()
        .no_color()
        .env("TODO_AUDIT_LOG", log_path.to_str().unwrap())
        .run(&app, cli_command(), ["tdoo", "add", "--title", "audited"])
        .assert_success();

    let log = std::fs::read_to_string(&log_path).expect("audit log written");
    assert!(log.contains("add\t1"), "unexpected audit log:\n{}", log);
}
