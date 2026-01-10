# Design Proposal: Declarative Command Flow for Outstanding

## Goal

Transform `outstanding-clap` from a help-interceptor into a full-fledged command dispatcher that handles the "Logic -> Data -> Render" lifecycle, while preserving `clap`'s flexibility and allowing partial adoption.

## Core Concept

We introduce a `Runnable` trait (or `Outstanding` trait) that couples a command's *logic* with its *presentation*. A unified execution entry point (`outstanding::execute`) then handles the boilerplate of running the logic, selecting the output mode (JSON/Text/Term), and rendering the result.

This approach requires no changes to `clap` parsing logic and integrates seamlessly with existng `derive` based CLIs.

## The Design

### 1. The `Runnable` Trait

Leaf commands (structs used in clap subcommands) implement this trait to define their behavior.

```rust
pub trait Runnable {
    /// The data type produced by this command.
    /// Must be Serializable for JSON/YAML output modes.
    type Output: serde::Serialize;

    /// The logic to execute. Returns the data to be rendered.
    fn run(&self) -> anyhow::Result<Self::Output>; // Or specific Error type

    /// The template to use for rendering the output.
    /// Can be dynamic based on the command state.
    fn template(&self) -> &str;
}
```

### 2. The `Outstanding` Derive Macro

For Enums (the command tree), a derive macro generates the dispatch logic. It recursively calls `run()` on the active variant.

```rust
#[derive(Subcommand, Outstanding)]
pub enum Commands {
    List(ListCmd),
    #[outstanding(skip)] // Partial adoption: skip variants that don't implement Runnable
    Legacy(LegacyCmd),
}
```

### 3. The Execution Entry Point

The entry point takes the parsed command structure and executes the lifecycle.

```rust
// Logic flow inside outstanding::execute(cmd):
// 1. Detect global flags (e.g., --output=json) from clap matches (or context)
// 2. Call cmd.run() to get Result<Data>
// 3. If Err, render error template/style
// 4. If Ok, determine OutputMode (Auto, Json, Text)
// 5. If JSON, serde_json::to_string(data)
// 6. If Template, render(cmd.template(), data)
```

## Code Samples

### 1. Defining a Command

```rust
#[derive(Args, Debug)]
pub struct ListCmd {
    #[arg(long)]
    all: bool,
}

impl Runnable for ListCmd {
    type Output = Vec<Item>; 

    fn run(&self) -> Result<Self::Output> {
         // Logic: Database calls, filtering, etc.
         let items = db::fetch_all(self.all)?;
         Ok(items)
    }

    fn template(&self) -> &str {
        "items/list.j2" // ID of the template in the registry
    }
}
```

### 2. The Command Tree

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Outstanding)] // Declarative dispatch
enum Commands {
    List(ListCmd),
    Create(CreateCmd),
}
```

### 3. Usage in `main.rs`

```rust
fn main() -> Result<()> {
    // 1. Normal Clap parsing (User retains full control)
    let cli = Cli::parse();
    
    // 2. Initialize Outstanding (Load templates, themes)
    let app = Outstanding::builder()
        .add_template("items/list.j2", "{{ name | style('bold') }}...")
        .build();

    // 3. Execute declarative flow
    // This handles:
    // - Dispatching to list.run()
    // - Catching errors
    // - Checking --output flag (already injected by outstanding-clap)
    // - Rendering the template OR serializing to JSON
    app.execute(&cli.command)?;

    Ok(())
}
```

## Handling Partial Adoption

Users might not want to migrate their entire CLI at once.

**Scenario 1: Mixed Enum**
Use `#[outstanding(skip)]` execution for variants that are handled manually.

```rust
#[derive(Subcommand, Outstanding)]
enum Commands {
    NewStyle(NewCmd),
    #[outstanding(skip)]
    OldStyle(OldCmd),
}

// In main:
match outstanding::execute(&cli.command) {
    Ok(_) => exit(0),
    Err(OutstandingError::Skipped) => {
        // Handle OldStyle manually here
    }
    Err(e) => handle_error(e),
}
```

**Scenario 2: Single Command Usage**
You don't need the Enum derive. You can execute leaf commands directly.

```rust
let cli = Cli::parse();
if let Commands::NewStyle(cmd) = cli.command {
    app.execute(&cmd)?; // Works on single struct
} else {
    // legacy handling
}
```

## Addressing "Direct Interactions"

The user requirement: *"User should be able to interact with the clap command... and still get readable (and writeable) state"*

Since `Runnable::run` takes `&self` (the `clap` struct), the user has full access to the parsed state inside the `run` method.
If the return type `Output` is generic or specific, `outstanding` doesn't hide it; it just consumes it for rendering.

If the user wants to intercept the *data* before rendering (e.g. for logging or extra processing), `execute` could return the data (or a wrapper), or we could offer a middleware hook.

```rust
// Advanced Usage: Intercepting data
let result = cli.command.run()?; // Call logic directly
log_analytics(&result);
app.render(&result, cli.command.template())?; // Just use the rendering part
```

This ensures the user is never "locked in" to the framework. They can pick and choose:

- Just the Router (Dispatch)
- Just the Renderer (Template)
- Or the full Bundle (`app.execute`)

## Proposed API Signatures

**`trait Runnable`**

```rust
trait Runnable {
    type Output: Serialize;
    fn run(&self) -> Result<Self::Output>;
    fn template(&self) -> &str;
}
```

**`trait Outstanding` (Derived)**

```rust
trait Outstanding {
    fn run_and_render(&self, app: &OutstandingApp) -> Result<()>;
}
```

**`struct OutstandingApp` methods**

```rust
impl OutstandingApp {
    /// Execute a runnable command and handle output
    pub fn execute<C: Runnable>(&self, cmd: &C) -> Result<()>;
    
    /// Execute a derived enum tree
    pub fn execute_tree<T: Outstanding>(&self, tree: &T) -> Result<()>;
}
```

## Roadmap for Implementation

1. **Phase 1**: Define `Runnable` trait in `outstanding`.
2. **Phase 2**: Implement `OutstandingApp` with `execute` command in `outstanding-clap`.
3. **Phase 3**: Create `outstanding-derive` crate for the `Outstanding` macro (optional but recommended for UX).
4. **Phase 4**: Update `docs/` and examples.

## Comparison to User Request

- **Declarative API**: Yes (`impl Runnable`, `derive(Outstanding)`).
- **Hooks up Post Logic**: Yes (`run()` -> `template()`).
- **Does not force adoption**: Yes, existing clap structs work as is. Integration is via trait impl.
- **Partial Adoption**: Supports skipped variants or manual execution.
- **Access to Clap Data**: Yes, `run(&self)` operates on the Clap struct itself.
