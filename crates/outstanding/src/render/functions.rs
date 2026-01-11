//! Core rendering functions.

use minijinja::{Environment, Error};
use serde::Serialize;

use super::filters::register_filters;
use crate::output::OutputMode;
use crate::theme::Theme;

/// Renders a template with automatic terminal color detection.
///
/// This is the simplest way to render styled output. It automatically detects
/// whether stdout supports colors and applies styles accordingly. Color mode
/// (light/dark) is detected from OS settings.
///
/// # Arguments
///
/// * `template` - A minijinja template string
/// * `data` - Any serializable data to pass to the template
/// * `theme` - Theme definitions to use for the `style` filter
///
/// # Example
///
/// ```rust
/// use outstanding::{render, Theme};
/// use console::Style;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { message: String }
///
/// let theme = Theme::new().add("ok", Style::new().green());
/// let output = render(
///     r#"{{ message | style("ok") }}"#,
///     &Data { message: "Success!".into() },
///     &theme,
/// ).unwrap();
/// ```
pub fn render<T: Serialize>(template: &str, data: &T, theme: &Theme) -> Result<String, Error> {
    render_with_output(template, data, theme, OutputMode::Auto)
}

/// Renders a template with explicit output mode control.
///
/// Use this when you need to override automatic terminal detection,
/// for example when honoring a `--output=text` CLI flag. Color mode
/// (light/dark) is detected from OS settings.
///
/// # Arguments
///
/// * `template` - A minijinja template string
/// * `data` - Any serializable data to pass to the template
/// * `theme` - Theme definitions to use for the `style` filter
/// * `mode` - Output mode: `Auto`, `Term`, or `Text`
///
/// # Example
///
/// ```rust
/// use outstanding::{render_with_output, Theme, OutputMode};
/// use console::Style;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { status: String }
///
/// let theme = Theme::new().add("ok", Style::new().green());
///
/// // Force plain text output
/// let plain = render_with_output(
///     r#"{{ status | style("ok") }}"#,
///     &Data { status: "done".into() },
///     &theme,
///     OutputMode::Text,
/// ).unwrap();
/// assert_eq!(plain, "done"); // No ANSI codes
///
/// // Force terminal output (with ANSI codes)
/// let term = render_with_output(
///     r#"{{ status | style("ok") }}"#,
///     &Data { status: "done".into() },
///     &theme,
///     OutputMode::Term,
/// ).unwrap();
/// // Contains ANSI codes for green
/// ```
pub fn render_with_output<T: Serialize>(
    template: &str,
    data: &T,
    theme: &Theme,
    mode: OutputMode,
) -> Result<String, Error> {
    // Validate style aliases before rendering
    theme
        .validate()
        .map_err(|e| Error::new(minijinja::ErrorKind::InvalidOperation, e.to_string()))?;

    // Detect color mode and resolve styles for that mode
    let color_mode = crate::theme::detect_color_mode();
    let styles = theme.resolve_styles(Some(color_mode));

    let mut env = Environment::new();
    register_filters(&mut env, styles, mode);

    env.add_template_owned("_inline".to_string(), template.to_string())?;
    let tmpl = env.get_template("_inline")?;
    tmpl.render(data)
}

