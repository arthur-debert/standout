# Outstanding Architecture & Design

This document describes the architecture, design decisions, and core primitives of the Outstanding framework. It is intended for developers evaluating the framework for adoption, contributors, and advanced users who need to understand the lower-level primitives.

## Overview

Outstanding is a CLI output framework that decouples application logic from terminal presentation. The architecture follows a clear separation:

```
Command Logic → Structured Data → Template + Theme → Terminal Output
                     ↓
              (OutputMode::Json)
                     ↓
              Structured Output (JSON)
```

The framework consists of two crates:
- **`outstanding`**: Core rendering engine (CLI-agnostic)
- **`outstanding-clap`**: Clap integration with command dispatch

---

## Design Principles

### 1. Logic/Presentation Separation

Commands produce structured data. Templates handle presentation. This enables:
- **Testability**: Logic returns data, not strings with ANSI codes
- **Multiple output modes**: Same data renders as terminal, plain text, or JSON
- **Maintainability**: Change styling without touching logic

### 2. Partial Adoption

The framework doesn't force all-or-nothing adoption:
- Register only specific commands with handlers
- Unregistered commands fall through for manual handling
- Use only the features you need (themes, topics, dispatch, etc.)

### 3. CLI-Agnostic Core

The `outstanding` crate has no opinion about argument parsing. It provides:
- Template rendering with style injection
- Theme management
- Output mode control
- Help topics system

The `outstanding-clap` crate adds argument parsing integration.

---

## Core Primitives

### OutputMode

Controls how output is rendered:

```rust
pub enum OutputMode {
    Auto,       // Detect terminal capabilities
    Term,       // Force ANSI escape codes
    Text,       // Force plain text (no ANSI)
    TermDebug,  // Bracket tags: [style]text[/style]
    Json,       // Serialize data directly (skip template)
}
```

**Key methods:**
- `should_use_color()`: Resolves mode to concrete color decision
- `is_structured()`: Returns `true` for JSON (and future formats like YAML)
- `is_debug()`: Returns `true` for TermDebug

**Design decision**: JSON mode bypasses template rendering entirely. The template is ignored and data is serialized directly. This ensures machine-readable output is always valid JSON, not a template that happens to output JSON-like text.

### Theme & Styles

Themes are named collections of `console::Style` values:

```rust
let theme = Theme::new()
    .add("title", Style::new().bold())
    .add("error", Style::new().red());
```

**Style Aliasing**: Styles can reference other styles, enabling layered architecture:

```rust
let theme = Theme::new()
    // Visual layer (concrete styles)
    .add("muted", Style::new().dim())
    .add("accent", Style::new().cyan().bold())
    // Presentation layer (aliases)
    .add("disabled", "muted")
    // Semantic layer (aliases to presentation)
    .add("timestamp", "disabled");
```

**Validation**: Aliases are validated at render time. Dangling references and cycles cause errors before any output is produced.

### AdaptiveTheme

Pairs light and dark themes with automatic OS detection:

```rust
let adaptive = AdaptiveTheme::new(light_theme, dark_theme);
// Uses dark-light crate to detect OS preference
```

### Rendering Functions

Three levels of rendering:

```rust
// 1. Simple: auto-detect colors
render(template, data, theme) -> Result<String>

// 2. Explicit mode control
render_with_output(template, data, theme, mode) -> Result<String>

// 3. Structured output support
render_or_serialize(template, data, theme, mode) -> Result<String>
```

**`render_or_serialize`** is the recommended function for command handlers. It:
- Uses the template for terminal modes
- Serializes directly for structured modes (JSON)

### Renderer

Pre-compiles templates for repeated use:

```rust
let mut renderer = Renderer::new(theme)?;
renderer.add_template("list", "{% for item in items %}...")?;
renderer.render("list", &data)?;
```

**Validation**: The `Renderer::new()` constructor validates all style aliases upfront. Invalid themes fail early, not at render time.

---

## Clap Integration Architecture

### Command Handler System

The clap adapter provides a declarative command registration system:

```rust
Outstanding::builder()
    .command("list", handler_fn, "{{ items | join(', ') }}")
    .command("config.get", handler_fn, "{{ key }}: {{ value }}")
    .run_and_print(cmd, args);
```

**Dot notation**: Command paths use dots for nesting. `"config.get"` matches `app config get`.

### Handler Trait

