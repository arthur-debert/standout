# Hooks, inputs, and piping

Use these extension points to keep cross-cutting shell concerns out of handlers.

## Hooks

Hooks run in registration order and stop on the first `HookError`:

- `pre_dispatch(&ArgMatches, &mut CommandContext)` validates, authenticates, or inserts request state before the handler.
- `post_dispatch(&ArgMatches, &CommandContext, RenderData) -> RenderData` transforms handler data before rendering; build replacements with `RenderData::from_serialize(&view)?` to keep formatted fields ([rendering-and-output.md](rendering-and-output.md)).
- `post_output(&ArgMatches, &CommandContext, RenderedOutput)` transforms or observes `Text`, `Binary`, `Artifact`, or `Silent` output after rendering.

Hooks receive the deepest subcommand's matches. Read `ctx.representation()` and `ctx.color_policy()` instead of parsing `--output`.

Attach hooks with `CommandConfig::pre_dispatch`/`post_dispatch`/`post_output` inside `command_with`, `#[dispatch(pre_dispatch(a, b))]` on a variant, or `AppBuilder::hooks(path, Hooks::new()...)`. Register each (command, phase) from only one of those paths; mixing them for the same phase fails the build. Pipes count as `post_output`. Keep reusable behavior in the CLI-free library and use hooks only when the shell concern crosses commands or pipeline phases.

## Declarative inputs

An `InputChain<T>` tries sources in order, validates the resolved value, and resolves before the handler. A questionnaire resolves first, then input chains, then the command's own pre-dispatch hooks, so those hooks can read inputs.

```rust
use standout::cli::{CommandConfig, CommandContextInput};
use standout::input::{ArgSource, InputChain, StdinSource};

pub fn add_inputs<H>(config: CommandConfig<H>) -> CommandConfig<H> {
    config.input(
        "title",
        InputChain::<String>::new()
            .try_source(ArgSource::new("title"))
            .try_source(StdinSource::new())
            .validate(|s: &String| !s.trim().is_empty(), "title cannot be empty"),
    )
}
// #[dispatch(pure, inputs = crate::handlers::add_inputs)] on the variant.
// In the handler: let title: &String = ctx.input("title")?;
```

Sources include `ArgSource`, `FlagSource`, `EnvSource`, `ConfigSource`, `StdinSource`, `ClipboardSource`, `DefaultSource`, editor, and prompt sources (feature-gated). A chain the handler resolves itself must use `chain.resolve_from(matches, ctx.input_sources())`; `resolve(matches)` reads the real process streams and bypasses harness mocks. Stdin, clipboard, and prompt sources bind to the run's `InputSources`; collected outside a chain, `StdinSource::new()` fails with `InputError::StdinNotBound`. A custom `InputCollector` must implement `bind_sources`, returning `None` unless it reads stdin, clipboard, or prompts. Script prompts in tests with `TestHarness::prompts` ([testing.md](testing.md)).

## Piping

Pipes are post-output hooks for text output:

- `.pipe_to(command)` sends plain text to a command and preserves original output.
- `.pipe_through(command)` replaces output with the command's stdout.
- `.pipe_to_clipboard()` consumes output after copying it; it uses `pbcopy` on macOS and `xclip` on Linux and errors elsewhere.

Each has a `_with_timeout` form; the default is 30 seconds. Multiple pipes chain in registration order. Binary, artifact, and silent output pass through unchanged. Commands execute through `sh -c` (`cmd` on Windows), so keep command strings fixed; never interpolate untrusted input.

Inspect `crates/standout-dispatch/src/hooks.rs`, `crates/standout-input/docs/topics/framework-integration.md`, and `crates/standout-pipe/docs/topics/piping.md` for detailed APIs.
