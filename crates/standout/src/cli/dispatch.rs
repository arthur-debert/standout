//! Command dispatch logic.
//!
//! Internal types and functions for dispatching commands to handlers.
//!
//! This module provides dispatch function types for both handler modes:
//!
//! - [`DispatchFn`]: Thread-safe dispatch using `Arc<dyn Fn + Send + Sync>`
//! - [`LocalDispatchFn`]: Local dispatch using `Rc<RefCell<dyn FnMut>>`

use clap::ArgMatches;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::cli::handler::CommandContext;
use crate::cli::hooks::Hooks;
use crate::context::{ContextRegistry, RenderContext};
use crate::{OutputMode, TemplateRegistry, Theme};

/// Internal result type for dispatch functions.
pub(crate) enum DispatchOutput {
    /// Text output (rendered template or JSON)
    Text(String),
    /// Binary output (bytes, filename)
    Binary(Vec<u8>, String),
    /// No output (silent)
    Silent,
}

/// Type-erased dispatch function for thread-safe handlers.
///
/// Takes ArgMatches, CommandContext, and optional Hooks. The hooks parameter
/// allows post-dispatch hooks to run between handler execution and rendering.
///
/// Used with [`App`](super::App) and [`Handler`](super::handler::Handler).
pub(crate) type DispatchFn = Arc<
    dyn Fn(&ArgMatches, &CommandContext, Option<&Hooks>) -> Result<DispatchOutput, String>
        + Send
        + Sync,
>;

/// Type-erased dispatch function for local (single-threaded) handlers.
///
/// Unlike [`DispatchFn`], this:
/// - Uses `Rc<RefCell<_>>` instead of `Arc` (no thread-safety overhead)
/// - Uses `FnMut` instead of `Fn` (allows mutable state)
/// - Does NOT require `Send + Sync`
///
/// Used with [`LocalApp`](super::LocalApp) and [`LocalHandler`](super::handler::LocalHandler).
pub(crate) type LocalDispatchFn = Rc<
    RefCell<
        dyn FnMut(&ArgMatches, &CommandContext, Option<&Hooks>) -> Result<DispatchOutput, String>,
    >,
>;

/// Extracts the command path from ArgMatches by following subcommand chain.
pub(crate) fn extract_command_path(matches: &ArgMatches) -> Vec<String> {
    let mut path = Vec::new();
    let mut current = matches;

    while let Some((name, sub)) = current.subcommand() {
        // Skip "help" as it's handled separately
        if name == "help" {
            break;
        }
        path.push(name.to_string());
        current = sub;
    }

    path
}

/// Gets the deepest subcommand matches.
pub(crate) fn get_deepest_matches(matches: &ArgMatches) -> &ArgMatches {
    let mut current = matches;

    while let Some((name, sub)) = current.subcommand() {
        if name == "help" {
            break;
        }
        current = sub;
    }

    current
}

/// Returns true if the matches contain a subcommand (excluding "help").
///
/// This is used to detect "naked" CLI invocations where no command was specified,
/// enabling default command behavior.
pub fn has_subcommand(matches: &ArgMatches) -> bool {
    matches
        .subcommand()
        .map(|(name, _)| name != "help")
        .unwrap_or(false)
}

/// Inserts a command name at position 1 (after program name) in the argument list.
///
/// This is used to implement default command support: when no subcommand is specified,
/// we insert the default command name and reparse.
///
/// # Example
///
/// ```ignore
/// let args = vec!["myapp".to_string(), "-v".to_string()];
/// let new_args = insert_default_command(args, "list");
/// assert_eq!(new_args, vec!["myapp", "list", "-v"]);
/// ```
pub fn insert_default_command<I, S>(args: I, command: &str) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut result: Vec<String> = args.into_iter().map(Into::into).collect();
    if !result.is_empty() {
        result.insert(1, command.to_string());
    } else {
        result.push(command.to_string());
    }
    result
}

// ============================================================================
// Shared Rendering Logic
// ============================================================================

/// Gets the current terminal width, or None if not available.
pub(crate) fn get_terminal_width() -> Option<usize> {
    terminal_size::terminal_size().map(|(w, _)| w.0 as usize)
}

