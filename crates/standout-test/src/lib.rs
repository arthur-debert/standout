//! In-process test harness for apps built on the `standout` CLI framework.
//!
//! `TestHarness` bundles the scattered injection seams — environment
//! detectors, env vars, working directory, stdin, clipboard, output mode,
//! and tempdir fixtures — into a single fluent builder, and restores every
//! override when the harness is dropped.
//!
//! # Example
//!
//! ```no_run
//! use standout_test::TestHarness;
//! # fn example(app: &standout::cli::App, cmd: clap::Command) {
//! let result = TestHarness::new()
//!     .env("HOME", "/tmp/fake")
//!     .clipboard("pasted content")
//!     .terminal_width(80)
//!     .piped_stdin("extra input\n")
//!     .no_color()
//!     .fixture("notes/todo.txt", "- buy milk\n")
//!     .run(app, cmd, ["myapp", "notes", "list"]);
//!
//! result.assert_success();
//! result.assert_stdout_contains("buy milk");
//! # }
//! ```
//!
//! # Concurrency
//!
//! The harness mutates process-global state (env vars, cwd, environment
//! detectors, default input readers). Tests that instantiate a
//! `TestHarness` must be annotated `#[serial]` (from the re-exported
//! `serial_test` crate). A `Drop` impl restores every override, including
//! on panic unwind.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Command;
use standout::cli::{App, RunResult};
use standout_input::env::{MockClipboard, MockStdin};
use standout_input::{
    reset_default_clipboard_reader, reset_default_stdin_reader, set_default_clipboard_reader,
    set_default_stdin_reader,
};
use standout_render::{
    reset_environment_detectors, set_color_capability_detector, set_terminal_width_detector,
    set_tty_detector, OutputMode,
};
use tempfile::TempDir;

pub use serial_test::serial;

/// How stdin should appear to handlers during the run.
#[derive(Debug, Clone)]
enum StdinMode {
    /// Leave the real-stdin default in place.
    Inherit,
    /// Simulate piped stdin with the given content.
    Piped(String),
    /// Simulate an interactive terminal (no piped input).
    Interactive,
}

/// Fluent builder for in-process CLI tests.
///
/// See the [crate-level docs](crate) for the usage pattern. The harness
/// installs every override in [`TestHarness::run`] and tears them down on
/// [`Drop`], so a failed assertion never leaks state into the next test.
#[must_use = "TestHarness is inert until you call run(...)"]
pub struct TestHarness {
    env_set: HashMap<String, String>,
    env_remove: Vec<String>,
    cwd: Option<PathBuf>,
    tempdir: Option<TempDir>,
    fixtures: Vec<(PathBuf, Vec<u8>)>,
    terminal_width: Option<Option<usize>>,
    is_tty: Option<bool>,
    color_capable: Option<bool>,
    output_mode: Option<OutputMode>,
    stdin: StdinMode,
    clipboard: Option<String>,
}

impl TestHarness {
    /// Creates an empty harness with no overrides applied.
    pub fn new() -> Self {
        Self {
            env_set: HashMap::new(),
            env_remove: Vec::new(),
            cwd: None,
            tempdir: None,
            fixtures: Vec::new(),
            terminal_width: None,
            is_tty: None,
            color_capable: None,
            output_mode: None,
            stdin: StdinMode::Inherit,
            clipboard: None,
        }
    }

    // --- environment variables ------------------------------------------------

