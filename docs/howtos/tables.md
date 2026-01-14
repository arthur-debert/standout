# How To: Format Tables and Columnar Output

Outstanding provides utilities for formatting columnar data with proper alignment, truncation, and Unicode handling.

## Quick Start: Template Filters

For simple tables, use filters directly in templates:

```jinja
{% for entry in entries %}
{{ entry.id | col(8) }}  {{ entry.name | col(20) }}  {{ entry.status | col(10, align='right') }}
{% endfor %}
```

Output:
```
abc123    Alice Johnson         active
def456    Bob Smith            pending
ghi789    Carol Williams       inactive
```

## The col Filter

`col(width, ...)` formats a value for column display:

```jinja
{{ value | col(10) }}                              {# Fixed width, left-aligned #}
{{ value | col(10, align='right') }}               {# Right-aligned #}
{{ value | col(10, align='center') }}              {# Centered #}
{{ value | col(15, truncate='middle') }}           {# Truncate in middle #}
{{ value | col(15, truncate='start') }}            {# Truncate at start #}
{{ value | col(20, ellipsis='...') }}              {# Custom ellipsis #}
```

Arguments:
- `width`: Column width in display columns
- `align`: `'left'` (default), `'right'`, `'center'`
- `truncate`: `'end'` (default), `'start'`, `'middle'`
- `ellipsis`: Truncation indicator (default `'…'`)

## Padding Filters

For simpler cases:

```jinja
{{ value | pad_right(10) }}   {# Left-align: "hello     " #}
{{ value | pad_left(10) }}    {# Right-align: "     hello" #}
{{ value | pad_center(10) }}  {# Center: "  hello   " #}
```

## Truncation Filter

Truncate without column formatting:

```jinja
{{ long_path | truncate_at(30) }}                  {# Truncate at end #}
{{ long_path | truncate_at(30, 'middle') }}        {# Keep start and end #}
{{ long_path | truncate_at(30, 'start', '...') }}  {# Custom ellipsis #}
```

## Display Width

Check visual width (handles Unicode):

```jinja
{% if name | display_width > 20 %}
{{ name | truncate_at(20) }}
{% else %}
{{ name }}
{% endif %}
```

CJK characters count as 2 columns. ANSI codes are ignored.

## FlatDataSpec: Structured Table Layout

For more control, define a table specification:

```rust
use outstanding::table::{FlatDataSpec, Column, Width, Align, TruncateAt};

let spec = FlatDataSpec::builder()
    .column(Column::new(Width::Fixed(8)))
    .column(Column::new(Width::Fixed(20)).align(Align::Left))
    .column(Column::new(Width::Fill))
    .column(Column::new(Width::Fixed(10)).align(Align::Right))
    .separator("  ")
    .build();
```

## Width Variants

```rust
Width::Fixed(10)              // Exactly 10 columns
Width::Bounded { min: 5, max: 20 }  // Auto-size within bounds
Width::Fill                   // Expand to fill remaining space
```

**Fixed**: Always exactly the specified width.

**Bounded**: Calculated from content, clamped to min/max. Without data, uses min.

**Fill**: Takes remaining space after Fixed and Bounded. Multiple Fill columns split evenly.

## Column Configuration

```rust
Column::new(Width::Fixed(12))
    .align(Align::Right)           // Left, Right, Center
    .truncate(TruncateAt::Middle)  // End, Start, Middle
    .ellipsis("...")               // Custom truncation indicator
    .null_repr("N/A")              // For missing values
    .key("author.name")            // JSON path for extraction
    .header("Author")              // CSV header
```

## TableFormatter

Apply a spec to format rows:

```rust
use outstanding::table::TableFormatter;

let formatter = TableFormatter::new(&spec, 80);  // 80 columns total

let row = formatter.format_row(&["abc123", "Alice", "Description", "active"]);
// "abc123    Alice                 Description here...      active"

// Format multiple rows
let rows = formatter.format_rows(&[
    &["id1", "Name 1", "Desc 1", "status1"],
    &["id2", "Name 2", "Desc 2", "status2"],
]);
```

## Width Resolution

Outstanding distributes available width:

1. Calculate overhead (separators, prefix, suffix)
2. Allocate Fixed columns their exact width
3. Allocate Bounded columns (min, or from data if provided)
4. Distribute remaining space to Fill columns
5. If no Fill columns, expand rightmost Bounded

