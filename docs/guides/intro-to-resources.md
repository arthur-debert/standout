# Introduction to the Resource System

This guide walks you through building a CRUD CLI for domain objects. We start with the typical manual approach—lots of repetitive code for list, view, create, update, delete—and progressively transform it into a declarative Resource definition that generates all the boilerplate for you.

The Resource system is built on top of Standout's dispatch and rendering layers. If you're new to Standout, we recommend reading [Introduction to Standout](./intro-to-standout.md) first.

**See Also:**

- [Handler Contract](../crates/dispatch/topics/handler-contract.md) - detailed handler API
- [App Configuration](../topics/app-configuration.md) - full builder options
- [Styling System](../crates/render/topics/styling-system.md) - customizing output
- [Tabular Layout](../crates/render/guides/intro-to-tabular.md) - table displays with `#[derive(Tabular)]`

## The Problem: CRUD Boilerplate

Most CLI applications deal with domain objects—tasks, projects, users, configurations. For each, you need:

- **list**: Show all items (with filtering, sorting, limits)
- **view**: Show details of one or more items
- **create**: Make a new item from CLI arguments
- **update**: Modify an existing item
- **delete**: Remove an item

Writing this by hand means defining clap arguments for each field, parsing them, building JSON payloads, calling your storage layer, and formatting output. Multiply by the number of domain types in your app, and you're looking at thousands of lines of repetitive code.

The Resource system lets you declare your domain type once and generate all of this automatically.

## 1. Start: The Manual Approach

Let's build a task manager CLI the traditional way. We'll define a `Task` type and implement all five CRUD operations manually.

```rust
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Parser)]
#[command(name = "tasks")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all tasks
    List {
        #[arg(long)]
        limit: Option<usize>,
    },
    /// View a task
    View { id: String },
    /// Create a new task
    Create {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "pending")]
        status: String,
    },
    /// Update a task
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Delete a task
    Delete { id: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::List { limit } => list_tasks(limit),
        Commands::View { id } => view_task(&id),
        Commands::Create { title, status } => create_task(&title, &status),
        Commands::Update { id, title, status } => update_task(&id, title, status),
        Commands::Delete { id } => delete_task(&id),
    }
}
```

Each handler function needs to:

1. Call the storage layer
2. Handle errors
3. Format and print output

That's five handlers, each with 10-20 lines. And this is just for one domain type with three fields. Add `Project`, `User`, `Config` types and watch the boilerplate explode.

> **Verify:** Run `cargo build` - it should compile.

## 2. Define Your Domain Type for Resources

Before we can use the Resource macro, we need a clean domain type with the right derives. This is the foundation everything builds on.

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
}
```

Key requirements:

- **`Serialize` + `Deserialize`**: For JSON/YAML output and data exchange with the store
- **`Clone`**: Resource handlers may need to clone items
- **`Debug`**: Helpful for development

> **Verify:** Run `cargo build` - your types should compile.

## 3. Implement the ResourceStore Trait

The Resource system doesn't dictate how you store data—files, databases, APIs—but it needs a common interface. The `ResourceStore` trait provides this abstraction.

```rust
use standout::cli::{ResourceStore, ResourceQuery};

pub struct TaskStore {
    // Your storage implementation (file, database, etc.)
}

impl ResourceStore for TaskStore {
    type Item = Task;
    type Id = String;
    type Error = anyhow::Error;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error> {
        // Validate ID format (e.g., check it's a valid UUID)
        Ok(id_str.to_string())
    }

    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error> {
        // Fetch from storage, return None if not found
        todo!()
    }

    fn not_found_error(id: &Self::Id) -> Self::Error {
        anyhow::anyhow!("Task '{}' not found", id)
    }

    fn list(&self, query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error> {
        // List items, applying filters from query
        let mut items = self.fetch_all()?;
        if let Some(q) = query {
            if let Some(limit) = q.limit {
                items.truncate(limit);
            }
        }
        Ok(items)
    }

    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        // Parse JSON data into Task, assign ID, save
        let task: Task = serde_json::from_value(data)?;
        self.save(&task)?;
        Ok(task)
    }

