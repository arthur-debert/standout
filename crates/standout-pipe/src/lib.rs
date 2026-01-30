pub mod pipe;
pub mod platform;
pub mod shell;

pub use pipe::{PipeError, PipeMode, PipeTarget, SimplePipe};
pub use platform::clipboard;
