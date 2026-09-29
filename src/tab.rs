//! A browser tab: owns one WebView, tracks its state, and surfaces
//! events to a callback for the window UI to consume.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::Widget;

use crate::config;
use crate::navigation::{parse, resolve, NavInput};
use crate::webkit_ffi as ffi;
use crate::webview::{LoadEvent, WebView};

#[derive(Debug, Clone)]
pub enum TabEvent {
    UriChanged(String),
    TitleChanged(String),
    LoadingChanged(bool),
    LoadFailed { uri: String, message: String },
    PinnedChanged(bool),
    FaviconReady(FaviconData),
    /// A `target=_blank` link / window.open on this tab wants `uri` opened.
    NewWindowRequested(String),
    /// WebKit asked the webview to go into/out of fullscreen (video player).
    FullscreenChanged(bool),
}

/// Raw RGBA favicon pixels for a tab.
#[derive(Debug, Clone)]
pub struct FaviconData {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

pub struct Tab {
    pub webview: WebView,
    /// Widget actually embedded in the UI.
    pub container: Widget,
    pub title: RefCell<String>,
    pub url: RefCell<String>,
    pub loading: Cell<bool>,
    pub pinned: Cell<bool>,
    on_event: Rc<RefCell<Option<Box<dyn Fn(TabEvent)>>>>,
}

impl Tab {
    /// Create a tab that opens on the given URL.
    pub fn new(home: &str) -> Rc<Self> {
        let webview = WebView::new();
        let container = webview.as_widget();

        let tab = Rc::new(Self {
            webview,
            container,
            title: RefCell::new(String::new()),
            url: RefCell::new(String::new()),
            loading: Cell::new(false),
            pinned: Cell::new(false),
            on_event: Rc::new(RefCell::new(None)),
        });

        tab.connect_signals();
        tab.navigate_to(home);
        tab
    }

    /// Create a blank tab showing the centered-search new-tab page.
    /// The page is loaded lazily once the WebView is parented; call
    /// `load_new_tab_page()` after the widget is in the tree.
    pub fn new_blank() -> Rc<Self> {
        let webview = WebView::new();
        let container = webview.as_widget();

        let tab = Rc::new(Self {
            webview,
            container,
            title: RefCell::new(String::from("New Tab")),
            url: RefCell::new(String::from("about:newtab")),
            loading: Cell::new(false),
            pinned: Cell::new(false),
            on_event: Rc::new(RefCell::new(None)),
        });

        tab.connect_signals();
        tab
    }

    pub fn on_event<F: Fn(TabEvent) + 'static>(&self, f: F) {
        *self.on_event.borrow_mut() = Some(Box::new(f));
    }

    fn emit(&self, ev: TabEvent) {
        if let Some(cb) = self.on_event.borrow().as_ref() {
            cb(ev);
        }
    }

