//! Helper functions for CLI integration.
//!
//! This module contains utility functions used by the App (formerly split
//! between App and AppBuilder).

use crate::setup::SetupError;
use clap::Command;
use standout_dispatch::verify::{verify_handler_args, ExpectedArg};
use std::collections::HashMap;

/// Gets the current terminal width, or None if not available.
pub(crate) fn get_terminal_width() -> Option<usize> {
    terminal_size::terminal_size().map(|(w, _)| w.0 as usize)
}

pub(crate) fn find_subcommand_recursive<'a>(
    cmd: &'a Command,
    keywords: &[&str],
) -> Option<&'a Command> {
    let mut current = cmd;
    for k in keywords {
        if let Some(sub) = find_subcommand(current, k) {
            current = sub;
        } else {
            return None;
        }
    }
    Some(current)
}

pub(crate) fn find_subcommand<'a>(cmd: &'a Command, name: &str) -> Option<&'a Command> {
    cmd.get_subcommands()
        .find(|s| s.get_name() == name || s.get_aliases().any(|a| a == name))
}

pub(crate) fn verify_recursive(
    cmd: &Command,
    expected_args: &HashMap<String, Vec<ExpectedArg>>,
    parent_path: &[&str],
    is_root: bool,
) -> Result<(), SetupError> {
    let mut current_path = parent_path.to_vec();
    if !is_root && !cmd.get_name().is_empty() {
        current_path.push(cmd.get_name());
    }

    // Check current command
    let path_str = current_path.join(".");
    if let Some(expected) = expected_args.get(&path_str) {
        verify_handler_args(cmd, &path_str, expected)?;
    }

    // Check subcommands
    for sub in cmd.get_subcommands() {
        verify_recursive(sub, expected_args, &current_path, false)?;
    }

    Ok(())
}