/// Renders a template with optional template registry support for includes.
///
/// This consolidates template rendering logic used by all dispatch functions.
/// Handles both templated modes (Term, Text, Auto) and structured modes (JSON, YAML, etc).
pub(crate) fn render_with_registry(
    template: &str,
    data: &serde_json::Value,
    theme: &Theme,
    mode: OutputMode,
    context_registry: &ContextRegistry,
    render_ctx: &RenderContext,
    template_registry: Option<&TemplateRegistry>,
) -> Result<String, String> {
    use crate::rendering::template::filters::register_filters;
    use crate::rendering::theme::detect_color_mode;
    use minijinja::Environment;
    use standout_bbparser::{BBParser, TagTransform, UnknownTagBehavior};

    // For structured modes, serialize directly
    if mode.is_structured() {
        return match mode {
            OutputMode::Json => serde_json::to_string_pretty(data)
                .map_err(|e| format!("JSON serialization error: {}", e)),
            OutputMode::Yaml => {
                serde_yaml::to_string(data).map_err(|e| format!("YAML serialization error: {}", e))
            }
            OutputMode::Xml => quick_xml::se::to_string(data)
                .map_err(|e| format!("XML serialization error: {}", e)),
            OutputMode::Csv => {
                let (headers, rows) = crate::util::flatten_json_for_csv(data);
                let mut wtr = csv::Writer::from_writer(Vec::new());
                wtr.write_record(&headers)
                    .map_err(|e| format!("CSV write error: {}", e))?;
                for row in rows {
                    wtr.write_record(&row)
                        .map_err(|e| format!("CSV write error: {}", e))?;
                }
                let bytes = wtr
                    .into_inner()
                    .map_err(|e| format!("CSV finalization error: {}", e))?;
                String::from_utf8(bytes).map_err(|e| format!("CSV UTF-8 error: {}", e))
            }
            _ => Err(format!("Unexpected structured mode: {:?}", mode)),
        };
    }

    let color_mode = detect_color_mode();
    let styles = theme.resolve_styles(Some(color_mode));

    // Validate style aliases before rendering
    styles
        .validate()
        .map_err(|e| format!("Style validation error: {}", e))?;

    let mut env = Environment::new();
    register_filters(&mut env);

    // Load all templates from registry if available (enables {% include %})
    if let Some(registry) = template_registry {
        for name in registry.names() {
            if let Ok(content) = registry.get_content(name) {
                env.add_template_owned(name.to_string(), content)
                    .map_err(|e| format!("Template load error: {}", e))?;
            }
        }
    }

    env.add_template_owned("_inline".to_string(), template.to_string())
        .map_err(|e| format!("Template parse error: {}", e))?;
    let tmpl = env
        .get_template("_inline")
        .map_err(|e| format!("Template error: {}", e))?;

    // Build combined context
    let context_values = context_registry.resolve(render_ctx);
    let mut combined: HashMap<String, minijinja::Value> = HashMap::new();

    // Add context values first (lower priority)
    for (key, value) in context_values {
        combined.insert(key, value);
    }

    // Add data values (higher priority)
    if let Some(obj) = data.as_object() {
        for (key, value) in obj {
            combined.insert(key.clone(), minijinja::Value::from_serialize(value));
        }
    }

    // Pass 1: MiniJinja template rendering
    let minijinja_output = tmpl
        .render(&combined)
        .map_err(|e| format!("Template render error: {}", e))?;

    // Pass 2: BBParser style tag processing
    let transform = match mode {
        OutputMode::Term | OutputMode::Auto => TagTransform::Apply,
        OutputMode::TermDebug => TagTransform::Keep,
        _ => TagTransform::Remove,
    };
    let resolved_styles = styles.to_resolved_map();
    let parser =
        BBParser::new(resolved_styles, transform).unknown_behavior(UnknownTagBehavior::Passthrough);
    let final_output = parser.parse(&minijinja_output);

    Ok(final_output)
}