    fn update(&self, id: &Self::Id, data: serde_json::Value) -> Result<Self::Item, Self::Error> {
        // Fetch existing, merge with data, save
        let mut task = self.resolve(id)?;
        if let Some(title) = data.get("title").and_then(|v| v.as_str()) {
            task.title = title.to_string();
        }
        if let Some(status) = data.get("status").and_then(|v| v.as_str()) {
            task.status = status.to_string();
        }
        self.save(&task)?;
        Ok(task)
    }

    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error> {
        // Remove from storage
        self.remove(id)
    }
}
```

The store receives field values as JSON (from CLI args) and returns domain objects. This keeps the Resource macro agnostic to your storage implementation.

> **Verify:** Run `cargo build` - the store should compile.

### Intermezzo A: Clean Data Layer

**What you achieved:** A clean separation between domain types and storage.

**What's now possible:**

- Unit test your store independently
- Swap storage implementations (file → database → API)
- Reuse the store outside the CLI

**What's next:** Generating the CLI commands automatically.

## 4. Add the Resource Derive Macro

Now the magic. Add `#[derive(Resource)]` and a few attributes, and all five CRUD commands are generated for you.

```rust
use standout_macros::{Resource, Tabular};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Resource, Tabular)]
#[resource(object = "task", store = TaskStore)]
pub struct Task {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    #[tabular(name = "TITLE")]
    pub title: String,

    #[tabular(name = "STATUS")]
    pub status: String,
}
```

That's it. This generates:

- **`TaskCommands`** enum with `List`, `View`, `Create`, `Update`, `Delete` variants
- **CLI arguments** for each field in create/update commands
- **Handler module** `__task_resource_handlers` with all implementations
- **Dispatch configuration** via `TaskCommands::dispatch_config()`

The `#[derive(Tabular)]` adds table formatting for the list view.

> **Verify:** Run `cargo build` - the generated code should compile.

### What Gets Generated

Conceptually, the macro expands to something like this:

```rust
#[derive(Subcommand)]
pub enum TaskCommands {
    /// List all items
    List {
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// View one or more items
    View {
        #[arg(num_args = 1..)]
        ids: Vec<String>,
    },
    /// Create a new item
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        status: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Update an existing item
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete one or more items
    Delete {
        #[arg(num_args = 1..)]
        ids: Vec<String>,
        #[arg(long)]
        confirm: bool,
        #[arg(long)]
        force: bool,
    },
}
```

Plus handler functions that:

1. Extract arguments from CLI
2. Build JSON payloads
3. Call your `ResourceStore`
4. Return properly typed results

## 5. Configure Field Behavior

Field-level attributes customize how each field behaves in CLI commands.

### Mark the ID Field

Every resource needs exactly one ID field:

```rust
#[resource(id)]
pub id: String,
```

The ID field is:

- Excluded from `create` arguments (generated by the store)
- Used as a positional argument in `view`, `update`, `delete`

### Add CLI Arguments

Make fields available as CLI options with `arg()`:

```rust
#[resource(arg(long))]
pub title: String,  // Generates: --title

#[resource(arg(long = "desc"))]
pub description: String,  // Generates: --desc (custom name)
```

The `long` option works exactly like clap's `#[arg]` attribute. By default, the long option name is derived from the field name (with underscores converted to hyphens).

### Set Default Values

Provide defaults for optional fields:

```rust
#[resource(arg(long), default = "pending")]
pub status: String,
```

When `--status` is not provided in `create`, the field gets the default value.

### Exclude Fields from Commands

For fields that shouldn't be user-modifiable:

```rust
#[resource(readonly)]
pub created_at: String,  // Included in output, excluded from create/update args

#[resource(skip)]
pub internal_cache: String,  // Excluded from everything
```

### Help Text from Doc Comments

Doc comments become CLI help text:

```rust
/// The task's title (required)
#[resource(arg(long))]
pub title: String,
```

Generates: `--title <TITLE>  The task's title (required)`

### Full Example

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Resource, Tabular)]
#[resource(object = "task", store = TaskStore)]
pub struct Task {
    #[resource(id)]
    #[tabular(name = "ID")]
    pub id: String,

    /// The task title
    #[resource(arg(long))]
    #[tabular(name = "TITLE")]
    pub title: String,

    /// Current status: pending, active, or done
    #[resource(arg(long), default = "pending")]
    #[tabular(name = "STATUS")]
    pub status: String,

    /// Optional priority (1-5)
    #[resource(arg(long))]
    #[tabular(name = "PRI")]
    pub priority: Option<u8>,

