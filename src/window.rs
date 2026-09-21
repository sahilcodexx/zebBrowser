//! Browser window: Zen-inspired layout with compact top tabs, a minimal
//! address field, and a blank centered-search new-tab page.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::gdk::Key;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, EventControllerKey,
    GestureClick, Label, Orientation, PopoverMenu, ProgressBar, Stack,
};

use crate::config;
use crate::tab::{Tab, TabEvent};
use crate::webkit_ffi as ffi;

/// Commands produced by the key dispatcher that the window handles.
#[derive(Debug, Clone, Copy)]
pub enum WinCmd {
    Back,
    Forward,
    Reload,
    Stop,
    FocusAddress,
    NewTab,
    CloseTab,
    NextTab,
    NextWorkspace,
    PrevTab,
    SwitchTab(i32),
    ToggleSidebar,
    TogglePin,
}

pub struct BrowserWindow {
    pub window: ApplicationWindow,
    pub address: gtk4::Entry,
    pub progress: ProgressBar,
    pub stack: Stack,
    pub tab_strip: GtkBox,
    /// Revealer wrapping the topbar – used for smooth auto-hide slide animation.
    topbar_revealer: gtk4::Revealer,
    /// True while the topbar is in auto-hide mode (real site loaded, not newtab).
    auto_hide: Cell<bool>,
    /// Maps each Tab's WebView widget pointer to its top tab pill.
    tab_rows: RefCell<Vec<Rc<TabRow>>>,
    tabs: RefCell<Vec<Rc<Tab>>>,
    current: RefCell<Option<Rc<Tab>>>,
    self_weak: Weak<Self>,
}

/// One compact tab pill in the top browser chrome.
struct TabRow {
    container: GtkBox,
    title_label: Label,
    close_btn: Button,
    tab: Weak<Tab>,
    #[allow(dead_code)]
    pin_btn: Button,
}

impl BrowserWindow {
    pub fn new(app: &Application) -> Rc<Self> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title(config::APP_NAME)
            .default_width(1180)
            .default_height(760)
            .build();

        window.set_decorated(false);

        // Minimal top chrome: mac-style window dots, small pinned tabs, active tab pill,
        // and a restrained URL field.
        let traffic = GtkBox::new(Orientation::Horizontal, 7);
        traffic.set_css_classes(&["traffic-dots"]);
        traffic.set_valign(gtk4::Align::Center);
        let close_dot = Button::new();
        close_dot.set_tooltip_text(Some("Close Window"));
        close_dot.set_focus_on_click(false);
        close_dot.set_valign(gtk4::Align::Center);
        close_dot.set_css_classes(&["traffic-dot", "traffic-close"]);
        let min_dot = Button::new();
        min_dot.set_tooltip_text(Some("Minimize"));
        min_dot.set_focus_on_click(false);
        min_dot.set_valign(gtk4::Align::Center);
        min_dot.set_css_classes(&["traffic-dot", "traffic-min"]);
        let zoom_dot = Button::new();
        zoom_dot.set_tooltip_text(Some("Maximize"));
        zoom_dot.set_focus_on_click(false);
        zoom_dot.set_valign(gtk4::Align::Center);
        zoom_dot.set_css_classes(&["traffic-dot", "traffic-zoom"]);
        traffic.append(&close_dot);
        traffic.append(&min_dot);
        traffic.append(&zoom_dot);

        let back = Button::from_icon_name("go-previous-symbolic");
        back.set_tooltip_text(Some("Back (Alt+Left)"));
        back.set_focus_on_click(false);
        let forward = Button::from_icon_name("go-next-symbolic");
        forward.set_tooltip_text(Some("Forward (Alt+Right)"));
        forward.set_focus_on_click(false);
        let reload = Button::from_icon_name("view-refresh-symbolic");
        reload.set_tooltip_text(Some("Reload (Ctrl+R / F5)"));
        reload.set_focus_on_click(false);
        let stop = Button::from_icon_name("process-stop-symbolic");
        stop.set_tooltip_text(Some("Stop (Escape)"));
        stop.set_focus_on_click(false);
        stop.set_visible(false);

        let address = gtk4::Entry::new();
        address.set_placeholder_text(Some("Search or enter address"));
        address.set_hexpand(true);
        address.set_vexpand(false);
        address.set_focus_on_click(true);
        address.set_valign(gtk4::Align::Center);
        address.set_css_classes(&["urlbar"]);

        let close_tab = Button::from_icon_name("window-close-symbolic");
        close_tab.set_tooltip_text(Some("Close Tab (Ctrl+W)"));
        close_tab.set_focus_on_click(false);
        close_tab.set_visible(false);
        let sidebar_toggle = Button::from_icon_name("sidebar-show-symbolic");
        sidebar_toggle.set_tooltip_text(Some("Toggle Tabs (Ctrl+B)"));
        sidebar_toggle.set_focus_on_click(false);
        sidebar_toggle.set_visible(false);

