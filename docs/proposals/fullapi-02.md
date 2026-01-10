Outstanding-Clap Declarative API Design

 Problem Statement

 Currently, outstanding-clap only intercepts help commands. Users must write boilerplate to connect:
 clap parsing → logic execution → result → template selection → rendering → output

 The goal is a declarative API that handles this flow with minimal code, while allowing:

- Full access to clap's configuration and state
- Partial adoption (per-command opt-in)
- Partial feature usage (use only what you need)

 Core Design: Command Handlers

 The Key Insight

 The missing piece is a command-to-handler-to-template mapping. Instead of intercepting only help, we intercept command dispatch itself, letting users declare:

 1. What logic runs for a command (handler function)
 2. What template renders the result
 3. What output formats are supported (term, text, json, etc.)

 Architecture Overview

 ┌─────────────────────────────────────────────────────────────────┐
 │  User's Clap Command Definition (unchanged)                     │
 └─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
 ┌─────────────────────────────────────────────────────────────────┐
 │  Outstanding Router                                              │
 │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
 │  │ CommandSpec  │  │ CommandSpec  │  │ (fallthrough)│          │
 │  │ "list"       │  │ "add"        │  │              │          │
 │  └──────────────┘  └──────────────┘  └──────────────┘          │
 └─────────────────────────────────────────────────────────────────┘
                               │
             ┌─────────────────┼─────────────────┐
             ▼                 ▼                 ▼
      ┌────────────┐    ┌────────────┐    ┌────────────┐
      │ Handler fn │    │ Handler fn │    │ Regular    │
      │ → Data     │    │ → Data     │    │ clap flow  │
      └────────────┘    └────────────┘    └────────────┘
             │                 │
             ▼                 ▼
      ┌────────────┐    ┌────────────┐
      │ Template   │    │ Template   │
      │ Renderer   │    │ Renderer   │
      └────────────┘    └────────────┘
             │                 │
             ▼                 ▼
      ┌────────────────────────────────────────┐
      │ Output (term/text/json/debug)          │
      └────────────────────────────────────────┘

 API Design

 1. Handler Trait

 Handlers return structured data. The data type is user-defined but must be Serialize.

 /// Result of a command handler
 pub enum CommandResult<T: Serialize> {
     /// Successful result with data to render
     Ok(T),
     /// Error with message to display
     Err(String),
     /// Exit with no output (e.g., user cancelled)
     Silent,
 }

 /// A command handler extracts data from matches and runs logic
 pub trait Handler: Send + Sync {
     type Output: Serialize;

     fn handle(&self, matches: &ArgMatches, ctx: &CommandContext) -> CommandResult<Self::Output>;
 }

 /// Context passed to handlers
 pub struct CommandContext {
     pub output_mode: OutputMode,
     pub command_path: Vec<String>,  // e.g., ["app", "config", "get"]
 }

 1. Command Specification

 Each command spec binds a handler to templates:

 pub struct CommandSpec<H: Handler> {
     /// Path to match (e.g., "config.get" or just "list")
     path: String,
     /// The handler that produces data
     handler: H,
     /// Template for terminal output
     template: String,
     /// Optional: different template for errors
     error_template: Option<String>,
     /// Supported output formats (default: term, text, json)
     formats: OutputFormats,
 }

 pub struct OutputFormats {
     pub term: bool,      // ANSI terminal
     pub text: bool,      // Plain text
     pub json: bool,      // JSON serialization
     pub debug: bool,     // Bracket tags [style]...[/style]
 }

 1. Router Builder

 The router is built declaratively:

 impl Outstanding {
     /// Register a command handler with inline template
     pub fn command<H: Handler>(
         self,
         path: &str,
         handler: H,
         template: &str
     ) -> Self;

     /// Register with a CommandSpec for full control
     pub fn command_spec<H: Handler>(self, spec: CommandSpec<H>) -> Self;

     /// Register a template file for a command path
     pub fn template_file(self, path: &str, file: &str) -> Self;
 }

 1. Closure-Based Handlers (Ergonomic API)

 For simple cases, closures work directly:

 Outstanding::builder()
     .command("list", |matches, _ctx| {
         let items = fetch_items();
         CommandResult::Ok(items)
     }, "{% for item in items %}{{ item.name }}{% endfor %}")
     .command("add", |matches, _ctx| {
         let name = matches.get_one::<String>("name").unwrap();
         add_item(name)?;
         CommandResult::Ok(AddResult { name: name.clone() })
     }, "Added: {{ name | style(\"success\") }}")
     .run(cmd)

 1. Partial Adoption: Fallthrough

 Commands without handlers fall through to normal clap flow:

 Outstanding::builder()
     // Only "list" is handled by outstanding
     .command("list", list_handler, LIST_TEMPLATE)
     // Everything else returns ArgMatches normally
     .run(cmd)

 Returns:
 pub enum RunResult {
     /// Command was handled by outstanding, output already printed
     Handled,
     /// No handler matched, here are the matches for manual handling
     Unhandled(ArgMatches),
 }

 1. Output Format Handling

 When --output=json is used on a handled command:

 // Internally, Outstanding does:
 match output_mode {
     OutputMode::Json => {
         println!("{}", serde_json::to_string_pretty(&data)?);
     }
     _=> {
         let rendered = render_with_output(template, &data, theme, mode)?;
         println!("{}", rendered);
     }
 }

 This makes any CLI instantly scriptable without user effort.

 Usage Examples

 Example 1: Simple App with Two Commands

 use clap::{Command, Arg};
 use outstanding_clap::{Outstanding, CommandResult};
 use serde::Serialize;

 #[derive(Serialize)]
 struct ListOutput {
     items: Vec<Item>,
     count: usize,
 }

 #[derive(Serialize)]
 struct AddOutput {
     name: String,
     id: u64,
 }

 fn main() {
     let cmd = Command::new("notes")
         .subcommand(Command::new("list"))
         .subcommand(
             Command::new("add")
                 .arg(Arg::new("name").required(true))
         );

     let result = Outstanding::builder()
         .command("list", |_, _| {
             let items = db::list_notes();
             CommandResult::Ok(ListOutput {
                 count: items.len(),
                 items,
             })
         }, include_str!("templates/list.txt"))

         .command("add", |m, _| {
             let name = m.get_one::<String>("name").unwrap();
             let id = db::add_note(name);
             CommandResult::Ok(AddOutput {
                 name: name.clone(),
                 id
             })
         }, r#"Created note "{{ name | style("title") }}" (id: {{ id }})"#)

         .run(cmd);

     // If no handler matched, result is Unhandled(matches)
     if let RunResult::Unhandled(matches) = result {
         // Handle manually or show help
     }
 }

 Example 2: Nested Commands

 Outstanding::builder()
     // Dot notation for nested paths
     .command("config.get", config_get_handler, CONFIG_GET_TEMPLATE)
     .command("config.set", config_set_handler, CONFIG_SET_TEMPLATE)
     .command("config.list", config_list_handler, CONFIG_LIST_TEMPLATE)
     .run(cmd)

 Example 3: Custom Formats Per Command

 Outstanding::builder()
     .command_spec(CommandSpec::new("export")
         .handler(export_handler)
         .template(EXPORT_TEMPLATE)
         .formats(OutputFormats {
             term: true,
             text: true,
             json: true,
             // Also support CSV for this command
             custom: vec![("csv", csv_formatter)],
         }))
     .run(cmd)

 Example 4: Struct-Based Handler

 struct ListHandler {
     db: Arc<Database>,
 }

 impl Handler for ListHandler {
     type Output = ListOutput;

     fn handle(&self, matches: &ArgMatches, ctx: &CommandContext) -> CommandResult<Self::Output> {
         let filter = matches.get_one::<String>("filter");
         let items = self.db.list(filter);
         CommandResult::Ok(ListOutput { items })
     }
 }

 // Usage
 Outstanding::builder()
     .command("list", ListHandler { db: db.clone() }, LIST_TEMPLATE)
     .run(cmd)

 Example 5: Partial Adoption with Mixed Flow

 let result = Outstanding::builder()
     // These commands are "outstanding-ified"
     .command("status", status_handler, STATUS_TEMPLATE)
     .command("list", list_handler, LIST_TEMPLATE)
     // Everything else is manual
     .run(cmd);

 match result {
     RunResult::Handled => {
         // Already printed output, nothing to do
     }
     RunResult::Unhandled(matches) => {
         // Traditional clap handling
         match matches.subcommand() {
             Some(("init", sub)) => init_project(sub),
             Some(("build", sub)) => build_project(sub),
             _ => {}
         }
     }
 }

 Example 6: Using Only Templates (No Handler Registration)

 For users who want template rendering without command interception:

 // After normal clap parsing
 let matches = cmd.get_matches();

 // Use outstanding just for rendering
 let output = outstanding::render_with_output(
     MY_TEMPLATE,
     &my_data,
     ThemeChoice::from(&theme),
     OutputMode::Auto,
 )?;
 println!("{}", output);

 This remains unchanged from current API - no adoption required.

 High-Level API Signatures

 // --- Core Types ---

 pub enum CommandResult<T: Serialize> {
     Ok(T),
     Err(String),
     Silent,
 }

 pub enum RunResult {
     Handled,
     Unhandled(ArgMatches),
 }

 pub struct CommandContext {
     pub output_mode: OutputMode,
     pub command_path: Vec<String>,
 }

 // --- Handler Trait ---

 pub trait Handler: Send + Sync {
     type Output: Serialize;
     fn handle(&self, matches: &ArgMatches, ctx: &CommandContext) -> CommandResult<Self::Output>;
 }

 // Blanket impl for closures
 impl<F, T> Handler for F
 where
     F: Fn(&ArgMatches, &CommandContext) -> CommandResult<T> + Send + Sync,
     T: Serialize,
 {
     type Output = T;
     fn handle(&self, m: &ArgMatches, ctx: &CommandContext) -> CommandResult<T> {
         (self)(m, ctx)
     }
 }

 // --- CommandSpec ---

 pub struct CommandSpec<H: Handler> {
     pub path: String,
     pub handler: H,
     pub template: String,
     pub error_template: Option<String>,
     pub formats: OutputFormats,
 }

 impl<H: Handler> CommandSpec<H> {
     pub fn new(path: &str) -> Self;
     pub fn handler(self, h: H) -> Self;
     pub fn template(self, t: &str) -> Self;
     pub fn error_template(self, t: &str) -> Self;
     pub fn formats(self, f: OutputFormats) -> Self;
 }

 // --- Outstanding Builder ---

 impl OutstandingBuilder {
     pub fn command<H, T>(self, path: &str, handler: H, template: &str) -> Self
     where
         H: Handler<Output = T>,
         T: Serialize;

     pub fn command_spec<H: Handler>(self, spec: CommandSpec<H>) -> Self;

     pub fn template_dir(self, path: &str) -> Self;  // Auto-map path to templates

     pub fn on_error<F>(self, f: F) -> Self          // Global error handler
     where
         F: Fn(&str, &CommandContext) -> String;

     pub fn run(self, cmd: Command) -> RunResult;
 }

 // --- Output Formats ---

 pub struct OutputFormats {
     pub term: bool,
     pub text: bool,
     pub json: bool,
     pub debug: bool,
 }

 impl Default for OutputFormats {
     fn default() -> Self {
         Self { term: true, text: true, json: true, debug: true }
     }
 }

 Extended Output Mode

 Add JSON output to the existing OutputMode:

 pub enum OutputMode {
     Auto,
     Term,
     Text,
     TermDebug,
     Json,        // NEW: serialize data as JSON, skip template
 }

 The --output flag values become: auto, term, text, term-debug, json.

 Template Directory Convention

 Optional: auto-discover templates by command path:

 templates/
   list.txt           → matches "list" command
   config/
     get.txt          → matches "config.get"
     set.txt          → matches "config.set"
   _error.txt         → default error template

 Outstanding::builder()
     .template_dir("templates/")
     .command("list", list_handler)      // Uses templates/list.txt
     .command("config.get", get_handler) // Uses templates/config/get.txt
     .run(cmd)

 Design Principles

 1. Opt-in, not opt-out: Unregistered commands pass through unchanged
 2. Clap stays clap: No wrapping of Command, no hidden state
 3. Composable: Use handlers, templates, themes, output modes independently
 4. Data-first: Handlers produce structured data, formatting is separate
 5. Scriptable by default: JSON output works on any handled command

 Migration Path

 Existing code continues to work:

 // Still works - help-only interception
 Outstanding::run(cmd)

 // Still works - direct rendering
 render_with_output(template, &data, theme, mode)

 New features are additive via .command() calls.

 Implementation Order

 1. Add Json variant to OutputMode
 2. Implement Handler trait and blanket impl for closures
 3. Implement CommandSpec struct
 4. Add command registration to OutstandingBuilder
 5. Implement command dispatch in run()
 6. Add template directory auto-discovery (optional)

 Files to Modify

- crates/outstanding/src/lib.rs: Add OutputMode::Json
- crates/outstanding-clap/src/lib.rs: Core implementation
  - CommandSpec struct
  - Handler trait
  - CommandResult, RunResult enums
  - Builder extensions
  - Dispatch logic

 Verification

 1. Existing tests continue to pass
 2. New integration test: register handlers, verify output
 3. Test partial adoption: some commands handled, others fall through
 4. Test JSON output mode: verify serialization
 5. Test closure-based and struct-based handlers
 6. Example app demonstrating full workflow