/// Renders data using a template, or serializes directly for structured output modes.
///
/// This is the recommended function when you want to support both human-readable
/// output (terminal, text) and machine-readable output (JSON). For structured modes
/// like `Json`, the data is serialized directly, skipping template rendering entirely.
///
/// # Arguments
///
/// * `template` - A minijinja template string (ignored for structured modes)
/// * `data` - Any serializable data to render or serialize
/// * `theme` - Theme definitions for the `style` filter (ignored for structured modes)
/// * `mode` - Output mode determining the output format
///
/// # Example
///
/// ```rust
/// use outstanding::{render_or_serialize, Theme, OutputMode};
/// use console::Style;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Report { title: String, count: usize }
///
/// let theme = Theme::new().add("title", Style::new().bold());
/// let data = Report { title: "Summary".into(), count: 42 };
///
/// // Terminal output uses the template
/// let term = render_or_serialize(
///     r#"{{ title | style("title") }}: {{ count }}"#,
///     &data,
///     &theme,
///     OutputMode::Text,
/// ).unwrap();
/// assert_eq!(term, "Summary: 42");
///
/// // JSON output serializes directly
/// let json = render_or_serialize(
///     r#"{{ title | style("title") }}: {{ count }}"#,
///     &data,
///     &theme,
///     OutputMode::Json,
/// ).unwrap();
/// assert!(json.contains("\"title\": \"Summary\""));
/// assert!(json.contains("\"count\": 42"));
/// ```
pub fn render_or_serialize<T: Serialize>(
    template: &str,
    data: &T,
    theme: &Theme,
    mode: OutputMode,
) -> Result<String, Error> {
    if mode.is_structured() {
        match mode {
            OutputMode::Json => serde_json::to_string_pretty(data)
                .map_err(|e| Error::new(minijinja::ErrorKind::InvalidOperation, e.to_string())),
            _ => unreachable!("is_structured() returned true for non-structured mode"),
        }
    } else {
        render_with_output(template, data, theme, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use console::Style;
    use serde::Serialize;

    #[derive(Serialize)]
    struct SimpleData {
        message: String,
    }

    #[derive(Serialize)]
    struct ListData {
        items: Vec<String>,
        count: usize,
    }

    #[test]
    fn test_render_with_output_text_no_ansi() {
        let theme = Theme::new().add("red", Style::new().red());
        let data = SimpleData {
            message: "test".into(),
        };

        let output = render_with_output(
            r#"{{ message | style("red") }}"#,
            &data,
            &theme,
            OutputMode::Text,
        )
        .unwrap();

        assert_eq!(output, "test");
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn test_render_with_output_term_has_ansi() {
        let theme = Theme::new().add("green", Style::new().green().force_styling(true));
        let data = SimpleData {
            message: "success".into(),
        };

        let output = render_with_output(
            r#"{{ message | style("green") }}"#,
            &data,
            &theme,
            OutputMode::Term,
        )
        .unwrap();

        assert!(output.contains("success"));
        assert!(output.contains("\x1b["));
    }

    #[test]
    fn test_render_unknown_style_shows_indicator() {
        let theme = Theme::new();
        let data = SimpleData {
            message: "hello".into(),
        };

        let output = render_with_output(
            r#"{{ message | style("unknown") }}"#,
            &data,
            &theme,
            OutputMode::Term,
        )
        .unwrap();

        assert_eq!(output, "(!?) hello");
    }

    #[test]
    fn test_render_unknown_style_shows_indicator_no_color() {
        let theme = Theme::new();
        let data = SimpleData {
            message: "hello".into(),
        };

        let output = render_with_output(
            r#"{{ message | style("unknown") }}"#,
            &data,
            &theme,
            OutputMode::Text,
        )
        .unwrap();

        assert_eq!(output, "(!?) hello");
    }

    #[test]
    fn test_render_template_with_loop() {
        let theme = Theme::new().add("item", Style::new().cyan());
        let data = ListData {
            items: vec!["one".into(), "two".into()],
            count: 2,
        };

        let template = r#"{% for item in items %}{{ item | style("item") }}
{% endfor %}"#;

        let output = render_with_output(template, &data, &theme, OutputMode::Text).unwrap();
        assert_eq!(output, "one\ntwo\n");
    }

    #[test]
    fn test_render_mixed_styled_and_plain() {
        let theme = Theme::new().add("count", Style::new().bold());
        let data = ListData {
            items: vec![],
            count: 42,
        };

        let template = r#"Total: {{ count | style("count") }} items"#;
        let output = render_with_output(template, &data, &theme, OutputMode::Text).unwrap();

        assert_eq!(output, "Total: 42 items");
    }

    #[test]
    fn test_render_literal_string_styled() {
        let theme = Theme::new().add("header", Style::new().bold());

        #[derive(Serialize)]
        struct Empty {}

        let output = render_with_output(
            r#"{{ "Header" | style("header") }}"#,
            &Empty {},
            &theme,
            OutputMode::Text,
        )
        .unwrap();

        assert_eq!(output, "Header");
    }

    #[test]
    fn test_empty_template() {
        let theme = Theme::new();

        #[derive(Serialize)]
        struct Empty {}

        let output = render_with_output("", &Empty {}, &theme, OutputMode::Text).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn test_template_syntax_error() {
        let theme = Theme::new();

        #[derive(Serialize)]
        struct Empty {}

        let result = render_with_output("{{ unclosed", &Empty {}, &theme, OutputMode::Text);
        assert!(result.is_err());
    }

    #[test]
    fn test_style_filter_with_nested_data() {
        #[derive(Serialize)]
        struct Item {
            name: String,
            value: i32,
        }

        #[derive(Serialize)]
        struct Container {
            items: Vec<Item>,
        }

        let theme = Theme::new().add("name", Style::new().bold());
        let data = Container {
            items: vec![
                Item {
                    name: "foo".into(),
                    value: 1,
                },
                Item {
                    name: "bar".into(),
                    value: 2,
                },
            ],
        };

        let template = r#"{% for item in items %}{{ item.name | style("name") }}={{ item.value }}
{% endfor %}"#;

        let output = render_with_output(template, &data, &theme, OutputMode::Text).unwrap();
        assert_eq!(output, "foo=1\nbar=2\n");
    }

    #[test]
    fn test_render_with_output_term_debug() {
        let theme = Theme::new()
            .add("title", Style::new().bold())
            .add("count", Style::new().cyan());

        #[derive(Serialize)]
        struct Data {
            name: String,
            value: usize,
        }

        let data = Data {
            name: "Test".into(),
            value: 42,
        };

        let output = render_with_output(
            r#"{{ name | style("title") }}: {{ value | style("count") }}"#,
            &data,
            &theme,
            OutputMode::TermDebug,
        )
        .unwrap();

        assert_eq!(output, "[title]Test[/title]: [count]42[/count]");
    }

    #[test]
    fn test_render_with_output_term_debug_missing_style() {
        let theme = Theme::new().add("known", Style::new().bold());

        #[derive(Serialize)]
        struct Data {
            message: String,
        }

        let data = Data {
            message: "hello".into(),
        };

        let output = render_with_output(
            r#"{{ message | style("unknown") }}"#,
            &data,
            &theme,
            OutputMode::TermDebug,
        )
        .unwrap();

        assert_eq!(output, "(!?) hello");

        let output = render_with_output(
            r#"{{ message | style("known") }}"#,
            &data,
            &theme,
            OutputMode::TermDebug,
        )
        .unwrap();

        assert_eq!(output, "[known]hello[/known]");
    }

    #[test]
    fn test_render_or_serialize_json_mode() {
        use serde_json::json;

        let theme = Theme::new();
        let data = json!({"name": "test", "count": 42});

        let output =
            render_or_serialize("unused template", &data, &theme, OutputMode::Json).unwrap();

        assert!(output.contains("\"name\": \"test\""));
        assert!(output.contains("\"count\": 42"));
    }

    #[test]
    fn test_render_or_serialize_text_mode_uses_template() {
        use serde_json::json;

        let theme = Theme::new();
        let data = json!({"name": "test"});

        let output =
            render_or_serialize("Name: {{ name }}", &data, &theme, OutputMode::Text).unwrap();

        assert_eq!(output, "Name: test");
    }

    #[test]
    fn test_render_or_serialize_term_mode_uses_template() {
        use serde_json::json;

        let theme = Theme::new().add("bold", Style::new().bold().force_styling(true));
        let data = json!({"name": "test"});

        let output = render_or_serialize(
            r#"{{ name | style("bold") }}"#,
            &data,
            &theme,
            OutputMode::Term,
        )
        .unwrap();

        assert!(output.contains("\x1b[1m"));
        assert!(output.contains("test"));
    }

    #[test]
    fn test_render_or_serialize_json_with_struct() {
        #[derive(Serialize)]
        struct Report {
            title: String,
            items: Vec<String>,
        }

        let theme = Theme::new();
        let data = Report {
            title: "Summary".into(),
            items: vec!["one".into(), "two".into()],
        };

        let output = render_or_serialize("unused", &data, &theme, OutputMode::Json).unwrap();

        assert!(output.contains("\"title\": \"Summary\""));
        assert!(output.contains("\"items\""));
        assert!(output.contains("\"one\""));
    }

    #[test]
    fn test_render_with_alias() {
        let theme = Theme::new()
            .add("base", Style::new().bold())
            .add("alias", "base");

        let output = render_with_output(
            r#"{{ "text" | style("alias") }}"#,
            &serde_json::json!({}),
            &theme,
            OutputMode::Text,
        )
        .unwrap();

        assert_eq!(output, "text");
    }

    #[test]
    fn test_render_with_alias_chain() {
        let theme = Theme::new()
            .add("muted", Style::new().dim())
            .add("disabled", "muted")
            .add("timestamp", "disabled");

        let output = render_with_output(
            r#"{{ "12:00" | style("timestamp") }}"#,
            &serde_json::json!({}),
            &theme,
            OutputMode::Text,
        )
        .unwrap();

        assert_eq!(output, "12:00");
    }

    #[test]
    fn test_render_fails_with_dangling_alias() {
        let theme = Theme::new().add("orphan", "missing");

        let result = render_with_output(
            r#"{{ "text" | style("orphan") }}"#,
            &serde_json::json!({}),
            &theme,
            OutputMode::Text,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("orphan"));
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn test_render_fails_with_cycle() {
        let theme = Theme::new().add("a", "b").add("b", "a");

        let result = render_with_output(
            r#"{{ "text" | style("a") }}"#,
            &serde_json::json!({}),
            &theme,
            OutputMode::Text,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_three_layer_styling_pattern() {
        let theme = Theme::new()
            .add("dim_style", Style::new().dim())
            .add("cyan_bold", Style::new().cyan().bold())
            .add("yellow_bg", Style::new().on_yellow())
            .add("muted", "dim_style")
            .add("accent", "cyan_bold")
            .add("highlighted", "yellow_bg")
            .add("timestamp", "muted")
            .add("title", "accent")
            .add("selected_item", "highlighted");

        assert!(theme.validate().is_ok());

        let output = render_with_output(
            r#"{{ time | style("timestamp") }} - {{ name | style("title") }}"#,
            &serde_json::json!({"time": "12:00", "name": "Report"}),
            &theme,
            OutputMode::Text,
        )
        .unwrap();

        assert_eq!(output, "12:00 - Report");
    }
}