        let tab_strip = GtkBox::new(Orientation::Horizontal, 4);
        tab_strip.set_css_classes(&["tab-strip"]);
        tab_strip.set_hexpand(false);

        let new_tab_btn = Button::from_icon_name("list-add-symbolic");
        new_tab_btn.set_tooltip_text(Some("New Tab (Ctrl+T)"));
        new_tab_btn.set_focus_on_click(false);
        new_tab_btn.set_css_classes(&["new-tab-button"]);

        let nav_group = GtkBox::new(Orientation::Horizontal, 2);
        nav_group.set_css_classes(&["nav-group"]);
        nav_group.append(&back);
        nav_group.append(&forward);
        nav_group.append(&reload);
        nav_group.set_visible(true);

        let left_chrome = GtkBox::new(Orientation::Horizontal, 6);
        left_chrome.set_css_classes(&["left-chrome"]);
        left_chrome.set_hexpand(false);
        left_chrome.set_valign(gtk4::Align::Center);
        left_chrome.append(&traffic);
        left_chrome.append(&nav_group);
        left_chrome.append(&tab_strip);
        left_chrome.append(&new_tab_btn);

        let right_chrome = GtkBox::new(Orientation::Horizontal, 4);
        right_chrome.set_css_classes(&["right-chrome"]);
        right_chrome.set_hexpand(false);
        right_chrome.set_valign(gtk4::Align::Center);
        right_chrome.append(&stop);
        right_chrome.append(&close_tab);
        right_chrome.append(&sidebar_toggle);

        let topbar = GtkBox::new(Orientation::Horizontal, 8);
        topbar.set_css_classes(&["topbar"]);
        topbar.append(&left_chrome);
        topbar.append(&address);
        topbar.append(&right_chrome);

        // Wrap topbar in a Revealer so it can slide in/out smoothly.
        let topbar_revealer = gtk4::Revealer::new();
        topbar_revealer.set_reveal_child(true);
        topbar_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        topbar_revealer.set_transition_duration(160);
        topbar_revealer.set_hexpand(true);
        topbar_revealer.set_child(Some(&topbar));

        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

        let progress = ProgressBar::new();
        progress.set_show_text(false);
        progress.set_fraction(0.0);
        progress.set_visible(false);
        progress.set_css_classes(&["progress-thin"]);

        // chrome_panel: floating panel (topbar + progress) that overlays the webview.
        // valign=Start keeps it anchored to the top regardless of content height.
        let chrome_panel = GtkBox::new(Orientation::Vertical, 0);
        chrome_panel.set_valign(gtk4::Align::Start);
        chrome_panel.set_hexpand(true);
        chrome_panel.set_css_classes(&["chrome-panel"]);
        chrome_panel.append(&topbar_revealer);
        chrome_panel.append(&progress);

        // Overlay: stack (web content) is the base; chrome_panel floats on top.
        // This lets the page fill the entire window when the topbar is hidden.
        let overlay = gtk4::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        overlay.set_child(Some(&stack));
        overlay.add_overlay(&chrome_panel);

        let outer = GtkBox::new(Orientation::Vertical, 0);
        outer.append(&overlay);
        window.set_child(Some(&outer));

        install_css();

        let me: Rc<Self> = Rc::new_cyclic(|weak_self| Self {
            window,
            address,
            progress,
            stack,
            tab_strip,
            topbar_revealer,
            auto_hide: Cell::new(false),
            tab_rows: RefCell::new(Vec::new()),
            tabs: RefCell::new(Vec::new()),
            current: RefCell::new(None),
            self_weak: weak_self.clone(),
        });

        me.wire_navigation(&back, &forward, &reload, &stop);
        me.wire_address();
        me.wire_keyboard();
        me.wire_window_controls(&close_dot, &min_dot, &zoom_dot);
        me.wire_sidebar_toggle(&sidebar_toggle);
        me.wire_new_tab(&new_tab_btn);
        me.wire_close_tab(&close_tab);
        me.wire_topbar_hover();

