use crate::pipe::SimplePipe;

/// Returns a clipboard pipe for macOS (pbcopy).
#[cfg(target_os = "macos")]
pub fn clipboard() -> Option<SimplePipe> {
    Some(SimplePipe::new("pbcopy").consume())
}

/// Returns a clipboard pipe for Linux (xclip).
#[cfg(target_os = "linux")]
pub fn clipboard() -> Option<SimplePipe> {
    Some(SimplePipe::new("xclip -selection clipboard").consume())
}

/// Returns None on unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn clipboard() -> Option<SimplePipe> {
    None
}
