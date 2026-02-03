use clap::{Arg, ArgAction, Command};
use standout_dispatch::verify::{verify_handler_args, ExpectedArg};

#[test]
fn test_repro_default_value_false_positive() {
    // Scenario: Clap has default_value (so it's optional at runtime),
    // but Handler expects required String (which is fine because Clap provides dependency).
    let command = Command::new("test").arg(Arg::new("mode").long("mode").default_value("fast"));

    let expected = vec![ExpectedArg::required_arg("mode", "mode")];

    // This should PASS now (fix implemented)
    let result = verify_handler_args(&command, "handler", &expected);
    assert!(
        result.is_ok(),
        "Verification failed: {}",
        result.unwrap_err()
    );
}

#[test]
fn test_repro_count_false_positive() {
    // Scenario: Clap has ArgAction::Count, Handler expects u8/usize.
    let command = Command::new("test").arg(Arg::new("verbose").short('v').action(ArgAction::Count));

    let expected = vec![ExpectedArg::required_arg("verbose", "verbose")];

    // This should PASS now (fix implemented)
    let result = verify_handler_args(&command, "handler", &expected);
    assert!(
        result.is_ok(),
        "Verification failed: {}",
        result.unwrap_err()
    );
}
