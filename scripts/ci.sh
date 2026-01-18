#!/bin/bash
set -e

echo "=== CI Checks ==="

echo "[1/6] cargo check (all packages)"
cargo check --all --quiet

echo "[2/6] cargo fmt --check"
cargo fmt --all -- --check

echo "[3/6] cargo clippy (all features)"
cargo clippy --all --all-features --quiet -- -D warnings

echo "[4/6] cargo test (default features)"
# Run lib tests for each crate explicitly (avoid --all which can include doctests)
cargo test -p standout --lib --quiet
cargo test -p standout-render --lib --quiet
cargo test -p standout-bbparser --lib --quiet
cargo test -p standout-macros --lib --quiet
# Run doctests for standout (not standout-render, as its docs reference standout::)
cargo test -p standout --doc --quiet
cargo test -p standout-bbparser --doc --quiet

echo "[5/6] cargo test (macros feature)"
cargo test -p standout --features macros --lib --quiet

echo "[6/6] cargo test (clap feature)"
cargo test -p standout --features clap --lib --quiet
cargo test -p standout --features clap --test '*' --quiet

echo "=== All checks passed ==="
