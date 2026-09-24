//! Safe wrapper around the raw webkitgtk-6.0 FFI.

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

use gtk4::glib::translate::FromGlibPtrFull;
use gtk4::prelude::ObjectType;
use gtk4::Widget;

use crate::webkit_ffi as ffi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadEvent {
    Started,
    Redirected,
    Committed,
    Finished,
}

pub struct WebView {
    widget: Widget,
}

impl WebView {
    pub fn new() -> Self {
        let settings = unsafe { ffi::webkit_settings_new() };
        if settings.is_null() {
            panic!("webkit_settings_new returned null");
        }
        unsafe {
            ffi::webkit_settings_set_enable_javascript(settings, 1);
            ffi::webkit_settings_set_auto_load_images(settings, 1);
            ffi::webkit_settings_set_javascript_can_open_windows_automatically(settings, 1);
            ffi::webkit_settings_set_enable_smooth_scrolling(settings, 1);
            ffi::webkit_settings_set_enable_media(settings, 1);
            ffi::webkit_settings_set_enable_webgl(settings, 1);
            ffi::webkit_settings_set_enable_webaudio(settings, 1);
            ffi::webkit_settings_set_enable_mediasource(settings, 1);
            ffi::webkit_settings_set_enable_media_capabilities(settings, 1);
            ffi::webkit_settings_set_enable_media_stream(settings, 1);
            ffi::webkit_settings_set_media_playback_requires_user_gesture(settings, 0);
            ffi::webkit_settings_set_media_playback_allows_inline(settings, 1);
            ffi::webkit_settings_set_enable_developer_extras(settings, 1);
            ffi::webkit_settings_set_enable_write_console_messages_to_stdout(settings, 1);
        }

        let raw = unsafe { ffi::webkit_web_view_new() };
        if raw.is_null() {
            panic!("webkit_web_view_new returned null");
        }
        let widget = unsafe { Widget::from_glib_full(raw as *mut gtk4::ffi::GtkWidget) };
        let wv_ptr = widget.as_ptr() as *mut ffi::WebKitWebView;
        unsafe {
            ffi::webkit_web_view_set_settings(wv_ptr, settings);
            ffi::g_object_unref(settings);
        }
        Self { widget }
    }

    pub fn as_widget(&self) -> Widget {
        self.widget.clone()
    }

    pub fn web_view_ptr(&self) -> *mut ffi::WebKitWebView {
        self.widget.as_ptr() as *mut ffi::WebKitWebView
    }

    pub fn load_uri(&self, uri: &str) {
        let c = CString::new(uri).expect("uri contained null byte");
        unsafe { ffi::webkit_web_view_load_uri(self.web_view_ptr(), c.as_ptr()) }
    }

    pub fn stop_loading(&self) {
        unsafe { ffi::webkit_web_view_stop_loading(self.web_view_ptr()) }
    }

    pub fn reload(&self) {
        unsafe { ffi::webkit_web_view_reload(self.web_view_ptr()) }
    }

    pub fn go_back(&self) {
        unsafe { ffi::webkit_web_view_go_back(self.web_view_ptr()) }
    }

    pub fn go_forward(&self) {
        unsafe { ffi::webkit_web_view_go_forward(self.web_view_ptr()) }
    }

    pub fn can_go_back(&self) -> bool {
        unsafe { ffi::webkit_web_view_can_go_back(self.web_view_ptr()) != 0 }
    }

    pub fn can_go_forward(&self) -> bool {
        unsafe { ffi::webkit_web_view_can_go_forward(self.web_view_ptr()) != 0 }
    }

    pub fn uri(&self) -> Option<String> {
        let p = unsafe { ffi::webkit_web_view_get_uri(self.web_view_ptr()) };
        if p.is_null() {
            None
        } else {
            let s = unsafe { std::ffi::CStr::from_ptr(p) };
            Some(s.to_string_lossy().into_owned())
        }
    }