        me
    }

    pub fn add_tab(&self, blank: bool) -> Rc<Tab> {
        let tab = if blank {
            Tab::new_blank()
        } else {
            Tab::new(config::HOME_PAGE)
        };
        let page_name = format!("page-{}", self.tabs.borrow().len());

        let me_weak: Weak<Self> = self.self_weak.clone();
        tab.on_event(move |ev| {
            let Some(me) = me_weak.upgrade() else { return };
            let cur = me.current.borrow().clone();
            let current_ref = cur.as_deref().map(|t| t as &Tab);
            me.on_tab_event(current_ref, ev);
        });

        self.stack
            .add_titled(tab.widget(), Some(&page_name), "New Tab");

        // If blank, load the new-tab page now that the widget is in the tree.
        if blank {
            tab.load_new_tab_page();
        }

        let row = TabRow::new_from_window(self, Rc::clone(&tab));
        self.tab_strip.append(&row.container);
        self.tab_rows.borrow_mut().push(Rc::clone(&row));

        self.tabs.borrow_mut().push(Rc::clone(&tab));
        self.activate_tab(Rc::clone(&tab));
        tab
    }

    fn activate_tab(&self, tab: Rc<Tab>) {
        self.stack.set_visible_child(tab.widget());
        // Reorder tabs so pinned tabs stay on the left.
        self.resort_tabs();
        // Update "active" CSS class on rows.
        for row in self.tab_rows.borrow().iter() {
            let is_active = row.tab.upgrade().map_or(false, |t| Rc::ptr_eq(&t, &tab));
            if is_active {
                row.container.add_css_class("active-tab");
            } else {
                row.container.remove_css_class("active-tab");
            }
        }
        let active_url = tab
            .webview
            .uri()
            .unwrap_or_else(|| tab.url.borrow().clone());
        self.address.set_text(&self.address_text_for(&active_url));
        self.update_auto_hide_for_url(&active_url);
        *self.current.borrow_mut() = Some(tab);
    }

    fn current_tab(&self) -> Option<Rc<Tab>> {
        self.current.borrow().as_ref().map(Rc::clone)
    }

    fn on_tab_event(&self, _current: Option<&Tab>, ev: TabEvent) {
        match ev {
            TabEvent::UriChanged(uri) => {
                let display_uri = self.address_text_for(&uri);
                if self.address.text().to_string() != display_uri {
                    self.address.set_text(&display_uri);
                }
                self.update_auto_hide_for_url(&uri);
                if let Some(tab) = self.current_tab() {
                    if let Some(row) = self.find_row_for(&tab) {
                        // Belt-and-suspenders: force label to "New Tab" for any
                        // data:/about: URI so the raw URL never leaks into the tab pill.
                        row.title_label.set_text(&self.short_url(&uri));
                    }
                }
                // Re-order if pin state changed via load (rare).
                self.resort_tabs();
            }
            TabEvent::TitleChanged(title) => {
                let short = self.short_title(&title);
                if let Some(tab) = self.current_tab() {
                    if let Some(row) = self.find_row_for(&tab) {
                        row.title_label.set_tooltip_text(Some(&short));
                        row.title_label
                            .set_text(&self.tab_label_for(&tab, tab.pinned.get()));
                    }
                }
            }
            TabEvent::LoadingChanged(loading) => {
                self.progress.set_visible(loading);
                if loading {
                    self.progress.pulse();
                } else {
                    self.progress.set_fraction(0.0);
                }
            }
            TabEvent::LoadFailed { uri, message } => {
                self.render_error_page(&uri, &message);
            }
            TabEvent::PinnedChanged(_pinned) => {
                self.resort_tabs();
            }
        }
    }

    fn find_row_for(&self, tab: &Rc<Tab>) -> Option<Rc<TabRow>> {
        self.tab_rows
            .borrow()
            .iter()
            .find(|r| r.tab.upgrade().map_or(false, |t| Rc::ptr_eq(&t, tab)))
            .map(Rc::clone)
    }

    /// Reorders top tabs so pinned tabs come first, then unpinned in original order.
    fn resort_tabs(&self) {
        let rows = self.tab_rows.borrow_mut();
        let mut pinned: Vec<Rc<TabRow>> = Vec::new();
        let mut unpinned: Vec<Rc<TabRow>> = Vec::new();
        for r in rows.iter() {
            let is_pinned = r.tab.upgrade().map(|t| t.pinned.get()).unwrap_or(false);
            if is_pinned {
                pinned.push(Rc::clone(r));
            } else {
                unpinned.push(Rc::clone(r));
            }
        }
        let mut ordered: Vec<Rc<TabRow>> = Vec::new();
        ordered.extend(pinned);
        ordered.extend(unpinned);

        for r in ordered.iter() {
            self.tab_strip.remove(&r.container);
        }
        for r in ordered.iter() {
            let is_pinned = r.tab.upgrade().map(|t| t.pinned.get()).unwrap_or(false);
            if is_pinned {
                r.container.add_css_class("pinned-tab");
                r.container.set_width_request(26);
                r.title_label.set_xalign(0.5);
                r.close_btn.set_visible(false);
                r.pin_btn.set_visible(false);
            } else {
                r.container.remove_css_class("pinned-tab");
                r.container.set_width_request(158);
                r.title_label.set_xalign(0.0);
                r.close_btn.set_visible(true);
            }
            if let Some(tab) = r.tab.upgrade() {
                r.title_label.set_text(&self.tab_label_for(&tab, is_pinned));
            }
            self.tab_strip.append(&r.container);
        }
    }

    fn short_title(&self, title: &str) -> String {
        if title.is_empty() {
            "New Tab".to_string()
        } else if title.chars().count() > 60 {
            let truncated: String = title.chars().take(59).collect();
            format!("{truncated}…")
        } else {
            title.to_string()
        }
    }

    fn short_url(&self, url: &str) -> String {
        if url.is_empty()
            || url == "about:newtab"
            || url == "about:blank"
            || url.starts_with("data:")
        {
            "New Tab".to_string()
        } else if url.chars().count() > 32 {
            let truncated: String = url.chars().take(31).collect();
            format!("{truncated}…")
        } else {
            url.to_string()
        }
    }

    fn address_text_for(&self, url: &str) -> String {
        if url.is_empty()
            || url == "about:newtab"
            || url == "about:blank"
            || url.starts_with("data:")
        {
            String::new()
        } else {
            url.to_string()
        }
    }

    fn tab_label_for(&self, tab: &Tab, pinned: bool) -> String {
        let title = tab.title.borrow().clone();
        let url = tab.url.borrow().clone();
        let text = if url.is_empty()
            || url == "about:newtab"
            || url == "about:blank"
            || url.starts_with("data:")
        {
            "New Tab".to_string()
        } else {
            self.short_url(&url)
        };

        if pinned {
            let source = if title.is_empty() { text } else { title };
            source
                .chars()
                .find(|c| c.is_ascii_alphanumeric())
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "N".to_string())
        } else {
            let title_is_real =
                !title.is_empty() && title != "New Tab" && !title.starts_with("data:");
            if title_is_real {
                self.short_title(&title)
            } else {
                text
            }
        }
    }

    fn render_error_page(&self, uri: &str, message: &str) {
        let html = config::ERROR_PAGE_HTML
            .replace("{uri}", &html_escape(uri))
            .replace("{message}", &html_escape(message));
        if let Some(tab) = self.current_tab() {
            use std::ffi::CString;
            let c_html = CString::new(html).unwrap_or_default();
            let c_base = CString::new(uri).unwrap_or_default();
            unsafe {
                ffi::webkit_web_view_load_html(
                    tab.webview.web_view_ptr(),
                    c_html.as_ptr(),
                    c_base.as_ptr(),
                );
            }
        }
    }

    fn wire_navigation(&self, back: &Button, forward: &Button, reload: &Button, stop: &Button) {
        let me_weak = self.self_weak.clone();
        back.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                if let Some(tab) = me.current_tab() {
                    tab.go_back();
                }
            }
        });
        let me_weak = self.self_weak.clone();
        forward.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                if let Some(tab) = me.current_tab() {
                    tab.go_forward();
                }
            }
        });
        let me_weak = self.self_weak.clone();
        reload.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                if let Some(tab) = me.current_tab() {
                    tab.reload();
                }
            }
        });
        let me_weak = self.self_weak.clone();
        stop.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                if let Some(tab) = me.current_tab() {
                    tab.stop();
                }
            }
        });
    }

    fn wire_address(&self) {
        let me_weak = self.self_weak.clone();
        self.address.connect_activate(move |e| {
            if let Some(me) = me_weak.upgrade() {
                if let Some(tab) = me.current_tab() {
                    let text = e.text().to_string();
                    tab.navigate(&text);
                }
            }
        });
    }

    fn wire_sidebar_toggle(&self, btn: &Button) {
        let me_weak = self.self_weak.clone();
        btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.toggle_sidebar();
            }
        });
    }

    fn wire_window_controls(&self, close: &Button, minimize: &Button, zoom: &Button) {
        let win = self.window.clone();
        close.connect_clicked(move |_| {
            win.close();
        });

        let win = self.window.clone();
        minimize.connect_clicked(move |_| {
            win.minimize();
        });

        let win = self.window.clone();
        zoom.connect_clicked(move |_| {
            if win.is_maximized() {
                win.unmaximize();
            } else {
                win.maximize();
            }
        });
    }

    fn wire_new_tab(&self, btn: &Button) {
        let me_weak = self.self_weak.clone();
        btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.add_tab(true);
            }
        });
    }

    fn wire_close_tab(&self, btn: &Button) {
        let me_weak = self.self_weak.clone();
        btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.close_current_tab();
            }
        });
    }

    fn toggle_sidebar(&self) {
        self.tab_strip.set_visible(!self.tab_strip.is_visible());
    }

    // ── Auto-hide topbar (Zen-style) ─────────────────────────────────────────

    /// Called on every URL change (navigation + tab switch) to decide whether
    /// the topbar should auto-hide.  New-tab / blank pages always show it.
    fn update_auto_hide_for_url(&self, url: &str) {
        let is_blank = url.is_empty()
            || url == "about:newtab"
            || url == "about:blank"
            || url.starts_with("data:");
        if is_blank {
            self.disable_auto_hide();
        } else {
            self.enable_auto_hide();
        }
    }

    /// Enter auto-hide mode: slide the topbar up out of view.
    fn enable_auto_hide(&self) {
        self.auto_hide.set(true);
        self.topbar_revealer.set_reveal_child(false);
    }

    /// Leave auto-hide mode: always show the topbar.
    fn disable_auto_hide(&self) {
        self.auto_hide.set(false);
        self.topbar_revealer.set_reveal_child(true);
    }

    /// Wire a window-level motion capture controller that:
    ///   • shows the topbar when the pointer is within 8 px of the top edge, and
    ///   • hides it again once the pointer moves more than 54 px below the top
    ///     (i.e. below the fully-revealed topbar).
    fn wire_topbar_hover(&self) {
        let motion = gtk4::EventControllerMotion::new();
        // Capture phase: we see events before WebKit or any child widget.
        motion.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let me_weak = self.self_weak.clone();
        motion.connect_motion(move |_, _x, y| {
            let Some(me) = me_weak.upgrade() else { return };
            if !me.auto_hide.get() {
                return;
            }
            if y < 8.0 {
                // Pointer is near the top edge → reveal.
                me.topbar_revealer.set_reveal_child(true);
            } else if y > 54.0 {
                // Pointer has moved below the topbar zone → hide again.
                me.topbar_revealer.set_reveal_child(false);
            }
        });
        self.window.add_controller(motion);
    }

    fn wire_keyboard(&self) {
        let controller = EventControllerKey::new();
        controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let me_weak = self.self_weak.clone();
        controller.connect_key_pressed(move |_ctrl, key, _keycode, state| {
            let Some(me) = me_weak.upgrade() else {
                return gtk4::glib::Propagation::Proceed;
            };
            let ctrl = state.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let shift = state.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
            let alt = state.contains(gtk4::gdk::ModifierType::ALT_MASK);

            let cmd = match (ctrl, shift, alt, key) {
                (true, false, false, Key::l) => Some(WinCmd::FocusAddress),
                (true, false, false, Key::r) => Some(WinCmd::Reload),
                (true, false, false, Key::b) => Some(WinCmd::ToggleSidebar),
                (false, false, true, Key::Left) => Some(WinCmd::Back),
                (false, false, true, Key::Right) => Some(WinCmd::Forward),
                (false, false, false, Key::F5) => Some(WinCmd::Reload),
                (false, false, false, Key::Escape) => Some(WinCmd::Stop),
                (true, false, false, Key::t) => Some(WinCmd::NewTab),
                (true, false, false, Key::w) => Some(WinCmd::CloseTab),
                (true, false, false, Key::Tab) => Some(WinCmd::NextTab),
                (true, true, false, Key::Tab) | (true, true, false, Key::ISO_Left_Tab) => {
                    Some(WinCmd::PrevTab)
                }
                (true, false, false, Key::p) => Some(WinCmd::TogglePin),
                (true, false, false, Key::Page_Up) => Some(WinCmd::NextWorkspace),
                (true, false, false, Key::_1) => Some(WinCmd::SwitchTab(1)),
                (true, false, false, Key::_2) => Some(WinCmd::SwitchTab(2)),
                (true, false, false, Key::_3) => Some(WinCmd::SwitchTab(3)),
                (true, false, false, Key::_4) => Some(WinCmd::SwitchTab(4)),
                (true, false, false, Key::_5) => Some(WinCmd::SwitchTab(5)),
                (true, false, false, Key::_6) => Some(WinCmd::SwitchTab(6)),
                (true, false, false, Key::_7) => Some(WinCmd::SwitchTab(7)),
                (true, false, false, Key::_8) => Some(WinCmd::SwitchTab(8)),
                (true, false, false, Key::_9) => Some(WinCmd::SwitchTab(9)),
                _ => None,
            };

            if let Some(cmd) = cmd {
                me.handle_cmd(cmd);
                gtk4::glib::Propagation::Stop
            } else {
                gtk4::glib::Propagation::Proceed
            }
        });
        self.window.add_controller(controller);
    }

    pub fn handle_cmd(&self, cmd: WinCmd) {
        match cmd {
            WinCmd::Back => {
                if let Some(t) = self.current_tab() {
                    t.go_back();
                }
            }
            WinCmd::Forward => {
                if let Some(t) = self.current_tab() {
                    t.go_forward();
                }
            }
            WinCmd::Reload => {
                if let Some(t) = self.current_tab() {
                    t.reload();
                }
            }
            WinCmd::Stop => {
                if let Some(t) = self.current_tab() {
                    t.stop();
                }
            }
            WinCmd::FocusAddress => {
                self.address.grab_focus();
                self.address.select_region(0, -1);
            }
            WinCmd::NewTab => {
                self.add_tab(true);
            }
            WinCmd::CloseTab => {
                self.close_current_tab();
            }
            WinCmd::NextTab => self.switch_tab_relative(1),
            WinCmd::NextWorkspace => self.switch_tab_relative(1),
            WinCmd::PrevTab => self.switch_tab_relative(-1),
            WinCmd::SwitchTab(n) => self.switch_tab_index((n - 1).max(0) as usize),
            WinCmd::ToggleSidebar => self.toggle_sidebar(),
            WinCmd::TogglePin => {
                if let Some(t) = self.current_tab() {
                    t.toggle_pinned();
                }
            }
        }
    }

    fn switch_tab_relative(&self, delta: i32) {
        let tabs = self.tabs.borrow();
        if tabs.is_empty() {
            return;
        }
        // When switching with Ctrl+Tab, skip pinned tabs (workspaces-style).
        // Simpler MVP: just iterate in tab order (pinned first after resort).
        let current_idx = self
            .current
            .borrow()
            .as_ref()
            .and_then(|cur| tabs.iter().position(|t| Rc::ptr_eq(t, cur)))
            .unwrap_or(0);
        let len = tabs.len() as i32;
        let new_idx = ((current_idx as i32 + delta) % len + len) % len;
        let tab = Rc::clone(&tabs[new_idx as usize]);
        drop(tabs);
        self.activate_tab(tab);
    }

    fn switch_tab_index(&self, idx: usize) {
        let tabs = self.tabs.borrow();
        if let Some(tab) = tabs.get(idx) {
            let tab = Rc::clone(tab);
            drop(tabs);
            self.activate_tab(tab);
        }
    }

    fn close_current_tab(&self) {
        let mut tabs = self.tabs.borrow_mut();
        if tabs.is_empty() {
            return;
        }
        let cur_idx = self
            .current
            .borrow()
            .as_ref()
            .and_then(|cur| tabs.iter().position(|t| Rc::ptr_eq(t, cur)))
            .unwrap_or(0);
        let tab = tabs.remove(cur_idx);
        self.stack.remove(tab.widget());

        let mut rows = self.tab_rows.borrow_mut();
        if let Some(row_pos) = rows
            .iter()
            .position(|r| r.tab.upgrade().map_or(false, |t| Rc::ptr_eq(&t, &tab)))
        {
            let row = rows.remove(row_pos);
            self.tab_strip.remove(&row.container);
        }
        drop(rows);

        if tabs.is_empty() {
            drop(tabs);
            self.add_tab(true);
        } else {
            let new_idx = cur_idx.min(tabs.len() - 1);
            let new_tab = Rc::clone(&tabs[new_idx]);
            drop(tabs);
            self.activate_tab(new_tab);
        }
    }
}

