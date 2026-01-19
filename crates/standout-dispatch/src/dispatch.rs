//! Command dispatch logic.
//!
//! Core utilities for extracting command paths from clap ArgMatches
//! and managing the dispatch pipeline.

use clap::ArgMatches;

/// Extracts the command path from ArgMatches by following the subcommand chain.
///
/// For example, `myapp db migrate` produces `["db", "migrate"]`.
pub fn extract_command_path(matches: &ArgMatches) -> Vec<String> {
    let mut path = Vec::new();
    let mut current = matches;

    while let Some((name, sub)) = current.subcommand() {
        // Skip "help" as it's handled separately
        if name == "help" {
            break;
        }
        path.push(name.to_string());
        current = sub;
    }

    path
}

/// Gets the deepest subcommand matches.
///
/// Traverses the subcommand chain and returns the ArgMatches
/// for the most deeply nested command.
pub fn get_deepest_matches(matches: &ArgMatches) -> &ArgMatches {
    let mut current = matches;

    while let Some((name, sub)) = current.subcommand() {
        if name == "help" {
            break;
        }
        current = sub;
    }

    current
}

/// Returns true if the matches contain a subcommand (excluding "help").
///
/// Used to detect "naked" CLI invocations where no command was specified.
pub fn has_subcommand(matches: &ArgMatches) -> bool {
    matches
        .subcommand()
        .map(|(name, _)| name != "help")
        .unwrap_or(false)
}

/// Inserts a command name at position 1 (after program name) in the argument list.
///
/// Used to implement default command support.
pub fn insert_default_command<I, S>(args: I, command: &str) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut result: Vec<String> = args.into_iter().map(Into::into).collect();
    if !result.is_empty() {
        result.insert(1, command.to_string());
    } else {
        result.push(command.to_string());
    }
    result
}

/// Converts a command path vector to a dot-separated string.
///
/// For example, `["db", "migrate"]` becomes `"db.migrate"`.
pub fn path_to_string(path: &[String]) -> String {
    path.join(".")
}

/// Parses a dot-separated command path string into a vector.
///
/// For example, `"db.migrate"` becomes `["db", "migrate"]`.
pub fn string_to_path(s: &str) -> Vec<String> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split('.').map(String::from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command;

    #[test]
    fn test_extract_command_path() {
        let cmd =
            Command::new("app").subcommand(Command::new("config").subcommand(Command::new("get")));

        let matches = cmd.try_get_matches_from(["app", "config", "get"]).unwrap();
        let path = extract_command_path(&matches);

        assert_eq!(path, vec!["config", "get"]);
    }

    #[test]
    fn test_extract_command_path_single() {
        let cmd = Command::new("app").subcommand(Command::new("list"));

        let matches = cmd.try_get_matches_from(["app", "list"]).unwrap();
        let path = extract_command_path(&matches);

        assert_eq!(path, vec!["list"]);
    }

    #[test]
    fn test_extract_command_path_empty() {
        let cmd = Command::new("app");

        let matches = cmd.try_get_matches_from(["app"]).unwrap();
        let path = extract_command_path(&matches);

        assert!(path.is_empty());
    }

    #[test]
    fn test_has_subcommand_true() {
        let cmd = Command::new("app").subcommand(Command::new("list"));

        let matches = cmd.try_get_matches_from(["app", "list"]).unwrap();
        assert!(has_subcommand(&matches));
    }

    #[test]
    fn test_has_subcommand_false() {
        let cmd = Command::new("app").subcommand(Command::new("list"));

        let matches = cmd.try_get_matches_from(["app"]).unwrap();
        assert!(!has_subcommand(&matches));
    }

    #[test]
    fn test_has_subcommand_help_excluded() {
        let cmd = Command::new("app")
            .disable_help_subcommand(true)
            .subcommand(Command::new("help"));

        let matches = cmd.try_get_matches_from(["app", "help"]).unwrap();
        assert!(!has_subcommand(&matches));
    }

    #[test]
    fn test_insert_default_command() {
        let args = vec!["myapp", "-v"];
        let result = insert_default_command(args, "list");
        assert_eq!(result, vec!["myapp", "list", "-v"]);
    }

    #[test]
    fn test_insert_default_command_no_args() {
        let args = vec!["myapp"];
        let result = insert_default_command(args, "list");
        assert_eq!(result, vec!["myapp", "list"]);
    }

    #[test]
    fn test_insert_default_command_empty() {
        let args: Vec<String> = vec![];
        let result = insert_default_command(args, "list");
        assert_eq!(result, vec!["list"]);
    }

    #[test]
    fn test_path_to_string() {
        assert_eq!(
            path_to_string(&["db".into(), "migrate".into()]),
            "db.migrate"
        );
        assert_eq!(path_to_string(&["list".into()]), "list");
        assert_eq!(path_to_string(&[]), "");
    }

    #[test]
    fn test_string_to_path() {
        assert_eq!(string_to_path("db.migrate"), vec!["db", "migrate"]);
        assert_eq!(string_to_path("list"), vec!["list"]);
        assert_eq!(string_to_path(""), Vec::<String>::new());
    }
}
