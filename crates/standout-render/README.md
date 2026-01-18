# standout-render

Styled terminal rendering with templates, themes, and adaptive color support.

This crate provides the rendering foundation for the `standout` CLI framework, but can be used independently for any application that needs rich terminal output.

## Features

- **Two-pass rendering**: MiniJinja templates + BBCode-style styling
- **Adaptive themes**: Light/dark mode support with automatic OS detection
- **Output modes**: Auto, Terminal, Text, JSON, YAML, CSV, XML
- **Tabular formatting**: Unicode-aware column layouts
- **File-based resources**: Hot-reload in dev, embedded in release

## Quick Start

```rust
use standout_render::{render, Theme};
use console::Style;
use serde::Serialize;

#[derive(Serialize)]
struct Data { title: String, count: usize }

let theme = Theme::new()
    .add("title", Style::new().bold())
    .add("count", Style::new().cyan());

let output = render(
    "[title]{{ title }}[/title]: [count]{{ count }}[/count] items",
    &Data { title: "Report".into(), count: 42 },
    &theme,
).unwrap();
```

## Relationship to `standout`

- `standout-render`: Pure rendering (templates, themes, styles) - no CLI knowledge
- `standout`: Full CLI framework with clap integration, dispatch, hooks

If you only need rendering without CLI features, use `standout-render` directly.
If you want the full framework, use `standout` which re-exports everything from this crate.

## License

MIT