/// Renders handler output after post-dispatch hooks have been applied.
///
/// This is the common logic extracted from all recipe implementations.
/// It handles creating the render context and calling `render_with_registry`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_handler_output(
    json_data: serde_json::Value,
    matches: &ArgMatches,
    ctx: &CommandContext,
    hooks: Option<&Hooks>,
    template: &str,
    theme: &Theme,
    context_registry: &ContextRegistry,
    template_registry: Option<&TemplateRegistry>,
) -> Result<DispatchOutput, String> {
    // Run post-dispatch hooks if present
    let json_data = if let Some(hooks) = hooks {
        hooks
            .run_post_dispatch(matches, ctx, json_data)
            .map_err(|e| format!("Hook error: {}", e))?
    } else {
        json_data
    };

    let render_ctx = RenderContext::new(ctx.output_mode, get_terminal_width(), theme, &json_data);

    let output = render_with_registry(
        template,
        &json_data,
        theme,
        ctx.output_mode,
        context_registry,
        &render_ctx,
        template_registry,
    )?;

    Ok(DispatchOutput::Text(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command;

    #[test]
    fn test_extract_command_path() {
        let cmd =
            Command::new("app").subcommand(Command::new("config").subcommand(Command::new("get")));

        let matches = cmd.try_get_matches_from(["app", "config", "get"]).unwrap();
        let path = extract_command_path(&matches);

        assert_eq!(path, vec!["config", "get"]);
    }

    #[test]
    fn test_extract_command_path_single() {
        let cmd = Command::new("app").subcommand(Command::new("list"));

        let matches = cmd.try_get_matches_from(["app", "list"]).unwrap();
        let path = extract_command_path(&matches);

        assert_eq!(path, vec!["list"]);
    }

    #[test]
    fn test_extract_command_path_empty() {
        let cmd = Command::new("app");

        let matches = cmd.try_get_matches_from(["app"]).unwrap();
        let path = extract_command_path(&matches);

        assert!(path.is_empty());
    }

    #[test]
    fn test_has_subcommand_true() {
        let cmd = Command::new("app").subcommand(Command::new("list"));

        let matches = cmd.try_get_matches_from(["app", "list"]).unwrap();
        assert!(has_subcommand(&matches));
    }

    #[test]
    fn test_has_subcommand_false_no_subcommand() {
        let cmd = Command::new("app").subcommand(Command::new("list"));

        let matches = cmd.try_get_matches_from(["app"]).unwrap();
        assert!(!has_subcommand(&matches));
    }

    #[test]
    fn test_has_subcommand_false_help() {
        // Use disable_help_subcommand to avoid conflict with clap's built-in help
        let cmd = Command::new("app")
            .disable_help_subcommand(true)
            .subcommand(Command::new("help"));

        let matches = cmd.try_get_matches_from(["app", "help"]).unwrap();
        // "help" subcommand is excluded from has_subcommand check
        // because standout handles help separately
        assert!(!has_subcommand(&matches));
    }

    #[test]
    fn test_insert_default_command_basic() {
        let args = vec!["myapp", "-v"];
        let result = insert_default_command(args, "list");
        assert_eq!(result, vec!["myapp", "list", "-v"]);
    }

    #[test]
    fn test_insert_default_command_no_args() {
        let args = vec!["myapp"];
        let result = insert_default_command(args, "list");
        assert_eq!(result, vec!["myapp", "list"]);
    }

    #[test]
    fn test_insert_default_command_empty() {
        let args: Vec<String> = vec![];
        let result = insert_default_command(args, "list");
        assert_eq!(result, vec!["list"]);
    }

    #[test]
    fn test_insert_default_command_with_options() {
        let args = vec!["myapp", "--verbose", "--output", "json"];
        let result = insert_default_command(args, "status");
        assert_eq!(
            result,
            vec!["myapp", "status", "--verbose", "--output", "json"]
        );
    }

    #[test]
    fn test_insert_default_command_with_positional() {
        let args = vec!["myapp", "file.txt"];
        let result = insert_default_command(args, "cat");
        assert_eq!(result, vec!["myapp", "cat", "file.txt"]);
    }
}
