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

/// A favicon returned from WebKit, downsampled to raw RGBA pixels.
pub struct Favicon {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
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
            // Chrome UA — sites (YouTube in particular) bot-detect unknown
            // engines like raw WebKitGTK and demand login before playing.
            // A mainstream UA keeps content playable without sign-in.
            let ua = CString::new(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            )
            .unwrap_or_default();
            ffi::webkit_settings_set_user_agent(settings, ua.as_ptr());
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
            Self::enable_favicon_database(wv_ptr);
            crate::adblock::attach_to_webview(wv_ptr);
        }
        Self { widget }
    }

    /// WebKitGTK 2.40+ ships with favicons disabled — without enabling the
    /// database on the website data manager, `notify::favicon` never fires.
    unsafe fn enable_favicon_database(wv_ptr: *mut ffi::WebKitWebView) {
        let session = ffi::webkit_web_view_get_network_session(wv_ptr);
        if session.is_null() {
            return;
        }
        let manager = ffi::webkit_network_session_get_website_data_manager(session);
        if manager.is_null() {
            return;
        }
        ffi::webkit_website_data_manager_set_favicons_enabled(manager, 1);
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

    /// The current favicon as raw RGBA pixels (empty if none available yet).
    pub fn favicon(&self) -> Option<Favicon> {
        let paintable = unsafe { ffi::webkit_web_view_get_favicon(self.web_view_ptr()) };
        if paintable.is_null() {
            return None;
        }
        unsafe { Self::snapshot_paintable(paintable) }
    }

    /// Connect to `notify::favicon` — fired whenever a page's icon becomes available.
    pub fn connect_favicon_changed<F>(&self, f: F)
    where
        F: Fn() + 'static,
    {
        let f: Box<F> = Box::new(f);
        let data: *mut F = Box::into_raw(f);
        unsafe {
            ffi::g_signal_connect_data(
                self.widget.as_ptr() as *mut ffi::GObject,
                c"notify::favicon".as_ptr() as *const c_char,
                Some(std::mem::transmute(
                    notify_trampoline::<F> as unsafe extern "C" fn(*mut ffi::GObject, u32, *mut c_void),
                )),
                data as *mut c_void,
                Some(destroy_notify::<F>),
                0,
            );
        }
    }

    /// Snapshot a GdkPaintable into downsampled RGBA bytes (GdkSnapshot + cairo).
    unsafe fn snapshot_paintable(paintable: *mut c_void) -> Option<Favicon> {
        use std::os::raw::c_int;

        #[link(name = "gtk-4")]
        extern "C" {
            fn gdk_paintable_get_intrinsic_width(p: *mut c_void) -> c_int;
            fn gdk_paintable_get_intrinsic_height(p: *mut c_void) -> c_int;
            fn gdk_paintable_snapshot(
                p: *mut c_void,
                snapshot: *mut c_void,
                width: f64,
                height: f64,
            );
            fn gtk_snapshot_new() -> *mut c_void;
            fn gtk_snapshot_to_paintable(snap: *mut c_void) -> *mut c_void;
            fn gdk_texture_get_width(t: *mut c_void) -> c_int;
            fn gdk_texture_get_height(t: *mut c_void) -> c_int;
            fn gdk_texture_download(t: *mut c_void, data: *mut u8, stride: usize);
            fn g_type_from_name(name: *const c_char) -> usize;
            fn g_type_check_instance_is_a(instance: *mut c_void, gtype: usize) -> i32;
        }

        // Only GdkTexture supports direct pixel download; anything else goes
        // through the snapshot fallback below.
        let texture_gtype = g_type_from_name(c"GdkTexture".as_ptr());
        let is_texture = texture_gtype != 0
            && g_type_check_instance_is_a(paintable as *mut c_void, texture_gtype) != 0;

        let w = gdk_paintable_get_intrinsic_width(paintable);
        let h = gdk_paintable_get_intrinsic_height(paintable);
        if w <= 0 || h <= 0 {
            return None;
        }

        // Fast path: paintable is already a texture — download its pixels directly.
        if is_texture {
            let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
            gdk_texture_download(paintable, rgba.as_mut_ptr(), (w as usize) * 4);
            let converted = convert_bgra_to_rgba(rgba);
            return Some(Favicon {
                width: w,
                height: h,
                rgba: converted,
            });
        }

        // Fallback: render into a GtkSnapshot and download the resulting texture.
        let snap = gtk_snapshot_new();
        gdk_paintable_snapshot(paintable, snap, w as f64, h as f64);
        let out = gtk_snapshot_to_paintable(snap);
        // GtkSnapshot is a GObject — release it with g_object_unref.
        ffi::g_object_unref(snap as *mut ffi::GObject);
        let result = if out.is_null() {
            None
        } else {
            let tw = gdk_texture_get_width(out);
            let th = gdk_texture_get_height(out);
            if tw <= 0 || th <= 0 {
                None
            } else {
                let mut rgba = vec![0u8; (tw as usize) * (th as usize) * 4];
                gdk_texture_download(out, rgba.as_mut_ptr(), (tw as usize) * 4);
                let converted = convert_bgra_to_rgba(rgba);
                Some(Favicon {
                    width: tw,
                    height: th,
                    rgba: converted,
                })
            }
        };
        if !out.is_null() {
            ffi::g_object_unref(out as *mut ffi::GObject);
        }
        result
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

    /// Connect to the `load-failed` signal. The callback receives the failing URL,
    /// the GError message and the error domain+code so callers can filter benign
    /// cancellations; return `true` to suppress WebKit's default error page.
    pub fn connect_load_failed<F>(&self, f: F)
    where
        F: Fn(&str, &str, u32, i32) -> bool + 'static,
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
            F: Fn(&str, &str, u32, i32) -> bool + 'static,
        {
            let f: &F = &*(data as *const F);
            let uri_str = if uri.is_null() {
                ""
            } else {
                std::ffi::CStr::from_ptr(uri).to_str().unwrap_or("")
            };
            let (msg_str, domain, code) = if gerror.is_null() {
                ("", 0u32, 0i32)
            } else {
                let err = &*gerror;
                (
                    std::ffi::CStr::from_ptr(err.message).to_str().unwrap_or(""),
                    err.domain,
                    err.code,
                )
            };
            if f(uri_str, msg_str, domain, code) {
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

/// gdk_texture_download delivers cairo ARGB32: premultiplied BGRA, top-down.
/// Convert in place to premultiplied RGBA for GdkMemoryTexture.
fn convert_bgra_to_rgba(mut rgba: Vec<u8>) -> Vec<u8> {
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2); // B <-> R
    }
    rgba
}

unsafe extern "C" fn notify_trampoline<F>(
    _obj: *mut ffi::GObject,
    _pspec: u32,
    data: *mut c_void,
) where
    F: Fn() + 'static,
{
    let f: &F = &*(data as *const F);
    f();
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