    #[resource(readonly)]
    pub created_at: String,
}
```

> **Verify:** Run `cargo build` - all attributes should compile.

### Intermezzo B: Full CRUD CLI Generated

**What you achieved:** A complete CRUD CLI from a single struct definition.

**What's now possible:**

- `app tasks list --limit 10`
- `app tasks view task-123`
- `app tasks create --title "New task" --status active`
- `app tasks update task-123 --status done`
- `app tasks delete task-123 --force`
- All with proper help text, argument parsing, and error handling

**What's next:** Advanced features for power users.

## 6. Advanced Configuration

### Select Operations

Don't need all five commands? Pick what you want:

```rust
#[resource(object = "config", store = ConfigStore, operations(view, update))]
pub struct Config {
    // Only generates view and update commands
}
```

Valid operations: `list`, `view`, `create`, `update`, `delete`

### Command Aliases

Rename commands to match your preferences:

```rust
#[resource(
    object = "task",
    store = TaskStore,
    aliases(view = "show", delete = "rm")
)]
pub struct Task { ... }
```

Now it's `app tasks show` and `app tasks rm` instead of `view` and `delete`.

To keep the original names as hidden aliases (for backwards compatibility):

```rust
#[resource(
    object = "task",
    store = TaskStore,
    aliases(view = "show", delete = "rm"),
    keep_aliases
)]
```

Both `app tasks show` and `app tasks view` will work.

### Shortcut Commands

Create convenience commands for common updates:

```rust
#[resource(
    object = "task",
    store = TaskStore,
    shortcut(name = "complete", sets(status = "done")),
    shortcut(name = "start", sets(status = "active"))
)]
pub struct Task { ... }
```

This generates:

- `app tasks complete <id>` - sets status to "done"
- `app tasks start <id>` - sets status to "active"

### Enum Fields with ValueEnum

For fields with constrained values, use clap's `ValueEnum`:

```rust
use clap::ValueEnum;

#[derive(Clone, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Active,
    Done,
}

#[derive(Resource)]
#[resource(object = "task", store = TaskStore)]
pub struct Task {
    #[resource(id)]
    pub id: String,

    #[resource(arg(long), value_enum)]
    pub status: Status,  // CLI shows: --status <pending|active|done>
}
```

The `value_enum` flag tells the macro to use clap's enum handling, which provides:

- Tab completion for valid values
- Automatic validation
- Help text showing all options

### Vec Fields for Multiple Values

Accept multiple values for a field:

```rust
#[resource(arg(long))]
pub tags: Vec<String>,  // --tags foo --tags bar
```

The macro detects `Vec<T>` and generates appropriate clap arguments with `num_args`.

### Optional Fields

`Option<T>` fields become optional arguments:

```rust
#[resource(arg(long))]
pub priority: Option<u8>,  // --priority is optional
```

### Validify Integration

For validation and field modifiers, enable validify:

```rust
use standout::Validify;

#[derive(Resource, Validify)]
#[resource(object = "task", store = TaskStore, validify)]
pub struct Task {
    #[resource(id)]
    pub id: String,

    #[modify(trim)]
    #[validate(length(min = 1, max = 100))]
    #[resource(arg(short, long))]
    pub title: String,

    #[modify(trim, lowercase)]
    #[resource(arg(short, long))]
    pub status: String,
}
```

With `validify` enabled:

- `#[modify(trim)]` trims whitespace before validation
- `#[validate(length(...))]` enforces constraints
- Validation errors are returned as proper CLI errors

## 7. Wire Into Your Application

Connect the generated commands to your Standout app:

```rust
use standout::cli::App;
use standout::embed_templates;

fn main() -> anyhow::Result<()> {
    // Create your store
    let store = TaskStore::load()?;

    // Build the app
    let app = App::builder()
        .app_state(store)  // Make store available to handlers
        .templates(embed_templates!("src/templates"))
        .commands(TaskCommands::dispatch_config())  // Register generated handlers
        .build()?;

    // Run with auto-dispatch
    app.run(Cli::command(), std::env::args());
    Ok(())
}
```

The generated handlers access the store via `ctx.app_state.get_required::<TaskStore>()`.

### Custom Templates

By default, Resource uses built-in templates (`standout/list-view`, `standout/detail-view`, etc.). To customize, create your own templates:

```text
src/templates/
├── tasks/
│   ├── list.jinja
│   ├── view.jinja
│   ├── create.jinja
│   ├── update.jinja
│   └── delete.jinja
```

And configure explicitly:

```rust
.commands(|g| {
    g.resource::<Task>(|cfg| {
        cfg.list_template("tasks/list.jinja")
           .view_template("tasks/view.jinja")
    })
})
```

> **Verify:** Run `app tasks list` - you should see formatted output.
> **Verify:** Run `app tasks list --output json` - JSON output works automatically.

### Intermezzo C: Production Ready

**What you achieved:** A fully-featured CRUD CLI from a single struct definition.

**What's now possible:**

- Multiple output modes (terminal, JSON, YAML, CSV)
- Proper error handling and validation
- Help text generated from code
- Batch operations (`view`, `delete` accept multiple IDs)
- Shortcut commands for common workflows
- Easy to extend with more domain types

**Your files now:**

```text
src/
├── main.rs              # App::builder() setup
├── models/
│   └── task.rs          # Task struct with #[derive(Resource)]
├── stores/
│   └── task_store.rs    # ResourceStore implementation
└── templates/
    └── tasks/
        └── list.jinja   # Optional custom templates
```

## Summary

The Resource system transforms this:

```rust
// 200+ lines of boilerplate per domain type
#[derive(Subcommand)]
enum TaskCommands {
    List { ... },
    View { ... },
    Create { ... },
    Update { ... },
    Delete { ... },
}

fn list_tasks(...) { ... }
fn view_task(...) { ... }
fn create_task(...) { ... }
fn update_task(...) { ... }
fn delete_task(...) { ... }
```

Into this:

```rust
// ~20 lines total
#[derive(Resource, Tabular)]
#[resource(object = "task", store = TaskStore)]
pub struct Task {
    #[resource(id)]
    pub id: String,

    #[resource(arg(long))]
    pub title: String,

    #[resource(arg(long), default = "pending")]
    pub status: String,
}
```

To set the default subcommand, configure it at the App level:

```rust
App::builder()
    .default("task")  // Makes `app` equivalent to `app task list`
    .group("task", TaskCommands::dispatch_config())
    .build()?
```

The macro generates type-safe CLI commands, argument parsing, validation, and output formatting—all from your domain type definition.

## Quick Reference

### Container Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `object` | Singular name (required) | `object = "task"` |
| `store` | Store type (required) | `store = TaskStore` |
| `plural` | Plural name | `plural = "tasks"` |
| `operations` | Subset of CRUD | `operations(list, view)` |
| `aliases` | Rename commands | `aliases(view = "show")` |
| `keep_aliases` | Keep original names | `keep_aliases` |
| `shortcut` | Convenience commands | `shortcut(name = "done", sets(status = "done"))` |
| `validify` | Enable validation | `validify` |

### Field Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `id` | Mark as identifier | `#[resource(id)]` |
| `arg(...)` | CLI argument options | `#[resource(arg(long))]` |
| `default` | Default value | `#[resource(default = "pending")]` |
| `readonly` | Exclude from write ops | `#[resource(readonly)]` |
| `skip` | Exclude entirely | `#[resource(skip)]` |
| `value_enum` | Enum with ValueEnum | `#[resource(value_enum)]` |
| `choices` | Constrained values | `#[resource(choices = ["a", "b"])]` |

### ResourceStore Trait

```rust
trait ResourceStore {
    type Item: Serialize + DeserializeOwned;
    type Id: Clone + Display + FromStr;
    type Error: std::error::Error + Send;

    fn parse_id(&self, id_str: &str) -> Result<Self::Id, Self::Error>;
    fn get(&self, id: &Self::Id) -> Result<Option<Self::Item>, Self::Error>;
    fn not_found_error(id: &Self::Id) -> Self::Error;
    fn list(&self, query: Option<&ResourceQuery>) -> Result<Vec<Self::Item>, Self::Error>;
    fn create(&self, data: serde_json::Value) -> Result<Self::Item, Self::Error>;
    fn update(&self, id: &Self::Id, data: serde_json::Value) -> Result<Self::Item, Self::Error>;
    fn delete(&self, id: &Self::Id) -> Result<(), Self::Error>;
}
```