    fn connect_signals(self: &Rc<Self>) {
        let me_weak: Weak<Self> = Rc::downgrade(self);

        self.webview.connect_load_changed(move |ev| {
            let Some(me) = me_weak.upgrade() else { return };
            match ev {
                LoadEvent::Started => {
                    me.loading.set(true);
                    me.emit(TabEvent::LoadingChanged(true));
                }
                LoadEvent::Committed => {
                    let uri = me.webview.uri().unwrap_or_default();
                    *me.url.borrow_mut() = uri.clone();
                    me.emit(TabEvent::UriChanged(uri));
                }
                LoadEvent::Finished => {
                    me.loading.set(false);
                    let raw_title = me.webview.title().unwrap_or_default();
                    let uri = me.webview.uri().unwrap_or_default();
                    // Never expose a data: / about: URI as the visible title.
                    let title = if !raw_title.is_empty() {
                        raw_title
                    } else if uri.is_empty() || uri.starts_with("data:") || uri == "about:blank" {
                        String::new()
                    } else {
                        uri
                    };
                    *me.title.borrow_mut() = title.clone();
                    me.emit(TabEvent::TitleChanged(title));
                    me.emit(TabEvent::LoadingChanged(false));
                }
                LoadEvent::Redirected => {}
            }
        });

        let me_weak2: Weak<Self> = Rc::downgrade(self);
        self.webview.connect_load_failed(move |uri, msg, domain, code| {
            let Some(me) = me_weak2.upgrade() else {
                return true;
            };
            // Benign failures: navigation policy changes and cancellations
            // (including cancellation caused by our own error-page render).
            // Rendering an error page for these causes an infinite loop.
            //
            // The domain is a GQuark, so it must come from the quark functions
            // at runtime — the values are derived from the quark strings, not
            // small integers, and hardcoding them meant nothing ever matched
            // and every interrupted frame showed an error page.
            const G_IO_ERROR_CANCELLED: i32 = 19;
            const WEBKIT_NETWORK_ERROR_CANCELLED: i32 = 3;
            const WEBKIT_POLICY_ERROR: i32 = 2; // POLICY_ERROR_FAILED_ACTION
            const WEBKIT_FRAME_LOAD_INTERRUPTED_BY_POLICY_CHANGE: i32 = 101;
            const WEBKIT_POLICY_USER_CANCELLED: i32 = 102;
            const WEBKIT_POLICY_PLUGIN_FAILED: i32 = 103;

            let (g_io, wk_net, wk_policy) = unsafe {
                (
                    ffi::g_io_error_quark(),
                    ffi::webkit_network_error_quark(),
                    ffi::webkit_policy_error_quark(),
                )
            };

            if domain == g_io && code == G_IO_ERROR_CANCELLED {
                return true; // G_IO_ERROR_CANCELLED ("Operation was cancelled")
            }
            if domain == wk_net && code == WEBKIT_NETWORK_ERROR_CANCELLED {
                return true; // WEBKIT_NETWORK_ERROR_CANCELLED
            }
            // A navigation that turns into a download is reported as the frame
            // load being interrupted (WebKit reuses the "Frame load
            // interrupted" message for USER_CANCELLED, code 102) — expected
            // here, so it must never turn into an error page.
            if domain == wk_policy
                && matches!(
                    code,
                    WEBKIT_FRAME_LOAD_INTERRUPTED_BY_POLICY_CHANGE
                        | WEBKIT_POLICY_USER_CANCELLED
                        | WEBKIT_POLICY_PLUGIN_FAILED
                        | WEBKIT_POLICY_ERROR
                )
            {
                return true;
            }
            // Last-resort guard: any transient cancellation (whatever domain
            // GIO/WebKit filed it under) must never render an error page —
            // that page-load itself cancels the current load and loops forever.
            let lower = msg.to_ascii_lowercase();
            if lower.contains("cancel") {
                return true;
            }
            me.loading.set(false);
            me.emit(TabEvent::LoadFailed {
                uri: uri.to_string(),
                message: msg.to_string(),
            });
            true
        });

        // Real favicon: re-render the row whenever WebKit publishes a new icon.
        let me_weak3: Weak<Self> = Rc::downgrade(self);
        self.webview.connect_favicon_changed(move || {
            let Some(me) = me_weak3.upgrade() else {
                return;
            };
            let Some(fav) = me.webview.favicon() else {
                return;
            };
            me.emit(TabEvent::FaviconReady(FaviconData {
                width: fav.width,
                height: fav.height,
                rgba: fav.rgba,
            }));
        });

        // target=_blank / window.open: surface as an event so the window can
        // open a real tab. Script popups (no user gesture) are suppressed
        // inside webview.rs by our popup blocker.
        let me_weak4: Weak<Self> = Rc::downgrade(self);
        self.webview.connect_create(move |uri| {
            let Some(me) = me_weak4.upgrade() else { return };
            me.emit(TabEvent::NewWindowRequested(uri));
        });

        // Video player fullscreen requests.
        let me_weak5: Weak<Self> = Rc::downgrade(self);
        self.webview.connect_fullscreen_mode(move |active| {
            let Some(me) = me_weak5.upgrade() else { return };
            me.emit(TabEvent::FullscreenChanged(active));
        });
        // NOTE: downloads are wired at the network-session level in window.rs —
        // `download-started` is not a WebKitWebView signal in the GTK4 API.
    }

    // ── Zoom ────────────────────────────────────────────────────────────────

