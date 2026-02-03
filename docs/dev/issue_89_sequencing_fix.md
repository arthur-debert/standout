# Issue 89: Builder Sequencing & Late Binding

## The Problem

In previous versions of Standout, the `AppBuilder` had a critical sequencing flaw involving `commands()` and `theme()`.

When `commands()` was called, it would often "finalize" the command handlers immediately or capture the *current* state of the builder to create the dispatch closure. If `theme()` (or `default_theme()` via a registry) was configured *after* `commands()`, the command handlers would capture an empty or default theme, ignoring the later configuration.

This manifested as bugs where styles (like `[header]`) were missing even when the theme was correctly configured in the builder, solely because of the order of method calls.

## The Solution: Late Binding

To solve this robustly without relying on fragile build-phase sequencing, we switched to **Late Binding**.

Instead of the dispatch closure capturing the `Theme` at creation time (build time), we modified the `DispatchFn` signature to accept the `Theme` as a runtime argument.

### Before (Capture)

```rust
// DispatchFn captured the theme via closure environment
type DispatchFn = Box<dyn Fn(ArgMatches, &CommandContext) -> RunResult>;

// Builder
let theme = self.theme.clone(); // Captured here!
let dispatch = move |matches, ctx| {
    // Uses captured 'theme' which might be stale
    render(..., &theme)
};
```

### After (Late Binding)

```rust
// DispatchFn requires Theme to be passed at runtime
type DispatchFn = Box<dyn Fn(ArgMatches, &CommandContext, &Theme) -> RunResult>;

// Builder
let dispatch = move |matches, ctx, theme| {
    // Uses the 'theme' passed at execution time
    render(..., theme)
};

// Execution
fn dispatch(&self, ...) {
    // Theme is fully resolved by the time we dispatch
    let theme = self.resolve_theme(); 
    (self.dispatch_fn)(matches, ctx, &theme);
}
```

## Benefits

1.  **Order Independence**: `commands()` and `theme()` can be called in any order.
2.  **Single Source of Truth**: The theme used is always the one present in the `App` at runtime, not a stale copy.
3.  **Compile-Time Guarantee**: It is impossible to call a handler without providing a theme, eliminating "forgotten theme" bugs.
