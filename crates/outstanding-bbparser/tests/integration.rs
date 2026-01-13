use console::Style;
use outstanding_bbparser::{BBParser, TagTransform};
use std::collections::HashMap;

fn test_styles() -> HashMap<String, Style> {
    let mut styles = HashMap::new();
    styles.insert("red".to_string(), Style::new().red().force_styling(true));
    styles.insert("bold".to_string(), Style::new().bold().force_styling(true));
    styles
}

#[test]
fn test_output_modes() {
    let styles = test_styles();
    let input = "[red]hello[/red] [bold]world[/bold]";

    // Test Keep (Debug)
    let parser = BBParser::new(styles.clone(), TagTransform::Keep);
    assert_eq!(parser.process(input), input);

    // Test Remove (Plain)
    let parser = BBParser::new(styles.clone(), TagTransform::Remove);
    assert_eq!(parser.process(input), "hello world");

    // Test Apply (Term)
    let parser = BBParser::new(styles.clone(), TagTransform::Apply);
    let output = parser.process(input);

    // Check it contains ANSI codes (basic check)
    assert!(output.contains("\x1b[31m")); // Red
    assert!(output.contains("\x1b[1m")); // Bold
                                         // Check content is preserved
    assert!(output.contains("hello"));
    assert!(output.contains("world"));
}

#[test]
fn test_compact_ansi_output() {
    // This test specifically targets the performance/bloat issue.
    // The current implementation creates a new style wrapper for EVERY character.
    // Fixed implementation should wrap the whole word.

    let styles = test_styles();
    let parser = BBParser::new(styles, TagTransform::Apply);

    let input = "[red]text[/red]";
    let output = parser.process(input);

    // Expected efficient output: \x1b[31mtext\x1b[0m
    // Bloated output would have many more escapes.

    let escape_count = output.matches("\x1b[").count();

    // In efficient output:
    // 1. Start red (\x1b[31m)
    // 2. Reset (\x1b[0m)
    // Total 2 escapes.

    // In bloated output (char-by-char):
    // t: \x1b[31mt\x1b[0m (2 escapes)
    // e: \x1b[31me\x1b[0m (2 escapes)
    // x: \x1b[31mx\x1b[0m (2 escapes)
    // t: \x1b[31mt\x1b[0m (2 escapes)
    // Total 8 escapes for 4 chars.

    // We assert bound to ensure we don't regress.
    // We expect <= 2 escapes (start + end).
    // Allow slightly loose bound for future changes, but definitely < 2 * length.
    assert!(
        escape_count <= 2,
        "Output too bloated! Found {} escapes for 'text'. Output: {:?}",
        escape_count,
        output
    );
}

#[test]
fn test_nested_tags_apply() {
    let styles = test_styles();
    let parser = BBParser::new(styles, TagTransform::Apply);

    // [bold][red]hi[/red][/bold]
    let output = parser.process("[bold][red]hi[/red][/bold]");

    // Should have bold on outer, red on inner.
    assert!(output.contains("hi"));
}