    /// Sets `key=value` as a real environment variable for the duration of
    /// the run. Handlers that use `EnvSource::new` / `std::env::var` will
    /// see it.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_set.insert(key.into(), value.into());
        self
    }

    /// Removes `key` from the real environment for the duration of the run.
    pub fn env_remove(mut self, key: impl Into<String>) -> Self {
        self.env_remove.push(key.into());
        self
    }

    // --- terminal detectors ---------------------------------------------------

    /// Forces the reported terminal width to `cols`.
    pub fn terminal_width(mut self, cols: usize) -> Self {
        self.terminal_width = Some(Some(cols));
        self
    }

    /// Forces terminal-width detection to report "unknown" (as if stdout
    /// is not a TTY).
    pub fn no_terminal_width(mut self) -> Self {
        self.terminal_width = Some(None);
        self
    }

    /// Claims stdout is attached to a TTY.
    pub fn is_tty(mut self) -> Self {
        self.is_tty = Some(true);
        self
    }

    /// Claims stdout is not a TTY (piped, redirected, …).
    pub fn no_tty(mut self) -> Self {
        self.is_tty = Some(false);
        self
    }

    /// Declares that the output target supports ANSI color.
    pub fn with_color(mut self) -> Self {
        self.color_capable = Some(true);
        self
    }

    /// Declares that the output target does not support ANSI color. When
    /// `--output=auto` is used, this forces the `Text` render path.
    pub fn no_color(mut self) -> Self {
        self.color_capable = Some(false);
        self
    }

    // --- explicit output-mode override ---------------------------------------

    /// Forces a specific [`OutputMode`] regardless of the `--output` flag.
    ///
    /// Internally this injects `--output=<mode>` as the last argument when
    /// [`TestHarness::run`] is called.
    pub fn output_mode(mut self, mode: OutputMode) -> Self {
        self.output_mode = Some(mode);
        self
    }

    /// Shortcut for [`output_mode(OutputMode::Text)`](Self::output_mode).
    pub fn text_output(self) -> Self {
        self.output_mode(OutputMode::Text)
    }

    // --- stdin ----------------------------------------------------------------

    /// Simulates piped stdin with `content`. Handlers using
    /// `StdinSource::new()` will see `is_terminal() == false` and read
    /// `content`.
    pub fn piped_stdin(mut self, content: impl Into<String>) -> Self {
        self.stdin = StdinMode::Piped(content.into());
        self
    }

    /// Simulates an interactive terminal for stdin (no piped content).
    pub fn interactive_stdin(mut self) -> Self {
        self.stdin = StdinMode::Interactive;
        self
    }

    // --- clipboard ------------------------------------------------------------

    /// Installs `content` as the mock clipboard. Handlers using
    /// `ClipboardSource::new()` will read it.
    pub fn clipboard(mut self, content: impl Into<String>) -> Self {
        self.clipboard = Some(content.into());
        self
    }

    // --- filesystem -----------------------------------------------------------

    /// Sets the working directory for the run to `path`.
    ///
    /// If not set and any [`fixture`](Self::fixture) is declared, the
    /// harness uses the fixture tempdir as the cwd.
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    /// Declares a file that should exist at `path` (relative to the
    /// fixture tempdir) with the given text `content`.
    ///
    /// The first call to `fixture` creates a fresh `tempfile::TempDir`
    /// which becomes the default cwd. Access it via [`tempdir`](Self::tempdir).
    pub fn fixture(mut self, path: impl AsRef<Path>, content: impl Into<String>) -> Self {
        let path = path.as_ref().to_path_buf();
        self.fixtures.push((path, content.into().into_bytes()));
        self.ensure_tempdir();
        self
    }

    /// Declares a binary fixture file. Same as [`fixture`](Self::fixture)
    /// but takes raw bytes.
    pub fn fixture_bytes(mut self, path: impl AsRef<Path>, content: impl Into<Vec<u8>>) -> Self {
        let path = path.as_ref().to_path_buf();
        self.fixtures.push((path, content.into()));
        self.ensure_tempdir();
        self
    }

    /// Returns the fixture tempdir path if one has been allocated.
    ///
    /// Useful for constructing absolute paths to pass as handler arguments.
    pub fn tempdir(&self) -> Option<&Path> {
        self.tempdir.as_ref().map(|t| t.path())
    }

    fn ensure_tempdir(&mut self) {
        if self.tempdir.is_none() {
            self.tempdir =
                Some(TempDir::new().expect("TestHarness: failed to create tempdir for fixtures"));
        }
    }

    // --- execution ------------------------------------------------------------

    /// Installs every override, runs `app` with the given `cmd` definition
    /// and argv, and returns a [`TestResult`].
    ///
    /// Overrides are torn down when the returned guard held inside the
    /// `TestResult` is dropped. The `TestResult` and the harness share the
    /// same lifetime, so a typical test binds the result and lets it fall
    /// out of scope at the end.
    pub fn run<I, T>(mut self, app: &App, cmd: Command, args: I) -> TestResult
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        // 1. Materialize fixtures + cwd.
        let mut restore = RestoreState::default();

        if let Some(dir) = self.tempdir.as_ref() {
            for (rel, content) in &self.fixtures {
                let abs = dir.path().join(rel);
                if let Some(parent) = abs.parent() {
                    std::fs::create_dir_all(parent)
                        .expect("TestHarness: failed to create fixture parent dir");
                }
                std::fs::write(&abs, content).expect("TestHarness: failed to write fixture file");
            }
        }

        let cwd_target = self
            .cwd
            .clone()
            .or_else(|| self.tempdir.as_ref().map(|d| d.path().to_path_buf()));
        if let Some(target) = cwd_target {
            restore.original_cwd = std::env::current_dir().ok();
            std::env::set_current_dir(&target)
                .expect("TestHarness: failed to change working directory");
        }

        // 2. Env vars. Save originals so we can restore even on panic.
        for (k, v) in &self.env_set {
            restore
                .env_originals
                .insert(k.clone(), std::env::var(k).ok());
            std::env::set_var(k, v);
        }
        for k in &self.env_remove {
            restore
                .env_originals
                .insert(k.clone(), std::env::var(k).ok());
            std::env::remove_var(k);
        }

        // 3. Environment detectors.
        if let Some(w) = self.terminal_width {
            static WIDTH_SLOT: std::sync::OnceLock<std::sync::Mutex<Option<usize>>> =
                std::sync::OnceLock::new();
            let slot = WIDTH_SLOT.get_or_init(|| std::sync::Mutex::new(None));
            *slot.lock().unwrap() = w;
            set_terminal_width_detector(|| {
                *WIDTH_SLOT
                    .get()
                    .expect("width slot initialized above")
                    .lock()
                    .unwrap()
            });
            restore.reset_env_detectors = true;
        }
        if let Some(flag) = self.is_tty {
            static TTY_SLOT: std::sync::OnceLock<std::sync::Mutex<bool>> =
                std::sync::OnceLock::new();
            let slot = TTY_SLOT.get_or_init(|| std::sync::Mutex::new(false));
            *slot.lock().unwrap() = flag;
            set_tty_detector(|| {
                *TTY_SLOT
                    .get()
                    .expect("tty slot initialized above")
                    .lock()
                    .unwrap()
            });
            restore.reset_env_detectors = true;
        }
        if let Some(flag) = self.color_capable {
            static COLOR_SLOT: std::sync::OnceLock<std::sync::Mutex<bool>> =
                std::sync::OnceLock::new();
            let slot = COLOR_SLOT.get_or_init(|| std::sync::Mutex::new(false));
            *slot.lock().unwrap() = flag;
            set_color_capability_detector(|| {
                *COLOR_SLOT
                    .get()
                    .expect("color slot initialized above")
                    .lock()
                    .unwrap()
            });
            restore.reset_env_detectors = true;
        }

        // 4. Stdin / clipboard overrides.
        match std::mem::replace(&mut self.stdin, StdinMode::Inherit) {
            StdinMode::Inherit => {}
            StdinMode::Piped(content) => {
                set_default_stdin_reader(Arc::new(MockStdin::piped(content)));
                restore.reset_stdin = true;
            }
            StdinMode::Interactive => {
                set_default_stdin_reader(Arc::new(MockStdin::terminal()));
                restore.reset_stdin = true;
            }
        }
        if let Some(content) = self.clipboard.take() {
            set_default_clipboard_reader(Arc::new(MockClipboard::with_content(content)));
            restore.reset_clipboard = true;
        }

        // 5. Argv: append --output=<mode> if forced.
        let mut argv: Vec<OsString> = args.into_iter().map(|a| a.into()).collect();
        if let Some(mode) = self.output_mode {
            argv.push(format!("--output={}", output_mode_flag(mode)).into());
        }

        let outcome = app.run_to_string(cmd, argv);

        // `self` (and its tempdir) move into TestResult so the fixture dir
        // survives until the test is finished with the result.
        TestResult {
            outcome,
            _tempdir: self.tempdir.take(),
            _restore: restore,
        }
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