impl TabRow {
    fn new_from_window(window: &BrowserWindow, tab: Rc<Tab>) -> Rc<Self> {
        let win_rc: Rc<BrowserWindow> = window
            .self_weak
            .upgrade()
            .expect("BrowserWindow must outlive TabRow");
        Self::new(&win_rc, tab)
    }

    fn new(window: &Rc<BrowserWindow>, tab: Rc<Tab>) -> Rc<Self> {
        let container = GtkBox::new(Orientation::Horizontal, 5);
        container.set_css_classes(&["tab-row"]);
        container.set_hexpand(false);
        container.set_width_request(158);

        let pin_btn = Button::from_icon_name("pin-symbolic");
        pin_btn.set_tooltip_text(Some("Pin (Ctrl+P)"));
        pin_btn.set_css_classes(&["tab-pin"]);
        pin_btn.set_focus_on_click(false);
        pin_btn.set_visible(false);
        container.append(&pin_btn);

        let title_label = Label::new(Some("New Tab"));
        title_label.set_xalign(0.0);
        title_label.set_hexpand(true);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(16);
        title_label.set_single_line_mode(true);
        container.append(&title_label);

        let close_btn = Button::from_icon_name("window-close-symbolic");
        close_btn.set_css_classes(&["tab-close"]);
        close_btn.set_tooltip_text(Some("Close Tab"));
        close_btn.set_focus_on_click(false);
        container.append(&close_btn);

        // Whole row clickable to activate.
        let gesture = GestureClick::new();
        gesture.set_button(gtk4::gdk::BUTTON_PRIMARY);
        let win_weak = Rc::downgrade(window);
        let tab_weak = Rc::downgrade(&tab);
        gesture.connect_pressed(move |_, n_press, _, _| {
            if n_press < 1 {
                return;
            }
            if let (Some(win), Some(tab)) = (win_weak.upgrade(), tab_weak.upgrade()) {
                win.activate_tab(tab);
            }
        });
        container.add_controller(gesture);

        // Middle-click closes.
        let gesture_mid = GestureClick::new();
        gesture_mid.set_button(gtk4::gdk::BUTTON_MIDDLE);
        let win_weak = Rc::downgrade(window);
        let tab_weak = Rc::downgrade(&tab);
        gesture_mid.connect_pressed(move |_, _, _, _| {
            if let (Some(win), Some(tab)) = (win_weak.upgrade(), tab_weak.upgrade()) {
                win.activate_tab(tab);
                win.close_current_tab();
            }
        });
        container.add_controller(gesture_mid);

        // Right-click → context menu with Pin/Unpin.
        let gesture_right = GestureClick::new();
        gesture_right.set_button(gtk4::gdk::BUTTON_SECONDARY);
        let win_weak = Rc::downgrade(window);
        let tab_weak = Rc::downgrade(&tab);
        gesture_right.connect_pressed(move |gesture, _, x, y| {
            let Some(win) = win_weak.upgrade() else {
                return;
            };
            let Some(tab) = tab_weak.upgrade() else {
                return;
            };
            win.show_pin_menu(gesture, &tab, x, y);
        });
        container.add_controller(gesture_right);

        // Close button.
        let win_weak = Rc::downgrade(window);
        let tab_weak = Rc::downgrade(&tab);
        close_btn.connect_clicked(move |_| {
            if let (Some(win), Some(tab)) = (win_weak.upgrade(), tab_weak.upgrade()) {
                win.activate_tab(Rc::clone(&tab));
                win.close_current_tab();
            }
        });

        // Pin button (visible on hover / always when pinned).
        let win_weak = Rc::downgrade(window);
        let tab_weak = Rc::downgrade(&tab);
        pin_btn.connect_clicked(move |_| {
            if let (Some(_win), Some(tab)) = (win_weak.upgrade(), tab_weak.upgrade()) {
                tab.toggle_pinned();
            }
        });

        // Hover: show pin button when not pinned.
        let motion = gtk4::EventControllerMotion::new();
        let pin_weak_for_show = pin_btn.clone();
        motion.connect_enter(move |_, _, _| {
            pin_weak_for_show.set_visible(false);
        });
        let pin_weak_for_hide = pin_btn.clone();
        let tab_weak_for_hide = Rc::downgrade(&tab);
        motion.connect_leave(move |_| {
            if let Some(t) = tab_weak_for_hide.upgrade() {
                if !t.pinned.get() {
                    pin_weak_for_hide.set_visible(false);
                }
            }
        });
        container.add_controller(motion);

        let row = Rc::new(Self {
            container,
            title_label,
            close_btn,
            tab: Rc::downgrade(&tab),
            pin_btn,
        });
        row
    }
}

