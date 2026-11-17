//! Internal pipe from the Windows credential vault into a session's PowerShell process.
//! Runs before Tauri's single-instance handler, with no GUI or persistent helper process.
use std::io::{self, Write};
use std::os::windows::io::AsRawHandle;
use traygent_core::github::{docker, KeyringVault};
use windows_sys::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE};

pub(crate) fn handle() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(docker::HELPER_ARG)) {
        return None;
    }
    let output = io::stdout();
    // SAFETY: output owns the standard handle across this read-only query.
    if unsafe { GetFileType(output.as_raw_handle()) } != FILE_TYPE_PIPE {
        return Some(1);
    }
    let id = args.next().and_then(|arg| arg.to_str()?.parse().ok());
    let (Some(id), None, Some(base)) = (id, args.next(), dirs::data_dir()) else {
        return Some(1);
    };
    let dir = base.join(traygent_core::startup::VALUE_NAME);
    let result = docker::session_token(&dir, id, &KeyringVault::new()).and_then(|token| {
        token.with_secret(|s| {
            let mut out = output.lock();
            writeln!(out, "{s}")
                .and_then(|()| out.flush())
                .map_err(|_| docker::UNAVAILABLE.to_string())
        })
    });
    Some(if result.is_ok() { 0 } else { 1 })
}