    const ZOOM_STEP: f64 = 1.1;
    const ZOOM_MIN: f64 = 0.3;
    const ZOOM_MAX: f64 = 5.0;

    pub fn zoom_in(&self) {
        let z = (self.webview.zoom_level() * Self::ZOOM_STEP).min(Self::ZOOM_MAX);
        self.webview.set_zoom_level(z);
    }

    pub fn zoom_out(&self) {
        let z = (self.webview.zoom_level() / Self::ZOOM_STEP).max(Self::ZOOM_MIN);
        self.webview.set_zoom_level(z);
    }

    pub fn zoom_reset(&self) {
        self.webview.set_zoom_level(1.0);
    }

    // ── Find in page ────────────────────────────────────────────────────────

    pub fn find(&self, text: &str, forward: bool) {
        self.webview.find_search(text, forward);
    }

    pub fn find_next(&self, text: &str) {
        self.webview.find_next();
        let _ = text;
    }

    pub fn find_prev(&self, text: &str) {
        self.webview.find_prev();
        let _ = text;
    }

    pub fn find_done(&self) {
        self.webview.find_finish();
    }

    /// Current URL for session persistence.
    pub fn session_uri(&self) -> String {
        self.webview
            .uri()
            .unwrap_or_else(|| self.url.borrow().clone())
    }

    pub fn load_new_tab_page(&self, dark: bool) {
        let html = config::NEW_TAB_HTML;
        // Encode the HTML as a data URL; WebKit reliably navigates these.
        let mut data = String::with_capacity(html.len() * 3 + 64);
        data.push_str("data:text/html;charset=utf-8,");
        for b in html.bytes() {
            match b {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~'
                | b'!'
                | b'*'
                | b'\''
                | b'('
                | b')'
                | b';'
                | b'/'
                | b':'
                | b'@'
                | b'&'
                | b'='
                | b'+'
                | b'$'
                | b','
                | b'?' => data.push(b as char),
                _ => data.push_str(&format!("%{:02X}", b)),
            }
        }
        if dark {
            data.push_str("#dark");
        } else {
            data.push_str("#light");
        }
        let c = std::ffi::CString::new(data).unwrap_or_default();
        unsafe {
            ffi::webkit_web_view_load_uri(self.webview.web_view_ptr(), c.as_ptr());
        }
    }

    /// Dynamically update theme on the new-tab page via JS without full-page navigation.
    pub fn update_theme(&self, dark: bool) {
        let js = if dark {
            "document.documentElement.classList.add('dark'); try { window.location.hash = '#dark'; } catch(e){}"
        } else {
            "document.documentElement.classList.remove('dark'); try { window.location.hash = '#light'; } catch(e){}"
        };
        self.webview.evaluate_javascript(js);
    }

    pub fn navigate(&self, input: &str) {
        match parse(input) {
            NavInput::Url(u) => self.navigate_to(&u),
            NavInput::Search(q) => {
                let url = resolve(&NavInput::Search(q));
                self.navigate_to(&url);
            }
        }
    }

    pub fn navigate_to(&self, uri: &str) {
        if uri.is_empty() {
            return;
        }
        // Don't navigate to internal "about:newtab" — instead re-render.
        if uri == "about:newtab" {
            self.load_new_tab_page(false);
            return;
        }
        *self.url.borrow_mut() = uri.to_string();
        self.webview.load_uri(uri);
    }

    pub fn go_back(&self) -> bool {
        if !self.webview.can_go_back() {
            return false;
        }
        self.webview.go_back();
        true
    }

    pub fn go_forward(&self) -> bool {
        if !self.webview.can_go_forward() {
            return false;
        }
        self.webview.go_forward();
        true
    }

    pub fn reload(&self) {
        self.webview.reload();
    }

    pub fn stop(&self) {
        self.webview.stop_loading();
    }

    pub fn set_pinned(&self, pinned: bool) {
        if self.pinned.get() != pinned {
            self.pinned.set(pinned);
            self.emit(TabEvent::PinnedChanged(pinned));
        }
    }

    pub fn toggle_pinned(&self) {
        self.set_pinned(!self.pinned.get());
    }

    pub fn widget(&self) -> &Widget {
        &self.container
    }
}
