use clap::Command;
use console::Style;
use standout::cli::{App, Output};
use standout::Theme;

#[test]
fn test_late_binding_theme_sequencing() {
    // 1. Create a theme with a custom style
    // Note: force_styling(true) is required in tests because there's no TTY
    let style = Style::new().cyan().force_styling(true);
    let theme = Theme::new().add("issue_89_style", style);

    // 2. Build the app with OUT-OF-ORDER configuration:
    //    Command is registered BEFORE the theme is set.
    //    Prior to the fix, this would capture the default (empty) theme.
    //    With Late Binding, it should use the theme provided at runtime.
    let app = App::builder()
        .command(
            "late_bind",
            |_m, _ctx| Ok(Output::Render("late_content".to_string())),
            "[issue_89_style]late_content[/issue_89_style]",
        )
        .unwrap()
        .theme(theme) // Theme set AFTER command registration
        .build()
        .expect("Failed to build app");

    // 3. Run to string
    let cmd = Command::new("app").subcommand(Command::new("late_bind"));

    // We simulate passing "--output=term" to force terminal output with colors
    let result = app.run_to_string(cmd, ["app", "--output=term", "late_bind"]);

    match result {
        standout::cli::RunResult::Handled(output) => {
            // 4. Verification: If theme works, output should contain ANSI cyan code: \x1b[36m
            assert!(
                output.contains("\x1b[36m"),
                "Output should contain Cyan ANSI code for late bound theme, but got: {:?}",
                output
            );
        }
        _ => panic!("Expected handled result, got {:?}", result),
    }
}
