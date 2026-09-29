//! Raw FFI declarations for WebKitGTK 6.0.
//!
//! This is a minimal, hand-written binding covering only the surface required
//! by the browser MVP.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

pub type WebKitWebView = c_void;
pub type WebKitSettings = c_void;
pub type WebKitDownload = c_void;
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
    pub fn webkit_web_view_get_favicon(web_view: *mut WebKitWebView) -> *mut c_void;
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
    pub fn webkit_settings_set_user_agent(
        settings: *mut WebKitSettings,
        user_agent: *const c_char,
    );
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
    pub fn g_object_ref(obj: *mut GObject);
    pub fn g_object_unref(obj: *mut GObject);

    // ----- WebKit error quarks -----
    pub fn webkit_network_error_quark() -> u32;
    pub fn webkit_policy_error_quark() -> u32;

    // ----- GLib -----
    pub fn g_signal_connect_data(
        instance: *mut GObject,
        detailed_signal: *const c_char,
        c_handler: Option<unsafe extern "C" fn()>,
        data: *mut c_void,
        destroy_data: Option<unsafe extern "C" fn(*mut c_void)>,
        connect_flags: c_int,
    ) -> c_uint;

    pub fn g_io_error_quark() -> u32;

    // ----- GIO -----
    pub fn g_file_new_for_path(path: *const c_char) -> *mut GObject;

    // ----- User content filters (WebKit content blocker / adblock) -----
    pub fn webkit_web_view_get_user_content_manager(
        web_view: *mut WebKitWebView,
    ) -> *mut c_void;
    pub fn webkit_user_content_manager_add_filter(manager: *mut c_void, filter: *mut c_void);
    pub fn webkit_user_content_manager_add_style_sheet(
        manager: *mut c_void,
        style_sheet: *mut c_void,
    );
    pub fn webkit_user_content_manager_add_script(manager: *mut c_void, script: *mut c_void);
    pub fn webkit_user_script_new(
        source: *const c_char,
        injected_frames: c_int,
        injection_time: c_int,
        allow_list: *const *const c_char,
        block_list: *const *const c_char,
    ) -> *mut c_void;
    pub fn webkit_user_style_sheet_new(
        source: *const c_char,
        injected_frames: c_int,
        level: c_int,
        allow_list: *const *const c_char,
        block_list: *const *const c_char,
    ) -> *mut c_void;
    pub fn webkit_user_content_filter_store_new(path: *const c_char) -> *mut c_void;
    pub fn webkit_user_content_filter_store_load(
        store: *mut c_void,
        identifier: *const c_char,
        cancellable: *mut c_void,
        callback: Option<unsafe extern "C" fn(*mut GObject, *mut c_void, *mut c_void)>,
        user_data: *mut c_void,
    );
    pub fn webkit_user_content_filter_store_load_finish(
        store: *mut c_void,
        result: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut c_void;
    pub fn webkit_user_content_filter_store_save_from_file(
        store: *mut c_void,
        identifier: *const c_char,
        file: *mut GObject,
        cancellable: *mut c_void,
        callback: Option<unsafe extern "C" fn(*mut GObject, *mut c_void, *mut c_void)>,
        user_data: *mut c_void,
    );
    pub fn webkit_user_content_filter_store_save_from_file_finish(
        store: *mut c_void,
        result: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut c_void;
    pub fn webkit_user_content_filter_unref(filter: *mut c_void);

    // ----- Network session / favicon database (WebKitGTK 2.40+) -----
    pub fn webkit_web_view_get_network_session(web_view: *mut WebKitWebView) -> *mut c_void;
    pub fn webkit_network_session_get_website_data_manager(session: *mut c_void) -> *mut c_void;
    pub fn webkit_website_data_manager_set_favicons_enabled(manager: *mut c_void, enabled: c_int);

    // ----- Navigation policy (adblock-rust engine integration) -----
    pub fn webkit_web_view_get_main_resource(web_view: *mut WebKitWebView) -> *mut c_void;
    pub fn webkit_uri_request_get_uri(request: *mut c_void) -> *const c_char;
    pub fn webkit_response_policy_decision_get_request(decision: *mut c_void) -> *mut c_void;
    pub fn webkit_navigation_policy_decision_get_navigation_action(decision: *mut c_void)
        -> *mut c_void;
    pub fn webkit_navigation_action_get_request(action: *mut c_void) -> *mut c_void;
    pub fn webkit_navigation_action_get_navigation_type(action: *mut c_void) -> c_uint;
    pub fn webkit_navigation_action_is_user_gesture(action: *mut c_void) -> i32;
    pub fn webkit_policy_decision_ignore(decision: *mut c_void);
    pub fn webkit_policy_decision_use(decision: *mut c_void);
    pub fn webkit_policy_decision_download(decision: *mut c_void);

    // Response policy (GTK4 has no webkit_response_policy_decision_is_download;
    // "should download" == the MIME type is not renderable in the view).
    pub fn webkit_response_policy_decision_get_response(decision: *mut c_void) -> *mut c_void;
    pub fn webkit_response_policy_decision_is_mime_type_supported(decision: *mut c_void) -> i32;
    pub fn webkit_uri_response_get_mime_type(response: *mut c_void) -> *const c_char;

    // ----- Downloads -----
    pub fn webkit_download_get_destination(download: *mut c_void) -> *const c_char;
    pub fn webkit_download_set_destination(download: *mut c_void, destination: *const c_char);
    pub fn webkit_download_get_received_data_length(download: *mut c_void) -> u64;
    pub fn webkit_download_get_estimated_progress(download: *mut c_void) -> f64;
    pub fn webkit_download_get_response(download: *mut c_void) -> *mut c_void;
    pub fn webkit_download_cancel(download: *mut c_void);
    pub fn webkit_uri_response_get_uri(response: *mut c_void) -> *const c_char;
    pub fn webkit_uri_response_get_suggested_filename(response: *mut c_void) -> *const c_char;
    pub fn webkit_uri_response_get_content_length(response: *mut c_void) -> u64;

    // ----- Find in page (WebKitFindController) -----
    pub fn webkit_web_view_get_find_controller(web_view: *mut WebKitWebView) -> *mut c_void;
    pub fn webkit_find_controller_search(
        controller: *mut c_void,
        search_text: *const c_char,
        find_options: c_uint,
        max_matches: c_uint,
    );
    pub fn webkit_find_controller_search_finish(controller: *mut c_void);
    pub fn webkit_find_controller_search_next(controller: *mut c_void);
    pub fn webkit_find_controller_search_previous(controller: *mut c_void);

    // ----- Zoom -----
    pub fn webkit_web_view_set_zoom_level(web_view: *mut WebKitWebView, zoom_level: f64);
    pub fn webkit_web_view_get_zoom_level(web_view: *mut WebKitWebView) -> f64;

    // ----- Fullscreen (video) -----
    pub fn gtk_window_fullscreen(window: *mut c_void);
    pub fn gtk_window_unfullscreen(window: *mut c_void);
}

/// WEBKIT_FIND_OPTIONS_CASE_INSENSITIVE (value 1); 0 = default options.
pub const FIND_OPTIONS_NONE: c_uint = 0;
pub const FIND_OPTIONS_CASE_INSENSITIVE: c_uint = 1;
/// WEBKIT_FIND_OPTIONS_WRAP_AROUND (value 4).
pub const FIND_OPTIONS_WRAP_AROUND: c_uint = 4;
/// Max matches reported by the find controller.
pub const FIND_MAX_MATCHES: c_uint = 500;

/// WEBKIT_NAVIGATION_TYPE_OTHER — navigations opened by scripts, i.e. popups.
pub const NAVIGATION_TYPE_OTHER: c_uint = 5;

/// WEBKIT_POLICY_DECISION_TYPE_* from WebKitWebView.h (enum values are 0-based).
/// `decide-policy` delivers these as guint.
pub const POLICY_DECISION_TYPE_NAVIGATION_ACTION: c_uint = 0;
pub const POLICY_DECISION_TYPE_NEW_WINDOW_ACTION: c_uint = 1;
pub const POLICY_DECISION_TYPE_RESPONSE: c_uint = 2;
