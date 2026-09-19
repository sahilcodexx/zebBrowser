//! Lightweight native Linux browser.
//!
//! Stack: Rust + GTK4 + WebKitGTK 6.0 (raw FFI).

mod app;
mod config;
mod navigation;
mod tab;
mod webkit_ffi;
mod webview;
mod window;

fn main() -> gtk4::glib::ExitCode {
    app::run()
}