fn output_mode_flag(mode: OutputMode) -> &'static str {
    match mode {
        OutputMode::Auto => "auto",
        OutputMode::Term => "term",
        OutputMode::Text => "text",
        OutputMode::TermDebug => "term-debug",
        OutputMode::Json => "json",
        OutputMode::Yaml => "yaml",
        OutputMode::Xml => "xml",
        OutputMode::Csv => "csv",
    }
}

/// Restores process-global state when dropped.
///
/// The harness hands ownership of this to the [`TestResult`] so restoration
/// runs after the test has finished consuming the result (and on panic).
#[derive(Default)]
struct RestoreState {
    env_originals: HashMap<String, Option<String>>,
    original_cwd: Option<PathBuf>,
    reset_env_detectors: bool,
    reset_stdin: bool,
    reset_clipboard: bool,
}

impl Drop for RestoreState {
    fn drop(&mut self) {
        for (k, original) in self.env_originals.drain() {
            match original {
                Some(v) => std::env::set_var(&k, v),
                None => std::env::remove_var(&k),
            }
        }
        if let Some(cwd) = self.original_cwd.take() {
            let _ = std::env::set_current_dir(cwd);
        }
        if self.reset_env_detectors {
            reset_environment_detectors();
        }
        if self.reset_stdin {
            reset_default_stdin_reader();
        }
        if self.reset_clipboard {
            reset_default_clipboard_reader();
        }
    }
}

