//! Raw FFI declarations for WebKitGTK 6.0.
//!
//! This is a minimal, hand-written binding covering only the surface required
//! by the browser MVP.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

pub type WebKitWebView = c_void;
pub type WebKitSettings = c_void;
pub type GObject = c_void;
pub type GtkWidget = c_void;

#[repr(C)]
pub struct GError {
    pub domain: u32,
    pub code: c_int,
    pub message: *mut c_char,
}

extern "C" {
    // ----- WebKitWebView -----
    pub fn webkit_web_view_new() -> *mut GtkWidget;

    pub fn webkit_web_view_load_uri(web_view: *mut WebKitWebView, uri: *const c_char);
    pub fn webkit_web_view_load_html(
        web_view: *mut WebKitWebView,
        contents: *const c_char,
        base_uri: *const c_char,
    );
    pub fn webkit_web_view_load_bytes(
        web_view: *mut WebKitWebView,
        bytes: *mut c_void,
        mime_type: *const c_char,
        encoding: *const c_char,
        base_uri: *const c_char,
    );
    pub fn webkit_web_view_load_plain_text(web_view: *mut WebKitWebView, plain_text: *const c_char);
    pub fn webkit_web_view_stop_loading(web_view: *mut WebKitWebView);
    pub fn webkit_web_view_reload(web_view: *mut WebKitWebView);
    pub fn webkit_web_view_go_back(web_view: *mut WebKitWebView);
    pub fn webkit_web_view_go_forward(web_view: *mut WebKitWebView);
    pub fn webkit_web_view_can_go_back(web_view: *mut WebKitWebView) -> c_int;
    pub fn webkit_web_view_can_go_forward(web_view: *mut WebKitWebView) -> c_int;

    pub fn webkit_web_view_get_uri(web_view: *mut WebKitWebView) -> *const c_char;
    pub fn webkit_web_view_get_title(web_view: *mut WebKitWebView) -> *const c_char;
    pub fn webkit_web_view_set_settings(
        web_view: *mut WebKitWebView,
        settings: *mut WebKitSettings,
    );
    pub fn webkit_web_view_evaluate_javascript(
        web_view: *mut WebKitWebView,
        script: *const c_char,
        length: isize,
        world_name: *const c_char,
        source_uri: *const c_char,
        cancellable: *mut c_void,
        callback: Option<unsafe extern "C" fn()>,
        user_data: *mut c_void,
    );

    // ----- WebKitSettings -----
    pub fn webkit_settings_new() -> *mut WebKitSettings;
    pub fn webkit_settings_set_enable_javascript(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_javascript_can_open_windows_automatically(
        settings: *mut WebKitSettings,
        enabled: c_int,
    );
    pub fn webkit_settings_set_enable_smooth_scrolling(
        settings: *mut WebKitSettings,
        enabled: c_int,
    );
    pub fn webkit_settings_set_auto_load_images(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_enable_media(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_enable_webgl(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_enable_webaudio(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_enable_mediasource(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_enable_media_capabilities(
        settings: *mut WebKitSettings,
        enabled: c_int,
    );
    pub fn webkit_settings_set_enable_media_stream(settings: *mut WebKitSettings, enabled: c_int);
    pub fn webkit_settings_set_media_playback_requires_user_gesture(
        settings: *mut WebKitSettings,
        required: c_int,
    );
    pub fn webkit_settings_set_media_playback_allows_inline(
        settings: *mut WebKitSettings,
        allowed: c_int,
    );
    pub fn webkit_settings_set_enable_developer_extras(
        settings: *mut WebKitSettings,
        enabled: c_int,
    );
    pub fn webkit_settings_set_enable_write_console_messages_to_stdout(
        settings: *mut WebKitSettings,
        enabled: c_int,
    );

    // ----- GObject -----
    pub fn g_object_unref(obj: *mut GObject);

    // ----- GLib -----
    pub fn g_signal_connect_data(
        instance: *mut GObject,
        detailed_signal: *const c_char,
        c_handler: Option<unsafe extern "C" fn()>,
        data: *mut c_void,
        destroy_data: Option<unsafe extern "C" fn(*mut c_void)>,
        connect_flags: c_int,
    ) -> c_uint;
}
