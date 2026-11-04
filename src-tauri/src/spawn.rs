//! Starts a [`LaunchPlan`] as a detached terminal process; Windows only.
use crate::launch::LaunchPlan;

pub const SPAWN_FAILED: &str = "Terminal konnte nicht gestartet werden.";

/// Console plans get their own console window. Stdio stays inherited: the release build has
/// no console, std then leaves `STARTF_USESTDHANDLES` unset and the child uses its new
/// console (UNVERIFIED on Windows; a debug build with a console hands its own handles down).
#[cfg(windows)]
pub fn spawn(plan: &LaunchPlan) -> Result<(), String> {
    use crate::models::TerminalId;
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let flags = match plan.terminal {
        TerminalId::Console => CREATE_NEW_CONSOLE,
        // wt.exe hands the tab to Windows Terminal; no console of its own.
        TerminalId::WindowsTerminal => CREATE_NO_WINDOW,
    };
    std::process::Command::new(&plan.program)
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .creation_flags(flags)
        .spawn()
        .map(drop)
        .map_err(|_| SPAWN_FAILED.into())
}

#[cfg(not(windows))]
pub fn spawn(plan: &LaunchPlan) -> Result<(), String> {
    let _ = plan;
    Err("Nur unter Windows verfügbar.".into())
}
