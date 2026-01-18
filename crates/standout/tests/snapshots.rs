#![cfg(feature = "clap")]

use clap::Command;
use insta::assert_snapshot;
use serde_json::json;
use standout::cli::{App, Output};
use standout::OutputMode;

#[test]
fn test_snapshots_term_output() {
    let app = App::<standout::cli::ThreadSafe>::builder()
        .command(
            "list",
            |_m, _ctx| {
                Ok(Output::Render(json!({
                    "items": ["apple", "banana", "cherry"],
                    "count": 3
                })))
            },
            "Items: {{ items }}\nCount: {{ count }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("list"));
    let matches = cmd.try_get_matches_from(["app", "list"]).unwrap();

    let result = app.dispatch(matches, OutputMode::Term);
    let output = result.output().unwrap();

    assert_snapshot!("term_list_output", output);
}

#[test]
fn test_snapshots_json_output() {
    let app = App::<standout::cli::ThreadSafe>::builder()
        .command(
            "list",
            |_m, _ctx| {
                Ok(Output::Render(json!({
                    "items": ["apple", "banana", "cherry"],
                    "count": 3
                })))
            },
            "Items: {{ items }}\nCount: {{ count }}",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("list"));
    let matches = cmd.try_get_matches_from(["app", "list"]).unwrap();

    let result = app.dispatch(matches, OutputMode::Json);
    let output = result.output().unwrap();

    // Verify structured output via snapshot
    // Sort keys to ensure stable snapshots if serde order varies?
    // Usually serde_json preserves order of insertion or alphabetical?
    // `json!` macro preserves order. `serde_json::to_string_pretty` might reorder?
    // Let's snapshot the raw string.
    assert_snapshot!("json_list_output", output);
}

#[test]
fn test_snapshots_error_handling() {
    let app = App::<standout::cli::ThreadSafe>::builder()
        .command(
            "fail",
            |_m, _ctx| -> standout::cli::HandlerResult<()> {
                Err(anyhow::anyhow!("Critical failure in operation"))
            },
            "",
        )
        .unwrap()
        .build()
        .unwrap();

    let cmd = Command::new("app").subcommand(Command::new("fail"));
    let matches = cmd.try_get_matches_from(["app", "fail"]).unwrap();

    let result = app.dispatch(matches, OutputMode::Term);

    // Result should be handled (Result::Handled) but containing error text?
    // Wait, App::dispatch returns RunResult.
    // If handler returns Err, App usually converts it to string error message?
    // Let's check `execution.rs`.
    // It calls `handler`. If Err, it converts to `Err(String)`.
    // Then dispatch catches it?
    // Ah, `dispatch` returns `RunResult::Handled(output)`.
    // If handler returns Err, `dispatch` might return `RunResult` containing the error string?
    // Actually `CommandRecipe::create_dispatch` returns `Result<DispatchOutput, String>`.
    // `dispatch` calls `recipe.dispatch()`.
    // If it returns Err(msg), `dispatch` prints it (if run()) or returns it?
    // `dispatch` implementation:
    // match recipe.dispatch(...) { Ok(output) => RunResult::Handled(output), Err(msg) => RunResult::Handled(format!("Error: {}", msg)) }
    // So output should contain "Error: ...".

    let output = result.output().unwrap();
    assert_snapshot!("error_output", output);
}
