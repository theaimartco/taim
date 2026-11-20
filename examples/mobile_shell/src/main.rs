//! The mobile shell as a desktop window; see the library for the
//! application itself.
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> taim::Result {
    mobile_shell::app().run()
}
