//! {{name}} as a desktop window; the application is in `lib.rs`.
// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> taim::Result {
    {{lib}}::app().run()
}
