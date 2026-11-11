//! A panic hook that makes a failure visible.
//!
//! The suite is a GUI app: on Windows it is built as a GUI-subsystem binary, so it has no
//! console. The default panic hook writes to standard error, which goes nowhere, and the
//! window simply disappears. The user is left with an app that does not start and nothing
//! to look at.
//!
//! So every panic is written to a log file next to the app's settings, and on Windows it is
//! also shown in a message box. The four of the five apps this suite merges already install a
//! hook like this; the suite is where it was missing.

use std::path::PathBuf;

/// The settings folder: the same place the apps keep their preferences.
fn config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h).join(format!(
                "Library/Application Support/{}",
                crate::brand("APP_NAME")
            ))
        })
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(crate::brand("APP_NAME")))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join(crate::brand("APP_SLUG")))
    }
}

/// Where the panic report is written.
pub fn log_file() -> Option<PathBuf> {
    Some(config_dir()?.join("logs").join("iridium-office.log"))
}

/// Show a message in a window. On Windows that is a message box; everywhere else there is
/// nowhere to show one, so the log file is all there is.
#[cfg(target_os = "windows")]
fn show(caption: &str, text: &str) {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }
    // MB_OK | MB_ICONERROR
    const KIND: u32 = 0x0000_0010 | 0x0000_0002;
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let (t, c) = (wide(text), wide(caption));
    // Safety: the pointers are to the local Vecs, which live until the call returns.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), KIND);
    }
    let _ = std::ffi::OsStr::new("");
}

#[cfg(not(target_os = "windows"))]
fn show(_caption: &str, _text: &str) {}

/// Install the hook. Call it before anything else: a panic during start-up is exactly the
/// case this exists for.
pub fn install() {
    std::panic::set_hook(Box::new(|info| {
        let trace = std::backtrace::Backtrace::capture();
        let report = if trace.status() == std::backtrace::BacktraceStatus::Captured {
            format!("internal error: {info}\n{trace}")
        } else {
            format!("internal error: {info}")
        };

        // Standard error first: it is free, and it is where a terminal user looks.
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!("iridium-office: {report}\n"),
        );

        // Then the log file, so the report survives the next start.
        if let Some(path) = log_file() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = writeln!(f, "--- {report}");
            }
        }

        // Then the window, because on Windows nothing else is visible.
        show(
            &format!("{} could not continue", crate::brand("APP_NAME")),
            &report,
        );
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_file_is_under_the_settings_folder() {
        // The path is only known when the environment has a home to put it in.
        if let Some(path) = log_file() {
            assert!(path.ends_with("iridium-office.log"), "{path:?}");
            assert!(path.to_string_lossy().contains("logs"), "{path:?}");
        }
    }
}