```rust
// Resolve without examining data
let widths = spec.resolve_widths(80);

// Resolve by examining actual content
let widths = spec.resolve_widths_from_data(80, &data_rows);
```

## CSV Integration

FlatDataSpec integrates with CSV output mode:

```rust
use outstanding::{render_auto_with_spec, OutputMode};

let spec = FlatDataSpec::builder()
    .column(Column::new(Width::Fixed(10)).key("name").header("Name"))
    .column(Column::new(Width::Fixed(10)).key("email").header("Email"))
    .column(Column::new(Width::Fixed(10)).key("meta.role").header("Role"))
    .build();

let output = render_auto_with_spec(
    "unused template",
    &data,
    &theme,
    OutputMode::Csv,
    Some(&spec),
)?;
```

The `key` field uses dot notation: `"meta.role"` extracts `data["meta"]["role"]`.

Output:
```csv
Name,Email,Role
Alice,alice@example.com,admin
Bob,bob@example.com,user
```

## Utility Functions

Standalone functions for custom formatting:

```rust
use outstanding::table::{display_width, truncate_end, truncate_middle, pad_left};

// Measure display width (Unicode-aware)
let width = display_width("Hello 日本");  // 10 (ASCII=1, CJK=2)

// Truncate
let short = truncate_end("Hello World", 8, "…");    // "Hello W…"
let short = truncate_middle("abcdefghij", 7, "…");  // "abc…hij"

// Pad
let padded = pad_left("42", 6);  // "    42"
```

All functions handle ANSI escape codes correctly—they're preserved but don't count toward width.

## Unicode Handling

The table system is fully Unicode-aware:

- **CJK characters**: Count as 2 display columns
- **Combining marks**: Count as 0 (combine with previous)
- **ANSI codes**: Preserved but not counted

```rust
display_width("café")    // 4 (é is 1 column)
display_width("日本語")   // 6 (each is 2 columns)

let styled = "\x1b[31mred\x1b[0m";
display_width(styled)    // 3 (just "red")
truncate_end(styled, 2, "…")  // "\x1b[31mr…\x1b[0m" (codes preserved)
```

## Using TableFormatter in Templates

TableFormatter implements MiniJinja's Object trait:

```rust
let formatter = TableFormatter::new(&spec, terminal_width);
// Add to template context as "table"
```

```jinja
{% for entry in entries %}
{{ table.row([entry.id, entry.name, entry.status]) }}
{% endfor %}
```

## Complete Example

```rust
use outstanding::table::{FlatDataSpec, Column, Width, Align, TableFormatter};
use outstanding::{render, Theme};
use console::Style;
use serde::Serialize;

#[derive(Serialize)]
struct Entry {
    id: String,
    name: String,
    path: String,
    status: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let entries = vec![
        Entry { id: "abc123".into(), name: "Alice".into(),
                path: "/very/long/path/to/file.txt".into(), status: "active".into() },
        Entry { id: "def456".into(), name: "Bob".into(),
                path: "/short/path".into(), status: "pending".into() },
    ];

    let spec = FlatDataSpec::builder()
        .column(Column::new(Width::Fixed(8)))
        .column(Column::new(Width::Fixed(10)))
        .column(Column::new(Width::Fill).truncate(TruncateAt::Middle))
        .column(Column::new(Width::Fixed(10)).align(Align::Right))
        .separator(" │ ")
        .build();

    let formatter = TableFormatter::new(&spec, 60);

    // Header
    println!("{}", formatter.format_row(&["ID", "Name", "Path", "Status"]));
    println!("{}", "─".repeat(60));

    // Rows
    for entry in &entries {
        let row = formatter.format_row(&[
            &entry.id, &entry.name, &entry.path, &entry.status
        ]);
        println!("{}", row);
    }

    Ok(())
}
```

Output:
```
ID       │ Name       │ Path                      │     Status
────────────────────────────────────────────────────────────────
abc123   │ Alice      │ /very/lon…/to/file.txt    │     active
def456   │ Bob        │ /short/path               │    pending
```

## Handling Missing Values

Columns have a `null_repr` for missing data:

```rust
Column::new(Width::Fixed(10)).null_repr("N/A")
```

In templates, use Jinja's default filter:

```jinja
{{ value | default("-") | col(10) }}
```
