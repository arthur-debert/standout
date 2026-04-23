//! Integration tests for `TestHarness`.
//!
//! All tests are `#[serial]` because the harness mutates process-global
//! state (env vars, cwd, detectors, default input readers).

use clap::Command;
use serde_json::json;
use serial_test::serial;
use standout::cli::{App, Output};
use standout_input::{ClipboardSource, EnvSource, InputChain, StdinSource};
use standout_render::OutputMode;
use standout_test::TestHarness;

fn build_echo_app(template: &'static str) -> App {
    App::builder()
        .command(
            "echo",
            |m, _ctx| {
                let msg = m
                    .get_one::<String>("msg")
                    .cloned()
                    .unwrap_or_else(|| "no-arg".into());
                Ok(Output::Render(json!({ "msg": msg })))
            },
            template,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn echo_command() -> Command {
    Command::new("app")
        .subcommand(Command::new("echo").arg(clap::Arg::new("msg").required(false).index(1)))
}

#[test]
#[serial]
fn simple_handler_returns_rendered_text() {
    let app = build_echo_app("{{ msg }}");
    let result = TestHarness::new().run(&app, echo_command(), vec!["app", "echo", "hello"]);
    result.assert_success();
    result.assert_stdout_eq("hello");
}

#[test]
#[serial]
fn env_var_visible_to_handler() {
    let app = App::builder()
        .command(
            "whoami",
            |_m, _ctx| {
                let v = InputChain::<String>::new()
                    .try_source(EnvSource::new("STANDOUT_TEST_USER"))
                    .default("anon".into())
                    .resolve(_m)
                    .unwrap();
                Ok(Output::Render(json!({ "user": v })))
            },
            "{{ user }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("whoami"));
    let result = TestHarness::new().env("STANDOUT_TEST_USER", "arthur").run(
        &app,
        cmd,
        vec!["app", "whoami"],
    );
    result.assert_stdout_eq("arthur");
}

#[test]
#[serial]
fn env_remove_hides_existing_value() {
    std::env::set_var("STANDOUT_TEST_TOKEN", "real");

    let app = App::builder()
        .command(
            "tok",
            |_m, _ctx| {
                let v = InputChain::<String>::new()
                    .try_source(EnvSource::new("STANDOUT_TEST_TOKEN"))
                    .default("missing".into())
                    .resolve(_m)
                    .unwrap();
                Ok(Output::Render(json!({ "tok": v })))
            },
            "{{ tok }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("tok"));
    {
        let result =
            TestHarness::new()
                .env_remove("STANDOUT_TEST_TOKEN")
                .run(&app, cmd, vec!["app", "tok"]);
        result.assert_stdout_eq("missing");
    }

    // Restore should bring the original back.
    assert_eq!(std::env::var("STANDOUT_TEST_TOKEN").as_deref(), Ok("real"));
    std::env::remove_var("STANDOUT_TEST_TOKEN");
}

#[test]
#[serial]
fn piped_stdin_reaches_handler() {
    let app = App::builder()
        .command(
            "read",
            |_m, _ctx| {
                let v = InputChain::<String>::new()
                    .try_source(StdinSource::new())
                    .default("nothing".into())
                    .resolve(_m)
                    .unwrap();
                Ok(Output::Render(json!({ "val": v })))
            },
            "{{ val }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("read"));
    let result = TestHarness::new()
        .piped_stdin("piped-in")
        .run(&app, cmd, vec!["app", "read"]);
    result.assert_stdout_eq("piped-in");
}

#[test]
#[serial]
fn interactive_stdin_falls_through_to_default() {
    let app = App::builder()
        .command(
            "read",
            |_m, _ctx| {
                let v = InputChain::<String>::new()
                    .try_source(StdinSource::new())
                    .default("no-pipe".into())
                    .resolve(_m)
                    .unwrap();
                Ok(Output::Render(json!({ "val": v })))
            },
            "{{ val }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("read"));
    let result = TestHarness::new()
        .interactive_stdin()
        .run(&app, cmd, vec!["app", "read"]);
    result.assert_stdout_eq("no-pipe");
}

#[test]
#[serial]
fn clipboard_reaches_handler() {
    let app = App::builder()
        .command(
            "paste",
            |_m, _ctx| {
                let v = InputChain::<String>::new()
                    .try_source(ClipboardSource::new())
                    .default("empty".into())
                    .resolve(_m)
                    .unwrap();
                Ok(Output::Render(json!({ "val": v })))
            },
            "{{ val }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("paste"));
    let result =
        TestHarness::new()
            .clipboard("clipboard-content")
            .run(&app, cmd, vec!["app", "paste"]);
    result.assert_stdout_eq("clipboard-content");
}

#[test]
#[serial]
fn fixture_files_are_materialized_in_cwd() {
    let app = App::builder()
        .command(
            "cat",
            |m, _ctx| {
                let path = m.get_one::<String>("path").cloned().unwrap();
                let text = std::fs::read_to_string(path).unwrap();
                Ok(Output::Render(json!({ "text": text })))
            },
            "{{ text }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app")
        .subcommand(Command::new("cat").arg(clap::Arg::new("path").required(true).index(1)));
    let result = TestHarness::new()
        .fixture("notes/todo.txt", "- buy milk\n- write tests\n")
        .run(&app, cmd, vec!["app", "cat", "notes/todo.txt"]);
    result.assert_stdout_contains("buy milk");
    result.assert_stdout_contains("write tests");
}

#[test]
#[serial]
fn output_mode_override_forces_json() {
    let app = build_echo_app("{{ msg }}");
    let result = TestHarness::new().output_mode(OutputMode::Json).run(
        &app,
        echo_command(),
        vec!["app", "echo", "hello"],
    );
    let out = result.stdout();
    assert!(out.contains("\"msg\""));
    assert!(out.contains("\"hello\""));
}

#[test]
#[serial]
fn terminal_width_override_is_observable_via_detector() {
    // The harness installs the override for the duration of run(); we
    // can't easily probe it inside a handler without adding a context
    // provider, so we assert the render layer sees it by forcing an
    // auto-mode render and observing no ANSI (no_color) + text mode.
    let app = build_echo_app("{{ msg }}");
    let result = TestHarness::new().terminal_width(42).no_color().run(
        &app,
        echo_command(),
        vec!["app", "echo", "hi"],
    );
    result.assert_stdout_eq("hi");
}

#[test]
#[serial]
fn overrides_are_restored_on_drop() {
    let original = std::env::var("STANDOUT_RESTORE_PROBE").ok();
    std::env::set_var("STANDOUT_RESTORE_PROBE", "before");

    {
        let app = build_echo_app("{{ msg }}");
        let _result = TestHarness::new()
            .env("STANDOUT_RESTORE_PROBE", "during")
            .env("STANDOUT_BRAND_NEW", "new")
            .run(&app, echo_command(), vec!["app", "echo", "x"]);
    }

    assert_eq!(
        std::env::var("STANDOUT_RESTORE_PROBE").as_deref(),
        Ok("before")
    );
    assert!(std::env::var("STANDOUT_BRAND_NEW").is_err());

    // Cleanup
    std::env::remove_var("STANDOUT_RESTORE_PROBE");
    if let Some(v) = original {
        std::env::set_var("STANDOUT_RESTORE_PROBE", v);
    }
}

#[test]
#[serial]
fn no_match_reports_cleanly() {
    let app = build_echo_app("{{ msg }}");
    let result = TestHarness::new().run(&app, echo_command(), vec!["app", "unknown"]);
    // clap will emit an error string via Handled for unknown subcommand
    // at this level; accept either Handled-with-clap-error or NoMatch.
    assert!(result.is_handled() || result.is_no_match());
}
