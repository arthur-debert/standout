# Consolidated Design: Outstanding Declarative API

## Executive Summary

This proposal merges the **Type-Driven** approach (Proposal 1) and the **Runtime Router** approach (Proposal 2) into a layered architecture.

1. **Core Layer (Runtime Router)**: A flexible, zero-magic router that allows mapping command strings (paths) to handlers and templates. This enables partial adoption, middleware, and dependency injection.
2. **Sugar Layer (Derive Macros)**: An optional, high-level API using `#[derive(Outstanding)]` that generates the router configuration automatically from Clap structs.

This hybrid approach satisfies all requirements:

- **Zero Boilerplate**: Use the macro for 90% of cases.
- **Maximum Flexibility**: Drop down to the router for complex cases (dependency injection, dynamic templates).
- **Partial Adoption**: The router naturally supports "falling through" to manual handling.

---

## 1. Shared Concepts

### Output Modes

We extend `OutputMode` to support structured data exports.

```rust
pub enum OutputMode {
    Auto,       // Detect based on TTY
    Term,       // Force ANSI
    Text,       // Force plain text
    TermDebug,  // [style]tags[/style]
    Json,       // Serialize output using serde_json
    Yaml,       // Serialize output using serde_yaml
}
```

### Command Context

A context object passed to every handler, providing environment details.

```rust
pub struct CommandContext {
    pub output_mode: OutputMode,
    pub path: Vec<String>, // e.g. ["config", "get"]
}
```

---

## 2. The Core Layer: Runtime Router

This layer manages the dispatch lifecycle: `Logic -> Data -> Template -> Output`. It usually consumes `clap` constructs but doesn't require owning them.

### Key Types

**`CommandResult`**: The standardized return type for all handlers.

```rust
pub enum CommandResult<T: Serialize> {
    Ok(T),
    Err(anyhow::Error), // Or String
    Archive(Vec<u8>, String), // For file exports
}
```

**`Handler` Trait**: The interface for logic.

```rust
pub trait Handler: Send + Sync {
    type Output: Serialize;
    fn handle(&self, matches: &ArgMatches, ctx: &CommandContext) -> CommandResult<Self::Output>;
}
```

### The Router API

The declarative builder allows mapping paths (dot-notation) to handlers.

```rust
let app = Outstanding::builder()
    // Explicit mapping
    .command("list", list_handler, "templates/list.j2")
    
    // Closure support
    .command("add", |m, _| { /* ... */ }, "templates/add.j2")
    
    // "Partial Adoption" - if a command isn't matched here, it falls through
    .run(cmd); 
```

---

## 3. The Sugar Layer: Derive Macros

For users who want the "Framework Experience", macros automate the wiring.

### `Runnable` Trait

Couples logic and presentation to the Clap struct.

```rust
pub trait Runnable {
    type Output: Serialize;

    fn run(&self) -> Result<Self::Output>;

    fn template(&self) -> &str {
        // Default conventions can span here, e.g.
        // lowercase_struct_name.j2
        "default.j2"
    }
}
```

### `#[derive(Outstanding)]`

Generates a `register_handlers` method that populates the Core Router.

```rust
#[derive(Subcommand, Outstanding)]
enum Commands {
    // Automatically registers path "list" mapping to ListCmd::run
    List(ListCmd),
    
    // Customization via attributes
    #[outstanding(path = "new", template = "create.j2")]
    Create(CreateCmd),

    // Skip variant (Partial Adoption)
    #[outstanding(skip)]
    Legacy(LegacyCmd),
}
```

---

## 4. Usage Examples

### Scenario A: The "Sugar" Way (Fastest)

```rust
#[derive(Args, Runnable)] 
#[outstanding(template = "list.j2")]
struct ListCmd { /* ... */ }

#[derive(Subcommand, Outstanding)]
enum Commands {
    List(ListCmd),
}

fn main() {
    let cli = Cli::parse();
    
    // The macro generates the router wiring for us
    outstanding::execute(&cli.command).unwrap();
}
```

### Scenario B: The "Core" Way (Dependency Injection)

When you need to pass database pools or services to handlers.

```rust
struct ListHandler {
    db: DatabasePool,
}

impl Handler for ListHandler { /* ... */ }

fn main() {
    let cmd = Command::new("app")...;

    Outstanding::builder()
        .command("list", ListHandler { db: pool }, "list.j2")
        .run(cmd);
}
```

### Scenario C: Mixed / Partial Adoption

```rust
let result = Outstanding::builder()
    .command("modern", modern_handler, "modern.j2")
    .run(cmd); // Returns RunResult

match result {
    RunResult::Handled => {}, // Outstanding took care of it
    RunResult::Unhandled(matches) => {
        // Fallback to legacy spaghetti code
        if let Some(m) = matches.subcommand_matches("legacy") {
            legacy_code(m);
        }
    }
}
```

---

## Implementation Roadmap

1. **Phase 1: Common Types**: Implement `OutputMode` extensions (Json) and `CommandContext` in `crates/outstanding`.
2. **Phase 2: Core Router**: Implement the Builder, `Handler` trait, and Dispatcher in `crates/outstanding-clap`.
3. **Phase 3: Verify**: Ensure the Core Router works for the "Partial Adoption" use case.
4. **Phase 4: Macros**: Implement `outstanding-derive` to generate the `.command(...)` calls automatically for `Runnable` structs.

## Why This Wins

- **Decoupling**: The Router doesn't *force* you to move logic into Structs (unlike Prop 1).
- **Ergonomics**: The Macro *allows* you to put logic in Structs if you prefer that style (like Prop 1).
- **Interop**: You can mix and match. Derive most commands, but manually register the complex ones that need special context.
