# Project map and API gotchas

## Ownership map

| Location | Responsibility |
| --- | --- |
| `crates/standout` | Framework facade, app builder, clap integration, help/topics |
| `crates/standout-dispatch` | Handler contract, context/state, hooks, dispatch primitives |
| `crates/standout-render` | MiniJinja, styles/themes, output modes, tabular rendering |
| `crates/standout-input` | Input chains, sources, prompt responders, questionnaires |
| `crates/standout-pipe` | Post-output command and clipboard piping |
| `crates/standout-macros` | Handler, dispatch, embedding, tabular, seeker, and questionnaire macros |
| `crates/standout-bbparser` | Semantic style-tag parsing and transformation |
| `crates/standout-seeker` | In-memory typed filtering, ordering, and query parsing |
| `crates/standout-test` | In-process application test harness |
| `crates/standout-types` | `Representation`, `ColorPolicy`, `RenderData`; re-exported from `standout` |
| `crates/todo-example/todo-core` | CLI-free worked library: domain behavior and JSON persistence |
| `crates/todo-example/tdoo` | Binary-only worked CLI: app wiring, adapters, views, assets, and harness tests |

Start at `crates/todo-example/README.md`, then `todo-core/src/lib.rs` (library) and `tdoo/src/app.rs` (CLI
assembly). `docs/SUMMARY.md` indexes guides and topics. Check a claim against the owning crate's public types and tests before copying it.

## Drift checks

Common copied examples can target older APIs. Confirm these current contracts:

- `App::run(...) -> bool`; it does not return `Option<ArgMatches>`. Use `run_with(cmd, args, target, sources)` and match `DispatchResult::NoMatch` on `into_outcome()` when fallback needs matches.
- `CommandContext` has `command_path`, `app_state`, and `extensions`; no `output_mode` field. Hooks read the run's `representation()` and `color_policy()`.
- `--output` takes only `json`, `yaml`, `csv`, `ndjson`, `term-debug`; `term`, `text`, and `auto` are usage errors. Color is `--color`.
- Strings in view data display literally; style tags and ANSI in them are not applied ([rendering-and-output.md](rendering-and-output.md)). Post-dispatch hooks and custom engines take `RenderData`, not JSON.
- Binary handler output is `Output::Binary { data, filename }`, not a tuple variant.
- `#[handler]` preserves the typed function and generates `name__handler`; wire the wrapper and unit-test the original.
- `#[derive(Dispatch)]` maps to `handlers::name`; add `#[dispatch(pure)]` for a `#[handler]`-generated wrapper.
- Structured output bypasses templates, so template fixes cannot change JSON/YAML/CSV/NDJSON. A structured-mode failure is a diagnostic document on stdout ([testing.md](testing.md)).
- Domain serialization and CLI structured output are separate interfaces. Map domain values into CLI-owned view DTOs.
- Embedded paths are resolved at compile time; debug hot reload depends on the original path remaining available.

Do not infer a crate's role from its name. In particular, `standout-seeker` is a query engine, not a file/resource resolver.
