//! Child processes the application starts for its own use.

use std::{ffi::OsStr, process::Command};

/// Starts a console program on Windows without a console window of its own.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `program` as a background child. The Windows release app is a GUI program
/// with no console for its children to share, so every console program it
/// starts (`git`, `gh`, `ssh`, `sh`, the `herdr` CLI) would otherwise flash a
/// console window of its own. Nothing the app starts on Windows is meant to
/// show one. Elsewhere this is `Command::new`.
pub fn background_command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    };
    command
}