/// Outcome of a [`TestHarness::run`] invocation.
///
/// Holds the raw [`RunResult`] produced by the app, plus convenience
/// accessors and assertion helpers oriented at text output.
pub struct TestResult {
    outcome: RunResult,
    // Kept alive so fixture files remain readable while the test inspects
    // the result; dropped after restore state is torn down.
    _tempdir: Option<TempDir>,
    _restore: RestoreState,
}

impl TestResult {
    /// Returns the raw [`RunResult`] for cases where the structured
    /// accessors aren't enough.
    pub fn outcome(&self) -> &RunResult {
        &self.outcome
    }

    /// Returns the rendered text output, or `""` for `Silent` / `Binary` /
    /// `NoMatch`.
    pub fn stdout(&self) -> &str {
        match &self.outcome {
            RunResult::Handled(s) => s.as_str(),
            _ => "",
        }
    }

    /// Returns `true` if the run produced text output.
    pub fn is_handled(&self) -> bool {
        matches!(self.outcome, RunResult::Handled(_))
    }

    /// Returns `true` if no handler matched the argv.
    pub fn is_no_match(&self) -> bool {
        matches!(self.outcome, RunResult::NoMatch(_))
    }

    /// If the run produced binary output, returns the bytes and suggested
    /// filename.
    pub fn binary(&self) -> Option<(&[u8], &str)> {
        match &self.outcome {
            RunResult::Binary(bytes, filename) => Some((bytes.as_slice(), filename.as_str())),
            _ => None,
        }
    }

    // --- assertions ----------------------------------------------------------

    /// Panics unless the run ended in `RunResult::Handled` or
    /// `RunResult::Silent` (successful dispatch).
    #[track_caller]
    pub fn assert_success(&self) {
        match &self.outcome {
            RunResult::Handled(_) | RunResult::Silent | RunResult::Binary(_, _) => {}
            RunResult::NoMatch(_) => {
                panic!("expected successful dispatch but no handler matched; stdout was empty")
            }
        }
    }

    /// Panics unless the run ended in `RunResult::NoMatch`.
    #[track_caller]
    pub fn assert_no_match(&self) {
        if !self.is_no_match() {
            panic!(
                "expected no handler match, got: {:?}",
                describe_outcome(&self.outcome)
            );
        }
    }

    /// Panics unless [`stdout`](Self::stdout) contains `needle`.
    #[track_caller]
    pub fn assert_stdout_contains(&self, needle: &str) {
        let out = self.stdout();
        if !out.contains(needle) {
            panic!(
                "stdout did not contain {:?}\n--- stdout ---\n{}\n--------------",
                needle, out
            );
        }
    }

    /// Panics unless [`stdout`](Self::stdout) equals `expected` exactly.
    #[track_caller]
    pub fn assert_stdout_eq(&self, expected: &str) {
        let out = self.stdout();
        if out != expected {
            panic!(
                "stdout mismatch\n--- expected ---\n{}\n--- actual -----\n{}\n----------------",
                expected, out
            );
        }
    }
}

fn describe_outcome(o: &RunResult) -> String {
    match o {
        RunResult::Handled(s) => format!("Handled({:?})", s),
        RunResult::Silent => "Silent".into(),
        RunResult::Binary(b, f) => format!("Binary(len={}, {:?})", b.len(), f),
        RunResult::NoMatch(_) => "NoMatch".into(),
    }
}