```rust
pub trait Handler: Send + Sync {
    type Output: Serialize;
    fn handle(&self, matches: &ArgMatches, ctx: &CommandContext)
        -> CommandResult<Self::Output>;
}
```

**Closures**: A `FnHandler` wrapper enables closure syntax:

```rust
.command("list", |matches, ctx| {
    CommandResult::Ok(ListOutput { items: vec![] })
}, template)
```

**Design decision**: We use a wrapper struct (`FnHandler`) instead of a blanket impl for closures. This avoids Higher-Ranked Trait Bound (HRTB) issues with Rust's type system.

### CommandResult

```rust
pub enum CommandResult<T: Serialize> {
    Ok(T),                    // Success with data to render
    Err(anyhow::Error),       // Error (displayed to user)
    Silent,                   // No output
    Archive(Vec<u8>, String), // Binary output (bytes, filename)
}
```

**Design decision**: `anyhow::Error` provides rich error context with backtraces. The `Archive` variant supports binary exports (PDFs, images, etc.) that should not go through template rendering.

### CommandContext

```rust
pub struct CommandContext {
    pub output_mode: OutputMode,
    pub command_path: Vec<String>,
}
```

Passed to every handler, providing execution environment information.

### RunResult

```rust
pub enum RunResult {
    Handled(String),           // Command processed, here's the output
    Binary(Vec<u8>, String),   // Binary output (bytes, filename)
    Unhandled(ArgMatches),     // No handler matched, manual handling needed
}
```

**Fallthrough semantics**: `Unhandled` enables partial adoption. Register some commands, handle others manually.

### Dispatch Methods

Three levels of dispatch:

```rust
// 1. Low-level: you provide parsed matches and mode
builder.dispatch(matches, output_mode) -> RunResult

// 2. Convenience: parses args, extracts mode from --output flag
builder.dispatch_from(cmd, args) -> RunResult

// 3. Complete: parse, dispatch, and print output
builder.run_and_print(cmd, args) -> bool
```

---

## Output Flag Integration

The `--output` flag is automatically injected:

```
--output=<auto|term|text|term-debug|json>
```

- `auto`: Detect terminal capabilities (default)
- `term`: Force ANSI codes
- `text`: Plain text (honors `--no-color` conventions)
- `term-debug`: Bracket tags for debugging templates
- `json`: Machine-readable JSON output

**Design decision**: The flag is enabled by default but can be disabled with `.no_output_flag()` or renamed with `.output_flag(Some("format"))`.

---

## Help Topics System

Extended documentation beyond `--help`:

```rust
Outstanding::builder()
    .topics_dir("docs/topics")  // Load .txt and .md files
    .add_topic(Topic::new(...)) // Or add programmatically
```

Users access via:
```
app help topics     # List all topics
app help <topic>    # View specific topic
app help --page X   # View with pager
```

---

## Type-Erased Dispatch

Internally, handlers are stored as type-erased functions:

```rust
type DispatchFn = Arc<dyn Fn(&ArgMatches, &CommandContext)
    -> Result<DispatchOutput, String> + Send + Sync>;
```

This enables storing handlers with different output types in the same `HashMap`. The template and serialization happen inside the closure, producing `DispatchOutput`:

```rust
enum DispatchOutput {
    Text(String),
    Binary(Vec<u8>, String),
    Silent,
}
```

---

## Error Handling Strategy

1. **Style validation**: Checked at render time (or `Renderer::new()`). Invalid aliases fail before output.

2. **Template errors**: MiniJinja errors propagate as `minijinja::Error`.

3. **Handler errors**: Return `CommandResult::Err(anyhow::Error)` for rich context.

4. **Parse errors**: Clap errors are converted to `RunResult::Handled(error_string)`.

---

## Thread Safety

All core types are `Send + Sync`:
- `Theme`, `Styles`, `AdaptiveTheme`: Clone + Send + Sync
- `Handler` trait requires `Send + Sync`
- Dispatch functions stored as `Arc<dyn ... + Send + Sync>`

This enables use in async contexts and parallel processing.

---

## Future Considerations

Documented but not yet implemented:

1. **YAML output**: `OutputMode::Yaml` for alternative structured format
2. **Derive macros**: `#[derive(Outstanding)]` for automatic handler registration
3. **Binary stdin detection**: Smart handling of piped binary input
4. **Interactive prompts**: Integration points for user input during execution