    pub fn title(&self) -> Option<String> {
        let p = unsafe { ffi::webkit_web_view_get_title(self.web_view_ptr()) };
        if p.is_null() {
            None
        } else {
            let s = unsafe { std::ffi::CStr::from_ptr(p) };
            Some(s.to_string_lossy().into_owned())
        }
    }

    /// Run JavaScript on the current page.
    pub fn evaluate_javascript(&self, script: &str) {
        let c = match CString::new(script) {
            Ok(s) => s,
            Err(_) => return,
        };
        unsafe {
            ffi::webkit_web_view_evaluate_javascript(
                self.web_view_ptr(),
                c.as_ptr(),
                -1,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut(),
            );
        }
    }

    /// Connect to the `load-changed` signal. Callback receives the load event kind.
    pub fn connect_load_changed<F>(&self, f: F)
    where
        F: Fn(LoadEvent) + 'static,
    {
        let f: Box<F> = Box::new(f);
        let data: *mut F = Box::into_raw(f);

        unsafe extern "C" fn trampoline<F>(
            _web_view: *mut ffi::WebKitWebView,
            event: c_int,
            data: *mut c_void,
        ) where
            F: Fn(LoadEvent) + 'static,
        {
            let f: &F = &*(data as *const F);
            let ev = match event {
                0 => LoadEvent::Started,
                1 => LoadEvent::Redirected,
                2 => LoadEvent::Committed,
                3 => LoadEvent::Finished,
                _ => return,
            };
            f(ev);
        }

        unsafe {
            ffi::g_signal_connect_data(
                self.widget.as_ptr() as *mut ffi::GObject,
                c"load-changed".as_ptr() as *const c_char,
                Some(std::mem::transmute(
                    trampoline::<F>
                        as unsafe extern "C" fn(*mut ffi::WebKitWebView, c_int, *mut c_void),
                )),
                data as *mut c_void,
                Some(destroy_notify::<F>),
                0,
            );
        }
    }

    /// Connect to the `load-failed` signal. The callback receives the failing URL
    /// and a GError message; return `true` to suppress WebKit's default error page.
    pub fn connect_load_failed<F>(&self, f: F)
    where
        F: Fn(&str, &str) -> bool + 'static,
    {
        let f: Box<F> = Box::new(f);
        let data: *mut F = Box::into_raw(f);

        unsafe extern "C" fn trampoline<F>(
            _web_view: *mut ffi::WebKitWebView,
            _event: c_int,
            uri: *const c_char,
            gerror: *mut ffi::GError,
            data: *mut c_void,
        ) -> c_int
        where
            F: Fn(&str, &str) -> bool + 'static,
        {
            let f: &F = &*(data as *const F);
            let uri_str = if uri.is_null() {
                ""
            } else {
                std::ffi::CStr::from_ptr(uri).to_str().unwrap_or("")
            };
            let msg_str = if gerror.is_null() {
                ""
            } else {
                std::ffi::CStr::from_ptr((*gerror).message)
                    .to_str()
                    .unwrap_or("")
            };
            if f(uri_str, msg_str) {
                1
            } else {
                0
            }
        }

        unsafe {
            ffi::g_signal_connect_data(
                self.widget.as_ptr() as *mut ffi::GObject,
                c"load-failed".as_ptr() as *const c_char,
                Some(std::mem::transmute(
                    trampoline::<F>
                        as unsafe extern "C" fn(
                            *mut ffi::WebKitWebView,
                            c_int,
                            *const c_char,
                            *mut ffi::GError,
                            *mut c_void,
                        ) -> c_int,
                )),
                data as *mut c_void,
                Some(destroy_notify::<F>),
                0,
            );
        }
    }
}

unsafe extern "C" fn destroy_notify<F>(data: *mut c_void) {
    if !data.is_null() {
        let _ = Box::from_raw(data as *mut F);
    }
}

impl Default for WebView {
    fn default() -> Self {
        Self::new()
    }
}
