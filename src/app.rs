//! Application lifecycle: register, activate, and present the main window.

use gtk4::prelude::*;
use gtk4::{glib, Application};

use crate::config;
use crate::window::BrowserWindow;

pub fn run() -> glib::ExitCode {
    let app = Application::builder()
        .application_id(config::APP_ID)
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    // Open additional URLs in new tabs (optional future feature).
    app.connect_open(|app, files, _hint| {
        if let Some(window) = app.active_window() {
            let _ = window.present();
        }
        for _file in files {
            // Future: route URI into a new tab via the BrowserWindow.
        }
    });

    app.connect_activate(|app| {
        // Try to re-use an existing window if one is already up.
        if let Some(win) = app.active_window() {
            win.present();
            return;
        }
        let bw = BrowserWindow::new(app);
        // Window must be presented before WebViews load HTML so they are realized.
        bw.window.present();
        bw.add_tab(true);
        // Leak the Rc so that the BrowserWindow (and its Tabs) outlive the
        // activate closure. The single window lives for the entire app lifetime.
        std::mem::forget(bw);
    });

    app.run()
}
