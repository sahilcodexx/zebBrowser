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
        self.webview.connect_load_failed(move |uri, msg| {
            let Some(me) = me_weak2.upgrade() else {
                return true;
            };
            me.loading.set(false);
            me.emit(TabEvent::LoadFailed {
                uri: uri.to_string(),
                message: msg.to_string(),
            });
            true
        });
    }

    pub fn load_new_tab_page(&self) {
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
        let c = std::ffi::CString::new(data).unwrap_or_default();
        unsafe {
            ffi::webkit_web_view_load_uri(self.webview.web_view_ptr(), c.as_ptr());
        }
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
            self.load_new_tab_page();
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
