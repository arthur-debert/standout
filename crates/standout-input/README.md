# standout-input

Declarative input collection for CLI applications.

## Overview

`standout-input` provides a unified way to acquire user input from multiple sources—CLI arguments, stdin, environment variables, editors, and interactive prompts—with automatic fallback chains.

```rust
use standout_input::{InputChain, ArgSource, StdinSource};

// Try argument first, then piped stdin, then default
let message = InputChain::<String>::new()
    .try_source(ArgSource::new("message"))
    .try_source(StdinSource::new())
    .default("default message".to_string())
    .resolve(&matches)?;
```

## Features

- **Declarative chains** - Define fallback sequences without imperative logic
- **Pluggable sources** - Arg, stdin, env, clipboard, editor, prompts
- **Validation** - Chain-level and source-level validation with retry support
- **Testable** - All sources accept mock implementations
- **Minimal deps** - Core has ~2 dependencies; heavy features are opt-in

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `editor` | Yes | Editor-based input via `$EDITOR` |
| `simple-prompts` | Yes | Basic terminal prompts |
| `inquire` | No | Rich TUI prompts via inquire crate |

## Usage

### Basic Chain

```rust
use standout_input::{InputChain, ArgSource, StdinSource, DefaultSource};

let chain = InputChain::<String>::new()
    .try_source(ArgSource::new("message"))
    .try_source(StdinSource::new())
    .try_source(DefaultSource::new("default".to_string()));

let value = chain.resolve(&matches)?;
```

### With Validation

```rust
let chain = InputChain::<String>::new()
    .try_source(ArgSource::new("email"))
    .validate(|s| s.contains('@'), "Must be a valid email");
```

### Testing with Mocks

```rust
use standout_input::{StdinSource, MockStdin};

// Simulate piped input
let source = StdinSource::with_reader(MockStdin::piped("test content"));

// Simulate terminal (no piped input)
let source = StdinSource::with_reader(MockStdin::terminal());
```

## Available Sources

| Source | Type | Description |
|--------|------|-------------|
| `ArgSource` | `String` | CLI argument |
| `FlagSource` | `bool` | CLI flag |
| `StdinSource` | `String` | Piped stdin |
| `EnvSource` | `String` | Environment variable |
| `ClipboardSource` | `String` | System clipboard |
| `DefaultSource<T>` | `T` | Fallback value |

## License

MIT
