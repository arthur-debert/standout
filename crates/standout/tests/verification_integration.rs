use clap::{Arg, Command};
use serde::Serialize;
use standout::cli::{App, Output};
use standout::handler;

#[derive(Serialize)]
struct Empty;

#[handler]
fn my_verified_handler(#[arg] foo: String) -> Result<standout::cli::Output<Empty>, anyhow::Error> {
    Ok(Output::Render(Empty))
}

#[test]
fn test_verification_success() {
    // Correct command definition: "test" subcommand with required "foo" arg
    let cmd_def =
        Command::new("app").subcommand(Command::new("test").arg(Arg::new("foo").required(true)));

    // Register handler using the generated struct "my_verified_handler_Handler"
    let app = App::builder()
        .command_handler("test", my_verified_handler_Handler, "")
        .unwrap()
        .build()
        .unwrap();

    // Verification should pass
    assert!(app.verify_command(&cmd_def).is_ok());
}

#[test]
fn test_verification_failure_missing_arg() {
    // Incorrect definition: missing "foo" arg
    let cmd_def = Command::new("app").subcommand(Command::new("test"));

    let app = App::builder()
        .command_handler("test", my_verified_handler_Handler, "")
        .unwrap()
        .build()
        .unwrap();

    let res = app.verify_command(&cmd_def);
    assert!(res.is_err());

    let err = res.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("verification failed"));
    assert!(msg.contains("foo")); // Should mention missing arg
}

#[test]
fn test_verification_failure_wrong_type() {
    // Incorrect definition: "foo" is flag instead of taking value (or vice versa? String implies required arg)
    // If we define "foo" as a flag, it won't match "required_arg".
    let cmd_def = Command::new("app").subcommand(
        Command::new("test").arg(Arg::new("foo").action(clap::ArgAction::SetTrue)), // Flag
    );

    let app = App::builder()
        .command_handler("test", my_verified_handler_Handler, "")
        .unwrap()
        .build()
        .unwrap();

    let res = app.verify_command(&cmd_def);
    assert!(res.is_err());
    let msg = res.unwrap_err().to_string();
    assert!(msg.contains("verification failed"));
    assert!(msg.contains("foo"));
}