impl BrowserWindow {
    fn show_pin_menu(&self, gesture: &GestureClick, tab: &Rc<Tab>, _x: f64, _y: f64) {
        let menu = gtk4::gio::Menu::new();
        if tab.pinned.get() {
            menu.append(Some("Unpin"), Some("win.unpin"));
        } else {
            menu.append(Some("Pin"), Some("win.pin"));
        }
        menu.append(Some("Close Tab"), Some("win.close"));

        let popover = PopoverMenu::from_model(Some(&menu));
        popover.set_parent(
            &gesture
                .widget()
                .unwrap_or_else(|| self.tab_strip.clone().upcast::<gtk4::Widget>()),
        );
        popover.set_has_arrow(false);

        let action_pinned = gtk4::gio::SimpleAction::new("pin", None);
        let tab_weak = Rc::downgrade(tab);
        action_pinned.connect_activate(move |_, _| {
            if let Some(t) = tab_weak.upgrade() {
                t.set_pinned(true);
            }
        });
        self.window.add_action(&action_pinned);

        let action_unpinned = gtk4::gio::SimpleAction::new("unpin", None);
        let tab_weak = Rc::downgrade(tab);
        action_unpinned.connect_activate(move |_, _| {
            if let Some(t) = tab_weak.upgrade() {
                t.set_pinned(false);
            }
        });
        self.window.add_action(&action_unpinned);

        let action_close = gtk4::gio::SimpleAction::new("close", None);
        let win_weak = self.self_weak.clone();
        let tab_weak = Rc::downgrade(tab);
        action_close.connect_activate(move |_, _| {
            if let (Some(win), Some(tab)) = (win_weak.upgrade(), tab_weak.upgrade()) {
                win.activate_tab(tab);
                win.close_current_tab();
            }
        });
        self.window.add_action(&action_close);

        popover.popup();
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn install_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        r#"
        window {
            background-color: #f0f0f0;
            color: #1c1c1e;
        }

        /* ── Chrome panel (floats over web content via Overlay) ─────── */
        .chrome-panel {
            /* Transparent background so only the topbar itself draws pixels. */
        }

        /* ── Topbar ──────────────────────────────────────────────────── */
        .topbar {
            min-height: 44px;
            padding: 0 12px;
            background-color: #ececec;
            /* Subtle shadow so the bar visually separates from page content
               when it floats over it in auto-hide overlay mode. */
            box-shadow: 0 1px 6px rgba(0,0,0,0.15);
            border-bottom: 1px solid rgba(0,0,0,0.10);
        }

        /* ── Traffic-light dots ────────────────────────────────── */
        .traffic-dots {
            margin-right: 10px;
        }

        /* Hard-cap size so GTK button defaults can't stretch them */
        .traffic-dot {
            min-width:  12px;
            min-height: 12px;
            max-width:  12px;
            max-height: 12px;
            padding: 0;
            margin: 0;
            border-radius: 999px;
            border: 0;
            box-shadow: inset 0 0 0 0.5px rgba(0,0,0,0.20);
        }
        /* Remove any icon/label padding GTK might inject */
        .traffic-dot > * { padding: 0; margin: 0; }

        .traffic-close { background: #ff5f57; background-image: none; }
        .traffic-min   { background: #ffbd2e; background-image: none; }
        .traffic-zoom  { background: #28c840; background-image: none; }

        .traffic-close:hover { background: #e0443c; background-image: none; }
        .traffic-min:hover   { background: #e0a012; background-image: none; }
        .traffic-zoom:hover  { background: #14a830; background-image: none; }

        /* ── Nav buttons ───────────────────────────────────────── */
        .nav-group {
            margin-right: 4px;
        }

        .nav-group button {
            min-width:  26px;
            min-height: 26px;
            max-height: 26px;
            padding: 3px;
            border-radius: 6px;
            color: rgba(0,0,0,0.45);
            background: transparent;
            border: 0;
            box-shadow: none;
        }

        .nav-group button:hover {
            color: rgba(0,0,0,0.75);
            background: rgba(0,0,0,0.07);
        }

        /* ── Tab strip ─────────────────────────────────────────── */
        .tab-strip {
            margin: 0 2px;
        }

        .tab-row {
            min-height: 28px;
            max-height: 28px;
            padding: 0 9px;
            border-radius: 8px;
            background: transparent;
            color: rgba(0,0,0,0.55);
            transition: background 100ms ease, color 100ms ease;
        }

        .tab-row:hover {
            background: rgba(0,0,0,0.06);
            color: rgba(0,0,0,0.80);
        }

        .tab-row.active-tab {
            background-color: #d8d8d8;
            color: #111111;
            box-shadow: 0 1px 3px rgba(0,0,0,0.10);
        }

        .tab-row.pinned-tab {
            padding: 0 4px;
            min-width: 28px;
            max-height: 28px;
            border-radius: 8px;
            background: rgba(0,0,0,0.05);
            color: rgba(0,0,0,0.40);
        }

        .tab-row.pinned-tab.active-tab {
            background-color: #d8d8d8;
            color: #111111;
        }

        .tab-row label {
            font-size: 12px;
            font-weight: 400;
        }

        .tab-row.pinned-tab label {
            font-size: 11px;
            font-weight: 600;
        }

        .tab-row .tab-close {
            padding: 0;
            min-width:  16px;
            min-height: 16px;
            max-width:  16px;
            max-height: 16px;
            border-radius: 999px;
            opacity: 0;
            color: rgba(0,0,0,0.50);
            background: transparent;
            border: 0;
            box-shadow: none;
        }

        .tab-row:hover .tab-close,
        .tab-row.active-tab .tab-close {
            opacity: 0.60;
        }

        .tab-row .tab-close:hover {
            opacity: 1.0;
            background: rgba(0,0,0,0.10);
        }

        .tab-row .tab-pin {
            padding: 0;
            min-width: 0;
            min-height: 0;
            opacity: 0;
        }

        /* ── URL bar ───────────────────────────────────────────── */
        .urlbar {
            min-height: 30px;
            max-height: 30px;
            padding: 0 14px;
            border-radius: 8px;
            background-color: rgba(0,0,0,0.08);
            color: #1c1c1e;
            border: 1px solid transparent;
            box-shadow: none;
            font-size: 12.5px;
        }

        .urlbar:focus {
            background-color: #ffffff;
            border-color: rgba(0,100,220,0.35);
            box-shadow: 0 0 0 3px rgba(0,100,220,0.10);
        }

        /* ── Progress bar ──────────────────────────────────────── */
        .progress-thin {
            min-height: 2px;
        }

        /* ── New-tab button ────────────────────────────────────── */
        .new-tab-button {
            min-width:  22px;
            min-height: 22px;
            max-width:  22px;
            max-height: 22px;
            padding: 2px;
            margin-left: 2px;
            opacity: 0.50;
            border-radius: 6px;
            background: transparent;
            border: 0;
            box-shadow: none;
            color: rgba(0,0,0,0.55);
        }

        .new-tab-button:hover {
            opacity: 0.85;
            background: rgba(0,0,0,0.07);
        }
        "#,
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
