#![cfg(feature = "clap")]

use clap::Command;
use proptest::prelude::*;
use serde_json::{json, Value};
use serde_yaml;
use standout::cli::{App, Output};
use standout::{OutputMode, Theme};

// Strategy for generating arbitrary OutputMode values
// Per design guidelines: all 8 output modes must be covered
fn output_mode_strategy() -> impl Strategy<Value = OutputMode> {
    prop_oneof![
        Just(OutputMode::Auto),
        Just(OutputMode::Term),
        Just(OutputMode::Text),
        Just(OutputMode::TermDebug),
        Just(OutputMode::Json),
        Just(OutputMode::Yaml),
        Just(OutputMode::Xml),
        Just(OutputMode::Csv),
    ]
}

// Strategy for generating arbitrary Themes (None vs Some)
fn theme_strategy() -> impl Strategy<Value = Option<Theme>> {
    prop_oneof![
        Just(None),
        Just(Some(Theme::new())),
        // Could add populated themes later
    ]
}

// Strategy for generating arbitrary JSON data
fn json_data_strategy() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<f64>().prop_map(|f| json!(f)),
        "[a-zA-Z0-9]*".prop_map(Value::String),
    ];
    leaf.prop_recursive(
        4,  // 4 levels deep
        64, // Max size 64 nodes
        10, // Items per collection
        |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..10).prop_map(Value::Array),
                prop::collection::hash_map("[a-zA-Z0-9]*", inner, 0..10)
                    .prop_map(|m| { Value::Object(m.into_iter().collect()) })
            ]
        },
    )
}

proptest! {
    #[test]
    fn test_rendering_invariants(
        mode in output_mode_strategy(),
        theme in theme_strategy(),
        data in json_data_strategy()
    ) {
        // App definition
        let builder = App::<standout::cli::ThreadSafe>::builder()
            .command(
                "test", // Command name
                move |_m, _ctx| Ok(Output::Render(data.clone())), // Handler returns prop data
                "{{ . }}", // Simple template dumping data
            ).unwrap();

        // Inject theme if generated
        let builder = if let Some(t) = theme {
            builder.theme(t)
        } else {
            builder
        };

        let app = builder.build().expect("Failed to build app");

        // Build command and parse args to get ArgMatches
        let cmd = Command::new("app").subcommand(Command::new("test"));
        let matches = cmd.try_get_matches_from(["app", "test"]).unwrap();

        // Run dispatch
        let result = app.dispatch(matches, mode);

        // Verification
        // 1. Should be handled
        assert!(result.is_handled());

        // 2. Structured output validation
        if let Some(output) = result.output() {
            match mode {
                OutputMode::Json => {
                    let parsed: Result<Value, _> = serde_json::from_str(output);
                    assert!(parsed.is_ok(), "JSON output should be parseable: {}", output);
                }
                OutputMode::Yaml => {
                    let parsed: Result<Value, _> = serde_yaml::from_str(output);
                    assert!(parsed.is_ok(), "YAML output should be parseable: {}", output);
                }
                // XML and CSV are harder to validate generically, but we verify no panic
                _ => {}
            }
        }

        // 3. No panic occurred (implicit by reaching here)
    }
}
