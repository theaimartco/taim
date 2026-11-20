//! Binary entry point for Virgo.
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| {
            let s = p.to_string_lossy();
            !s.starts_with("-psn") && !s.starts_with('-')
        })
        .map(|p| {
            if p.is_file() {
                p.parent().map(|parent| parent.to_path_buf()).unwrap_or(p)
            } else {
                p
            }
        })
        .or_else(|| std::env::current_dir().ok())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    taim_ide::run_with_workspace(workspace)
}
