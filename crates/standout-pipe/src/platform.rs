use crate::pipe::SimplePipe;

pub fn clipboard() -> Option<SimplePipe> {
    if cfg!(target_os = "macos") {
        Some(SimplePipe::new("pbcopy").consume())
    } else if cfg!(target_os = "linux") {
        // Try xclip, then xsel? Or just xclip as per proposal.
        Some(SimplePipe::new("xclip -selection clipboard").consume())
    } else {
        None
    }
}
