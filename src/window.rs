//! Browser window: Zen-inspired layout with compact top tabs, a minimal
//! address field, and a blank centered-search new-tab page.

use std::cell::{Cell, RefCell};
use std::os::raw::c_void;
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
    OpenBrowserMenu,
    OpenSettings,
    FindOpen,
    FindNext,
    FindPrev,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleFullscreen,
}

/// One active download, tracked for the bottom-bar progress pill.
struct ActiveDownload {
    id: u64,
    name: String,
    dest: String,
    /// Strong ref (taken at download-started, released at every removal
    /// site). `download-started` only loans the object — stashing the raw
    /// pointer while we poll it every 400ms is a use-after-free and shows up
    /// as heap corruption once WebKit finalizes the download.
    download: *mut ffi::WebKitDownload,
}

/// Widgets kept alive for the bottom-right download card, so each poll tick
/// updates text and bar in place instead of rebuilding the overlay.
struct DownloadPill {
    root: GtkBox,
    title: Label,
    meta: Label,
    bar: ProgressBar,
}


/// Theme preferences supported by the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeLayout {
    Topbar,
    Sidebar,
}

pub struct BrowserWindow {
    pub window: ApplicationWindow,
    pub address: gtk4::Entry,
    sidebar_address: gtk4::Entry,
    /// Sidebar navigation row (hidden in Topbar layout).
    sidebar_nav_back: Button,
    sidebar_nav_forward: Button,
    sidebar_nav_reload: Button,
    pub progress: ProgressBar,
    pub stack: Stack,
    pub tab_strip: GtkBox,
    pub tab_scroll: gtk4::ScrolledWindow,
    browser_menu_btn: Button,
    sidebar_pinned_label: Label,
    sidebar_pinned_tabs: GtkBox,
    sidebar_tabs_label: Label,
    sidebar_tabs: GtkBox,
    pub overlay: gtk4::Overlay,
    /// Revealer wrapping the topbar – used for smooth auto-hide slide animation.
    topbar_revealer: gtk4::Revealer,
    /// Revealer wrapping the left sidebar – used for smooth auto-hide slide animation.
    sidebar_revealer: gtk4::Revealer,
    /// Active in-app settings modal container, if open.
    active_settings_modal: RefCell<Option<GtkBox>>,
    /// True while the topbar/sidebar is in auto-hide mode (real site loaded, not newtab).
    auto_hide: Cell<bool>,
    /// Whether the left sidebar is enabled in settings.
    sidebar_enabled: Cell<bool>,
    chrome_layout: Cell<ChromeLayout>,
    /// Current theme preference (System, Light, Dark).
    theme_pref: Cell<ThemePreference>,
    /// Maps each Tab's WebView widget pointer to its top tab pill.
    tab_rows: RefCell<Vec<Rc<TabRow>>>,
    tabs: RefCell<Vec<Rc<Tab>>>,
    current: RefCell<Option<Rc<Tab>>>,
    self_weak: Weak<Self>,
    /// Live downloads, for the bottom-bar progress pill.
    downloads: RefCell<Vec<ActiveDownload>>,
    next_download_id: Cell<u64>,
    /// Sidebar/downloads-button badge: total completed downloads this session.
    completed_downloads: Cell<u64>,
    /// Live popover, if open — refreshed on progress ticks.
    downloads_popover: RefCell<Option<gtk4::Popover>>,
    /// Popover rows container (rebuilt on progress ticks while open).
    downloads_popover_rows: RefCell<Option<GtkBox>>,
    /// Open full-page downloads view (native overlay), if shown.
    active_downloads_page: RefCell<Option<GtkBox>>,
    /// Row container inside the open downloads page.
    downloads_page_list: RefCell<Option<GtkBox>>,
    /// Search box inside the open downloads page.
    downloads_page_search: RefCell<Option<gtk4::Entry>>,
    /// Persistent pill shown while any download is active.
    download_pill: RefCell<Option<DownloadPill>>,
    /// Open "Save Download" dialogs by download id, kept alive so a dropped
    /// visible dialog can't be a use-after-free, and closed automatically if
    /// the download they belong to fails or is cancelled.
    pending_save_dialogs: RefCell<Vec<(u64, gtk4::FileChooserDialog)>>,
    /// Persisted user settings (download dir, ask-every-time, ...).
    settings: RefCell<config::Settings>,
    /// Downloads button in the sidebar bottom bar (badge updates).
    downloads_btn: Button,
    /// Find bar widgets (None until first opened).
    find_bar: RefCell<Option<FindBar>>,
    /// Webview in fullscreen video mode.
    fullscreen: Cell<bool>,
    /// Window size to restore when leaving fullscreen.
    pre_fullscreen: Cell<(i32, i32)>,
}

/// Find-in-page bar state.
struct FindBar {
    container: GtkBox,
    entry: gtk4::Entry,
}

/// One compact tab pill in the top browser chrome.
struct TabRow {
    container: GtkBox,
    title_label: Label,
    close_btn: Button,
    favicon_letter: Label,
    favicon_image: gtk4::Image,
    favicon_stack: gtk4::Stack,
    has_icon: Cell<bool>,
    tab: Weak<Tab>,
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
        let make_traffic_button = |class_name: &str, tooltip: &str| {
            let button = Button::new();
            button.set_size_request(12, 12);
            button.set_tooltip_text(Some(tooltip));
            button.set_focus_on_click(false);
            button.set_valign(gtk4::Align::Center);
            button.set_css_classes(&["traffic-dot", class_name]);
            button
        };

        let traffic = GtkBox::new(Orientation::Horizontal, 7);
        traffic.set_css_classes(&["traffic-dots"]);
        traffic.set_valign(gtk4::Align::Center);
        let close_dot = make_traffic_button("traffic-close", "Close Window");
        let min_dot = make_traffic_button("traffic-min", "Minimize");
        let zoom_dot = make_traffic_button("traffic-zoom", "Maximize");
        traffic.append(&close_dot);
        traffic.append(&min_dot);
        traffic.append(&zoom_dot);

        let back = Button::from_icon_name("go-previous-symbolic");
        back.set_size_request(26, 26);
        back.set_tooltip_text(Some("Back (Alt+Left)"));
        back.set_focus_on_click(false);
        let forward = Button::from_icon_name("go-next-symbolic");
        forward.set_size_request(26, 26);
        forward.set_tooltip_text(Some("Forward (Alt+Right)"));
        forward.set_focus_on_click(false);
        let reload = Button::from_icon_name("view-refresh-symbolic");
        reload.set_size_request(26, 26);
        reload.set_tooltip_text(Some("Reload (Ctrl+R / F5)"));
        reload.set_focus_on_click(false);
        let stop = Button::from_icon_name("process-stop-symbolic");
        stop.set_size_request(26, 26);
        stop.set_tooltip_text(Some("Stop (Escape)"));
        stop.set_focus_on_click(false);
        stop.set_visible(false);

        let address = gtk4::Entry::new();
        address.set_placeholder_text(Some("Search or enter address"));
        address.set_icon_from_icon_name(
            gtk4::EntryIconPosition::Primary,
            Some("system-search-symbolic"),
        );
        address.set_hexpand(false);
        address.set_vexpand(false);
        address.set_width_request(240);
        address.set_focus_on_click(true);
        address.set_valign(gtk4::Align::Center);
        address.set_css_classes(&["urlbar"]);

        let close_tab = Button::from_icon_name("window-close-symbolic");
        close_tab.set_tooltip_text(Some("Close Tab (Ctrl+W)"));
        close_tab.set_focus_on_click(false);
        close_tab.set_visible(false);
        let sidebar_toggle = Button::from_icon_name("sidebar-show-symbolic");
        sidebar_toggle.set_tooltip_text(Some("Switch Chrome Layout (Ctrl+B)"));
        sidebar_toggle.set_focus_on_click(false);
        sidebar_toggle.set_visible(false);

        let tab_strip = GtkBox::new(Orientation::Horizontal, 4);
        tab_strip.set_css_classes(&["tab-strip"]);
        tab_strip.set_hexpand(false);

        let tab_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .propagate_natural_width(true)
            .propagate_natural_height(true)
            .hexpand(true)
            .build();
        tab_scroll.set_css_classes(&["tab-scroll"]);
        tab_scroll.set_child(Some(&tab_strip));
        tab_scroll.set_valign(gtk4::Align::Center);

        let scroll_ctrl = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL
                | gtk4::EventControllerScrollFlags::HORIZONTAL,
        );
        let tab_scroll_weak = tab_scroll.downgrade();
        scroll_ctrl.connect_scroll(move |_, dx, dy| {
            if let Some(sc) = tab_scroll_weak.upgrade() {
                let hadj = sc.hadjustment();
                let delta = if dy.abs() > dx.abs() { dy } else { dx };
                let step = delta * 40.0;
                let new_val = (hadj.value() + step).clamp(
                    hadj.lower(),
                    (hadj.upper() - hadj.page_size()).max(hadj.lower()),
                );
                hadj.set_value(new_val);
            }
            gtk4::glib::Propagation::Stop
        });
        tab_scroll.add_controller(scroll_ctrl);

        let browser_menu_btn = Button::from_icon_name("view-more-symbolic");
        browser_menu_btn.set_size_request(28, 28);
        browser_menu_btn.set_tooltip_text(Some("Browser Menu (Alt+F)"));
        browser_menu_btn.set_focus_on_click(false);
        browser_menu_btn.set_css_classes(&["browser-menu-button"]);

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
        left_chrome.append(&address);

        let tabs_box = GtkBox::new(Orientation::Horizontal, 4);
        tabs_box.set_css_classes(&["tabs-box"]);
        tabs_box.set_hexpand(true);
        tabs_box.set_valign(gtk4::Align::Center);
        tabs_box.append(&tab_scroll);

        let right_chrome = GtkBox::new(Orientation::Horizontal, 4);
        right_chrome.set_css_classes(&["right-chrome"]);
        right_chrome.set_hexpand(false);
        right_chrome.set_valign(gtk4::Align::Center);
        right_chrome.append(&stop);
        right_chrome.append(&close_tab);
        right_chrome.append(&sidebar_toggle);
        right_chrome.append(&browser_menu_btn);

        let topbar = GtkBox::new(Orientation::Horizontal, 8);
        topbar.set_css_classes(&["topbar"]);
        topbar.append(&left_chrome);
        topbar.append(&tabs_box);
        topbar.append(&right_chrome);

        // Wrap topbar in a Revealer so it can slide in/out smoothly.
        let topbar_revealer = gtk4::Revealer::new();
        topbar_revealer.set_reveal_child(true);
        topbar_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        topbar_revealer.set_transition_duration(160);
        topbar_revealer.set_hexpand(true);
        topbar_revealer.set_child(Some(&topbar));

        let sidebar = GtkBox::new(Orientation::Vertical, 0);
        sidebar.set_css_classes(&["sidebar"]);
        sidebar.set_width_request(250);
        sidebar.set_vexpand(true);

        let sidebar_header = GtkBox::new(Orientation::Horizontal, 8);
        sidebar_header.set_css_classes(&["sidebar-header"]);

        let sidebar_traffic = GtkBox::new(Orientation::Horizontal, 7);
        sidebar_traffic.set_css_classes(&["traffic-dots"]);
        let sidebar_close_dot = make_traffic_button("traffic-close", "Close Window");
        let sidebar_min_dot = make_traffic_button("traffic-min", "Minimize");
        let sidebar_zoom_dot = make_traffic_button("traffic-zoom", "Maximize");
        sidebar_traffic.append(&sidebar_close_dot);
        sidebar_traffic.append(&sidebar_min_dot);
        sidebar_traffic.append(&sidebar_zoom_dot);
        sidebar_header.append(&sidebar_traffic);

        // Navigation sits beside the traffic lights — the topbar is hidden
        // in sidebar layout, so back/forward/reload live up here.
        let sidebar_nav_box = GtkBox::new(Orientation::Horizontal, 2);
        sidebar_nav_box.set_css_classes(&["nav-group", "sidebar-nav"]);
        sidebar_nav_box.set_hexpand(true);
        sidebar_nav_box.set_valign(gtk4::Align::Center);
        let sidebar_nav_back = Button::from_icon_name("go-previous-symbolic");
        sidebar_nav_back.set_size_request(26, 26);
        sidebar_nav_back.set_tooltip_text(Some("Back (Alt+Left)"));
        sidebar_nav_back.set_focus_on_click(false);
        sidebar_nav_back.set_hexpand(true);
        let sidebar_nav_forward = Button::from_icon_name("go-next-symbolic");
        sidebar_nav_forward.set_size_request(26, 26);
        sidebar_nav_forward.set_tooltip_text(Some("Forward (Alt+Right)"));
        sidebar_nav_forward.set_focus_on_click(false);
        sidebar_nav_forward.set_hexpand(true);
        let sidebar_nav_reload = Button::from_icon_name("view-refresh-symbolic");
        sidebar_nav_reload.set_size_request(26, 26);
        sidebar_nav_reload.set_tooltip_text(Some("Reload (Ctrl+R)"));
        sidebar_nav_reload.set_focus_on_click(false);
        sidebar_nav_reload.set_hexpand(true);
        sidebar_nav_box.append(&sidebar_nav_back);
        sidebar_nav_box.append(&sidebar_nav_forward);
        sidebar_nav_box.append(&sidebar_nav_reload);
        sidebar_header.append(&sidebar_nav_box);

        sidebar.append(&sidebar_header);

        let sidebar_address = gtk4::Entry::new();
        sidebar_address.set_placeholder_text(Some("Search or enter address"));
        sidebar_address.set_icon_from_icon_name(
            gtk4::EntryIconPosition::Primary,
            Some("system-search-symbolic"),
        );
        sidebar_address.set_hexpand(true);
        sidebar_address.set_vexpand(false);
        sidebar_address.set_focus_on_click(true);
        sidebar_address.set_css_classes(&["urlbar", "sidebar-urlbar"]);
        sidebar.append(&sidebar_address);

        let sidebar_content = GtkBox::new(Orientation::Vertical, 4);
        sidebar_content.set_css_classes(&["sidebar-content"]);
        sidebar_content.set_vexpand(true);
        sidebar_content.set_hexpand(true);

        let sidebar_pinned_label = Label::new(Some("Pinned"));
        sidebar_pinned_label.set_css_classes(&["sidebar-section-label"]);
        sidebar_pinned_label.set_xalign(0.0);
        sidebar_content.append(&sidebar_pinned_label);

        let sidebar_pinned_tabs = GtkBox::new(Orientation::Vertical, 2);
        sidebar_pinned_tabs.set_css_classes(&["sidebar-pinned-tabs"]);
        sidebar_pinned_tabs.set_hexpand(true);
        sidebar_content.append(&sidebar_pinned_tabs);

        let sidebar_tabs_label = Label::new(Some("Tabs"));
        sidebar_tabs_label.set_css_classes(&["sidebar-section-label"]);
        sidebar_tabs_label.set_xalign(0.0);
        sidebar_content.append(&sidebar_tabs_label);

        let sidebar_tabs = GtkBox::new(Orientation::Vertical, 2);
        sidebar_tabs.set_css_classes(&["sidebar-tabs"]);
        sidebar_tabs.set_hexpand(true);
        let sidebar_new_tab_btn = Button::new();
        sidebar_new_tab_btn.set_tooltip_text(Some("New Tab (Ctrl+T)"));
        sidebar_new_tab_btn.set_focus_on_click(false);
        sidebar_new_tab_btn.set_halign(gtk4::Align::Fill);
        sidebar_new_tab_btn.set_hexpand(true);
        sidebar_new_tab_btn.set_css_classes(&["sidebar-new-tab-btn"]);
        let new_tab_inner = GtkBox::new(Orientation::Horizontal, 8);
        let new_tab_icon = gtk4::Image::from_icon_name("list-add-symbolic");
        let new_tab_label = Label::new(Some("New Tab"));
        new_tab_label.set_xalign(0.0);
        new_tab_label.set_hexpand(true);
        new_tab_inner.append(&new_tab_icon);
        new_tab_inner.append(&new_tab_label);
        sidebar_new_tab_btn.set_child(Some(&new_tab_inner));
        sidebar_content.append(&sidebar_new_tab_btn);

        sidebar_content.append(&sidebar_tabs);
        sidebar.append(&sidebar_content);

        let sidebar_bottom = GtkBox::new(Orientation::Horizontal, 4);
        sidebar_bottom.set_css_classes(&["sidebar-bottom"]);
        sidebar_bottom.set_valign(gtk4::Align::End);
        sidebar_bottom.set_hexpand(true);

        // Workspace pill (left)
        let workspace_pill = Button::new();
        let pill_inner = GtkBox::new(Orientation::Horizontal, 6);
        let pill_label = Label::new(Some("Personal"));
        pill_inner.append(&pill_label);
        workspace_pill.set_child(Some(&pill_inner));
        workspace_pill.set_tooltip_text(Some("Personal workspace"));
        workspace_pill.set_focus_on_click(false);
        workspace_pill.set_css_classes(&["sidebar-workspace-pill"]);

        let download_btn = Button::from_icon_name("folder-download-symbolic");
        download_btn.set_tooltip_text(Some("Downloads"));
        download_btn.set_focus_on_click(false);
        download_btn.set_css_classes(&["sidebar-icon-btn"]);

        let settings_btn = Button::from_icon_name("emblem-system-symbolic");
        settings_btn.set_size_request(28, 28);
        settings_btn.set_tooltip_text(Some("Settings"));
        settings_btn.set_focus_on_click(false);
        settings_btn.set_css_classes(&["sidebar-settings-btn"]);

        let bottom_spacer = GtkBox::new(Orientation::Horizontal, 0);
        bottom_spacer.set_hexpand(true);

        sidebar_bottom.append(&workspace_pill);
        sidebar_bottom.append(&bottom_spacer);
        sidebar_bottom.append(&download_btn);
        sidebar_bottom.append(&settings_btn);

        sidebar.append(&sidebar_bottom);

        let sidebar_revealer = gtk4::Revealer::new();
        sidebar_revealer.set_reveal_child(false);
        sidebar_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideRight);
        sidebar_revealer.set_transition_duration(160);
        sidebar_revealer.set_vexpand(true);
        sidebar_revealer.set_child(Some(&sidebar));

        let sidebar_panel = GtkBox::new(Orientation::Horizontal, 0);
        sidebar_panel.set_halign(gtk4::Align::Start);
        sidebar_panel.set_vexpand(true);
        sidebar_panel.set_css_classes(&["sidebar-panel"]);
        sidebar_panel.append(&sidebar_revealer);

        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

        let progress = ProgressBar::new();
        progress.set_show_text(false);
        progress.set_fraction(0.0);
        progress.set_visible(false);
        progress.set_css_classes(&["progress-thin"]);

        // chrome_panel: floating/sliding topbar + progress bar at the top of the window
        let chrome_panel = GtkBox::new(Orientation::Vertical, 0);
        chrome_panel.set_hexpand(true);
        chrome_panel.set_css_classes(&["chrome-panel"]);
        chrome_panel.append(&topbar_revealer);
        chrome_panel.append(&progress);

        // Content overlay: webview stack is base; sidebar_panel floats on the left over webview
        let content_overlay = gtk4::Overlay::new();
        content_overlay.set_hexpand(true);
        content_overlay.set_vexpand(true);
        content_overlay.set_child(Some(&stack));
        content_overlay.add_overlay(&sidebar_panel);

        // Main layout: Vertical box where chrome_panel is at the TOP, and content_overlay is directly below it
        let main_box = GtkBox::new(Orientation::Vertical, 0);
        main_box.set_hexpand(true);
        main_box.set_vexpand(true);
        main_box.append(&chrome_panel);
        main_box.append(&content_overlay);

        // Root overlay for window: contains main_box, and allows in-app modal backdrop to overlay the whole window
        let overlay = gtk4::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        overlay.set_child(Some(&main_box));

        window.set_child(Some(&overlay));

        install_css();

        let me: Rc<Self> = Rc::new_cyclic(|weak_self| Self {
            window,
            address,
            sidebar_address,
            sidebar_nav_back,
            sidebar_nav_forward,
            sidebar_nav_reload,
            progress,
            stack,
            tab_strip,
            tab_scroll,
            browser_menu_btn: browser_menu_btn.clone(),
            sidebar_pinned_label,
            sidebar_pinned_tabs,
            sidebar_tabs_label,
            sidebar_tabs,
            overlay,
            topbar_revealer,
            sidebar_revealer,
            active_settings_modal: RefCell::new(None),
            auto_hide: Cell::new(false),
            sidebar_enabled: Cell::new(true),
            chrome_layout: Cell::new(ChromeLayout::Topbar),
            theme_pref: Cell::new(ThemePreference::System),
            tab_rows: RefCell::new(Vec::new()),
            tabs: RefCell::new(Vec::new()),
            current: RefCell::new(None),
            self_weak: weak_self.clone(),
            downloads: RefCell::new(Vec::new()),
            next_download_id: Cell::new(1),
            completed_downloads: Cell::new(0),
            downloads_popover: RefCell::new(None),
            downloads_popover_rows: RefCell::new(None),
            active_downloads_page: RefCell::new(None),
            downloads_page_list: RefCell::new(None),
            downloads_page_search: RefCell::new(None),
            download_pill: RefCell::new(None),
            pending_save_dialogs: RefCell::new(Vec::new()),
            settings: RefCell::new(config::load_settings()),
            downloads_btn: download_btn.clone(),
            find_bar: RefCell::new(None),
            fullscreen: Cell::new(false),
            pre_fullscreen: Cell::new((0, 0)),
        });

        me.apply_chrome_layout();
        me.apply_theme();
        me.wire_system_theme_listener();
        me.wire_navigation(&back, &forward, &reload, &stop);
        me.wire_navigation(
            &me.sidebar_nav_back,
            &me.sidebar_nav_forward,
            &me.sidebar_nav_reload,
            &stop,
        );
        me.wire_address();
        me.wire_keyboard();
        me.wire_window_controls(&close_dot, &min_dot, &zoom_dot);
        me.wire_window_controls(&sidebar_close_dot, &sidebar_min_dot, &sidebar_zoom_dot);
        me.wire_sidebar_toggle(&sidebar_toggle);
        me.wire_browser_menu(&browser_menu_btn);
        me.wire_new_tab(&sidebar_new_tab_btn);
        me.wire_close_tab(&close_tab);
        me.wire_settings(&settings_btn);
        me.wire_hover();
        me.wire_download_ui(&download_btn);
        // Restore the previous session's tabs (falls back to one new tab).
        me.restore_session();
        // `download-started` fires on the NETWORK SESSION in the GTK4 API —
        // per-webview connects fail with a GObject-CRITICAL and never run.
        // Needs a live webview, so wire it after session restore.
        me.wire_session_downloads();

        me
    }

    /// Open a new tab for a `target=_blank` navigation and navigate it.
    pub fn open_url_in_new_tab(&self, uri: &str) {
        let tab = self.add_tab(false);
        tab.navigate_to(uri);
        self.save_session();
    }

    pub fn add_tab(&self, blank: bool) -> Rc<Tab> {
        let tab = if blank {
            Tab::new_blank()
        } else {
            Tab::new(config::HOME_PAGE)
        };
        let page_name = format!("page-{}", self.tabs.borrow().len());

        let me_weak: Weak<Self> = self.self_weak.clone();
        let owning_tab: Rc<Tab> = Rc::clone(&tab);
        tab.on_event(move |ev| {
            let Some(me) = me_weak.upgrade() else { return };
            let cur = me.current.borrow().clone();
            let current_ref = cur.as_deref().map(|t| t as &Tab);
            me.on_tab_event(current_ref, Some(&owning_tab), ev);
        });

        self.stack
            .add_titled(tab.widget(), Some(&page_name), "New Tab");

        // If blank, load the new-tab page now that the widget is in the tree.
        if blank {
            tab.load_new_tab_page(self.is_dark_active());
        }

        let row = TabRow::new_from_window(self, Rc::clone(&tab));
        self.tab_rows.borrow_mut().push(Rc::clone(&row));

        self.tabs.borrow_mut().push(Rc::clone(&tab));
        self.activate_tab(Rc::clone(&tab));

        // Put the cursor in the address/search input so typing works
        // immediately in a fresh tab — but only for blank (user-initiated)
        // tabs; session restore and _blank links keep focus where it was.
        if blank {
            let me_focus = self.self_weak.clone();
            gtk4::glib::idle_add_local_once(move || {
                if let Some(me) = me_focus.upgrade() {
                    me.focus_address();
                }
            });
        }
        self.save_session();

        let scroll = self.tab_scroll.clone();
        gtk4::glib::idle_add_local_once(move || {
            let hadj = scroll.hadjustment();
            hadj.set_value((hadj.upper() - hadj.page_size()).max(hadj.lower()));
        });
        tab
    }

    fn apply_chrome_layout(&self) {
        match self.chrome_layout.get() {
            ChromeLayout::Topbar => {
                self.topbar_revealer.set_visible(true);
                self.topbar_revealer.set_reveal_child(!self.auto_hide.get());
                self.sidebar_revealer.set_reveal_child(false);
                self.sidebar_revealer.set_visible(false);
            }
            ChromeLayout::Sidebar => {
                self.progress.set_visible(false);
                self.topbar_revealer.set_reveal_child(false);
                self.topbar_revealer.set_visible(false);
                self.sidebar_revealer.set_visible(true);
                self.sidebar_revealer
                    .set_reveal_child(!self.auto_hide.get());
            }
        }
    }

    pub fn set_chrome_layout(&self, layout: ChromeLayout) {
        self.chrome_layout.set(layout);
        if layout == ChromeLayout::Sidebar {
            self.sidebar_enabled.set(true);
        }
        self.apply_chrome_layout();
        self.resort_tabs();
        if let Some(tab) = self.current_tab() {
            let url = tab
                .webview
                .uri()
                .unwrap_or_else(|| tab.url.borrow().clone());
            self.update_auto_hide_for_url(&url);
        }
    }

    fn remove_tab_row(&self, row: &TabRow) {
        // The row lives in exactly one parent; Gtk.Box::remove emits a
        // Gtk-CRITICAL if the widget isn't a child, so check first.
        let parent = row.container.parent();
        if let Some(p) = parent {
            if p.is::<gtk4::Box>() {
                p.downcast::<gtk4::Box>().unwrap().remove(&row.container);
            }
        }
    }

    fn sidebar_tab_label_for(&self, tab: &Tab) -> String {
        let title = tab.title.borrow().clone();
        if !title.is_empty() {
            self.short_title(&title)
        } else {
            self.short_url(&tab.url.borrow())
        }
    }

    fn set_address_text(&self, text: &str) {
        if self.address.text() != text {
            self.address.set_text(text);
        }
        if self.sidebar_address.text() != text {
            self.sidebar_address.set_text(text);
        }
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
        self.set_address_text(&self.address_text_for(&active_url));
        self.update_auto_hide_for_url(&active_url);
        *self.current.borrow_mut() = Some(tab);
    }

    fn current_tab(&self) -> Option<Rc<Tab>> {
        self.current.borrow().as_ref().map(Rc::clone)
    }

    fn on_tab_event(&self, _current: Option<&Tab>, owner: Option<&Rc<Tab>>, ev: TabEvent) {
        match ev {
            TabEvent::UriChanged(uri) => {
                let display_uri = self.address_text_for(&uri);
                if let Some(cur) = self.current_tab() {
                    if owner.is_some_and(|o| Rc::ptr_eq(&cur, o)) {
                        self.set_address_text(&display_uri);
                        self.update_auto_hide_for_url(&uri);
                    }
                }
                if let Some(tab) = owner {
                    if let Some(row) = self.find_row_for(tab) {
                        // Belt-and-suspenders: force label to "New Tab" for any
                        // data:/about: URI so the raw URL never leaks into the tab pill.
                        row.title_label.set_text(&self.short_url(&uri));
                        // New document → old icon is stale; show the letter tile
                        // until WebKit publishes the new favicon.
                        row.reset_favicon();
                        row.update_favicon(tab);
                    }
                }
                // Re-order if pin state changed via load (rare).
                self.resort_tabs();
                self.save_session();
            }
            TabEvent::TitleChanged(title) => {
                let short = self.short_title(&title);
                if let Some(tab) = owner {
                    if let Some(row) = self.find_row_for(tab) {
                        row.title_label.set_tooltip_text(Some(&short));
                        row.update_favicon(tab);
                        let label = if self.chrome_layout.get() == ChromeLayout::Sidebar {
                            self.sidebar_tab_label_for(tab)
                        } else {
                            self.tab_label_for(tab, tab.pinned.get())
                        };
                        row.title_label.set_text(&label);
                    }
                }
            }
            TabEvent::LoadingChanged(loading) => {
                // Only reflect loading state of the visible tab.
                if let Some(cur) = self.current_tab() {
                    if !owner.is_some_and(|o| Rc::ptr_eq(&cur, o)) {
                        return;
                    }
                } else {
                    return;
                }
                let show_progress = loading && self.chrome_layout.get() == ChromeLayout::Topbar;
                self.progress.set_visible(show_progress);
                if show_progress {
                    self.progress.pulse();
                } else {
                    self.progress.set_fraction(0.0);
                }
            }
            TabEvent::LoadFailed { uri, message } => {
                // Render the error page in the tab that actually failed —
                // never in the current tab (that used to cancel an unrelated
                // in-flight load and cause an error-page cascade).
                if let Some(tab) = owner {
                    self.render_error_page_in(tab, &uri, &message);
                }
            }
            TabEvent::FaviconReady(data) => {
                if let Some(tab) = owner {
                    if let Some(row) = self.find_row_for(tab) {
                        row.set_favicon_rgba(&data);
                    }
                }
            }
            TabEvent::PinnedChanged(_pinned) => {
                self.resort_tabs();
            }
            TabEvent::NewWindowRequested(uri) => {
                if uri.is_empty() {
                    return;
                }
                self.open_url_in_new_tab(&uri);
            }
            TabEvent::FullscreenChanged(active) => {
                if owner.is_some_and(|o| {
                    self.current_tab().is_some_and(|c| Rc::ptr_eq(&c, o))
                }) {
                    self.set_video_fullscreen(active);
                }
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

    fn resort_tabs(&self) {
        let rows: Vec<Rc<TabRow>> = self.tab_rows.borrow().iter().cloned().collect();
        let mut pinned: Vec<Rc<TabRow>> = Vec::new();
        let mut unpinned: Vec<Rc<TabRow>> = Vec::new();
        for row in &rows {
            let is_pinned = row
                .tab
                .upgrade()
                .map(|tab| tab.pinned.get())
                .unwrap_or(false);
            if is_pinned {
                pinned.push(Rc::clone(row));
            } else {
                unpinned.push(Rc::clone(row));
            }
        }

        let sidebar_mode = self.chrome_layout.get() == ChromeLayout::Sidebar;
        let unpinned_len = unpinned.len().max(1);
        let tab_width = if unpinned_len <= 3 {
            130
        } else if unpinned_len <= 5 {
            110
        } else if unpinned_len <= 8 {
            85
        } else if unpinned_len <= 12 {
            68
        } else {
            50
        };
        let has_pinned = !pinned.is_empty();
        let has_unpinned = !unpinned.is_empty();

        let mut ordered = pinned;
        ordered.extend(unpinned);
        for row in &ordered {
            self.remove_tab_row(row);
        }

        for row in &ordered {
            let is_pinned = row
                .tab
                .upgrade()
                .map(|tab| tab.pinned.get())
                .unwrap_or(false);
            if sidebar_mode {
                row.container.remove_css_class("tab-row");
                row.container.remove_css_class("pinned-tab");
                row.container.add_css_class("sidebar-tab-row");
                row.container.set_hexpand(true);
                row.container.set_width_request(0);
                row.title_label.set_xalign(0.0);
                row.close_btn.set_visible(true);
                if is_pinned {
                    row.container.add_css_class("sidebar-pinned-tab-row");
                    self.sidebar_pinned_tabs.append(&row.container);
                } else {
                    self.sidebar_tabs.append(&row.container);
                }
                if let Some(tab) = row.tab.upgrade() {
                    row.title_label.set_text(&self.sidebar_tab_label_for(&tab));
                    row.update_favicon(&tab);
                }
            } else {
                row.container.remove_css_class("sidebar-tab-row");
                row.container.remove_css_class("sidebar-pinned-tab-row");
                row.container.add_css_class("tab-row");
                row.container.set_hexpand(false);
                if is_pinned {
                    row.container.add_css_class("pinned-tab");
                    row.container.set_width_request(28);
                    row.title_label.set_xalign(0.5);
                    row.close_btn.set_visible(false);
                } else {
                    row.container.remove_css_class("pinned-tab");
                    row.container.set_width_request(tab_width);
                    row.title_label.set_xalign(0.0);
                    row.close_btn.set_visible(true);
                }
                if let Some(tab) = row.tab.upgrade() {
                    row.title_label
                        .set_text(&self.tab_label_for(&tab, is_pinned));
                }
                self.tab_strip.append(&row.container);
            }
        }

        self.sidebar_pinned_label.set_visible(has_pinned);
        self.sidebar_pinned_tabs.set_visible(has_pinned);
        self.sidebar_tabs_label.set_visible(has_unpinned);
        self.sidebar_tabs.set_visible(has_unpinned);
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

    fn render_error_page_in(&self, tab: &Rc<Tab>, uri: &str, message: &str) {
        let html = config::ERROR_PAGE_HTML
            .replace("{uri}", &html_escape(uri))
            .replace("{message}", &html_escape(message));
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
        for entry in [self.address.clone(), self.sidebar_address.clone()] {
            let me_weak = self.self_weak.clone();
            entry.connect_activate(move |e| {
                if let Some(me) = me_weak.upgrade() {
                    let text = e.text().to_string();
                    me.set_address_text(&text);
                    if let Some(tab) = me.current_tab() {
                        tab.navigate(&text);
                    }
                }
            });
        }
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

    fn wire_browser_menu(&self, btn: &Button) {
        let me_weak = self.self_weak.clone();
        btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.show_browser_menu();
            }
        });

        let action_new_tab = gtk4::gio::SimpleAction::new("browser-new-tab", None);
        let me_weak = self.self_weak.clone();
        action_new_tab.connect_activate(move |_, _| {
            if let Some(me) = me_weak.upgrade() {
                me.add_tab(true);
            }
        });
        self.window.add_action(&action_new_tab);

        let action_reload = gtk4::gio::SimpleAction::new("browser-reload", None);
        let me_weak = self.self_weak.clone();
        action_reload.connect_activate(move |_, _| {
            if let Some(me) = me_weak.upgrade() {
                if let Some(tab) = me.current_tab() {
                    tab.reload();
                }
            }
        });
        self.window.add_action(&action_reload);

        let action_layout = gtk4::gio::SimpleAction::new("browser-layout", None);
        let me_weak = self.self_weak.clone();
        action_layout.connect_activate(move |_, _| {
            if let Some(me) = me_weak.upgrade() {
                me.toggle_sidebar();
            }
        });
        self.window.add_action(&action_layout);

        let action_settings = gtk4::gio::SimpleAction::new("browser-settings", None);
        let me_weak = self.self_weak.clone();
        action_settings.connect_activate(move |_, _| {
            if let Some(me) = me_weak.upgrade() {
                me.open_settings_dialog();
            }
        });
        self.window.add_action(&action_settings);
    }

    fn show_browser_menu(&self) {
        let actions = gtk4::gio::Menu::new();
        actions.append(Some("New Tab  (Ctrl+T)"), Some("win.browser-new-tab"));
        actions.append(Some("Reload  (Ctrl+R)"), Some("win.browser-reload"));
        actions.append(Some("Switch Layout  (Ctrl+B)"), Some("win.browser-layout"));

        let settings = gtk4::gio::Menu::new();
        settings.append(Some("Settings  (Ctrl+,)"), Some("win.browser-settings"));

        let menu = gtk4::gio::Menu::new();
        menu.append_section(None, &actions);
        menu.append_section(None, &settings);

        let popover = PopoverMenu::from_model(Some(&menu));
        popover.set_parent(&self.browser_menu_btn);
        popover.set_has_arrow(false);
        popover.popup();
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

    pub fn is_system_dark() -> bool {
        if let Some(settings) = gtk4::Settings::default() {
            if settings.is_gtk_application_prefer_dark_theme() {
                return true;
            }
            if let Some(theme_name) = settings.gtk_theme_name() {
                let lower = theme_name.to_lowercase();
                if lower.contains("dark") || lower.contains("black") {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_dark_active(&self) -> bool {
        match self.theme_pref.get() {
            ThemePreference::Dark => true,
            ThemePreference::Light => false,
            ThemePreference::System => Self::is_system_dark(),
        }
    }

    pub fn apply_theme(&self) {
        let is_dark = self.is_dark_active();

        if is_dark {
            self.window.remove_css_class("light");
            self.window.add_css_class("dark");
        } else {
            self.window.remove_css_class("dark");
            self.window.add_css_class("light");
        }

        // Live update any open new tab pages with updated theme via JS (instant, zero reload)
        for tab in self.tabs.borrow().iter() {
            let url = tab.url.borrow().clone();
            if url.is_empty() || url == "about:newtab" || url.starts_with("data:text/html") {
                tab.update_theme(is_dark);
            }
        }
    }

    pub fn set_theme_preference(&self, pref: ThemePreference) {
        self.theme_pref.set(pref);
        self.apply_theme();
    }

    fn set_download_dir(&self, dir: &str) {
        self.settings.borrow_mut().download_dir = dir.to_string();
        config::save_settings(&self.settings.borrow());
    }

    fn set_ask_download_location(&self, ask: bool) {
        self.settings.borrow_mut().ask_download_location = ask;
        config::save_settings(&self.settings.borrow());
    }

    fn wire_system_theme_listener(&self) {
        if let Some(settings) = gtk4::Settings::default() {
            let me_weak = self.self_weak.clone();
            settings.connect_gtk_application_prefer_dark_theme_notify(move |_| {
                if let Some(me) = me_weak.upgrade() {
                    if me.theme_pref.get() == ThemePreference::System {
                        me.apply_theme();
                    }
                }
            });
            let me_weak = self.self_weak.clone();
            settings.connect_gtk_theme_name_notify(move |_| {
                if let Some(me) = me_weak.upgrade() {
                    if me.theme_pref.get() == ThemePreference::System {
                        me.apply_theme();
                    }
                }
            });
        }
    }

    pub fn set_sidebar_enabled(&self, enabled: bool) {
        self.sidebar_enabled.set(enabled);
        if !enabled {
            if self.chrome_layout.get() == ChromeLayout::Sidebar {
                self.set_chrome_layout(ChromeLayout::Topbar);
            }
            self.sidebar_revealer.set_reveal_child(false);
            self.sidebar_revealer.set_visible(false);
            self.apply_chrome_layout();
            self.resort_tabs();
        } else if self.chrome_layout.get() == ChromeLayout::Sidebar {
            self.sidebar_revealer.set_visible(true);
            self.sidebar_revealer.set_reveal_child(true);
        }
    }

    fn toggle_sidebar(&self) {
        let next = match self.chrome_layout.get() {
            ChromeLayout::Topbar => ChromeLayout::Sidebar,
            ChromeLayout::Sidebar => ChromeLayout::Topbar,
        };
        self.set_chrome_layout(next);
    }

    // ── Auto-hide chrome (Zen-style) ─────────────────────────────────────────

    /// Called on every URL change (navigation + tab switch) to decide whether
    /// the topbar should auto-hide. New-tab / blank pages always show it.
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

    fn enable_auto_hide(&self) {
        self.auto_hide.set(true);
        match self.chrome_layout.get() {
            ChromeLayout::Topbar => self.topbar_revealer.set_reveal_child(false),
            ChromeLayout::Sidebar => self.sidebar_revealer.set_reveal_child(false),
        }
    }

    fn disable_auto_hide(&self) {
        self.auto_hide.set(false);
        match self.chrome_layout.get() {
            ChromeLayout::Topbar => self.topbar_revealer.set_reveal_child(true),
            ChromeLayout::Sidebar => self.sidebar_revealer.set_reveal_child(true),
        }
    }

    /// Wire a window-level motion capture controller that:
    ///   • shows the topbar when the pointer is within 8 px of the top edge,
    ///     and hides it again once the pointer moves > 54 px below the top.
    ///   • shows the sidebar when the pointer is within 8 px of the left edge,
    ///     and hides it again once the pointer moves > 230 px away from the left edge.
    ///   • in Sidebar (vertical tabs) layout: shows the sidebar when the
    ///     pointer is within 8 px of the left edge, hides it again once the
    ///     pointer moves > 260 px away from the left edge.
    fn wire_hover(&self) {
        let motion = gtk4::EventControllerMotion::new();
        // Capture phase: we see events before WebKit or any child widget.
        motion.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let me_weak = self.self_weak.clone();
        motion.connect_motion(move |_, x, y| {
            let Some(me) = me_weak.upgrade() else { return };
            if !me.auto_hide.get() {
                return;
            }
            match me.chrome_layout.get() {
                ChromeLayout::Topbar => {
                    if y < 8.0 {
                        me.topbar_revealer.set_reveal_child(true);
                    } else if y > 54.0 {
                        me.topbar_revealer.set_reveal_child(false);
                    }
                }
                ChromeLayout::Sidebar => {
                    // Sidebar is 250 px wide; reveal near the left edge,
                    // hide again once the pointer moves well into content.
                    if x < 8.0 {
                        me.sidebar_revealer.set_reveal_child(true);
                    } else if x > 260.0 {
                        me.sidebar_revealer.set_reveal_child(false);
                    }
                }
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
                (true, false, false, Key::comma) => Some(WinCmd::OpenSettings),
                (false, false, true, Key::Left) => Some(WinCmd::Back),
                (false, false, true, Key::Right) => Some(WinCmd::Forward),
                (false, false, true, Key::f) => Some(WinCmd::OpenBrowserMenu),
                (false, false, false, Key::Escape) if me.fullscreen.get() => {
                    Some(WinCmd::ToggleFullscreen)
                }
                (false, false, false, Key::F5) => Some(WinCmd::Reload),
                (false, false, false, Key::Escape) => {
                    if me.active_settings_modal.borrow().is_some() {
                        me.close_settings_dialog();
                        None
                    } else {
                        Some(WinCmd::Stop)
                    }
                }
                (true, false, false, Key::t) => Some(WinCmd::NewTab),
                (true, false, false, Key::f) => Some(WinCmd::FindOpen),
                (false, false, false, Key::F3) => Some(WinCmd::FindNext),
                (true, false, false, Key::g) => Some(WinCmd::FindNext),
                (true, true, false, Key::g) => Some(WinCmd::FindPrev),
                (true, false, false, Key::equal) => Some(WinCmd::ZoomIn),
                (true, false, false, Key::plus) => Some(WinCmd::ZoomIn),
                (true, false, false, Key::minus) => Some(WinCmd::ZoomOut),
                (true, false, false, Key::_0) => Some(WinCmd::ZoomReset),
                (false, false, false, Key::F11) => Some(WinCmd::ToggleFullscreen),
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

    /// Focus the layout-appropriate address input and select its contents.
    fn focus_address(&self) {
        let entry = if self.chrome_layout.get() == ChromeLayout::Sidebar {
            &self.sidebar_address
        } else {
            &self.address
        };
        entry.grab_focus();
        entry.select_region(0, -1);
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
            WinCmd::FocusAddress => self.focus_address(),
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
            WinCmd::OpenBrowserMenu => self.show_browser_menu(),
            WinCmd::OpenSettings => self.open_settings_dialog(),
            WinCmd::FindOpen => self.open_find_bar(),
            WinCmd::FindNext => self.find_next(),
            WinCmd::FindPrev => self.find_prev(),
            WinCmd::ZoomIn => {
                if let Some(t) = self.current_tab() {
                    t.zoom_in();
                }
            }
            WinCmd::ZoomOut => {
                if let Some(t) = self.current_tab() {
                    t.zoom_out();
                }
            }
            WinCmd::ZoomReset => {
                if let Some(t) = self.current_tab() {
                    t.zoom_reset();
                }
            }
            WinCmd::ToggleFullscreen => self.toggle_fullscreen_window(),
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

    // ── Find in page ─────────────────────────────────────────────────────────

    fn open_find_bar(&self) {
        let reuse = self.find_bar.borrow().is_some();
        if !reuse {
            let container = GtkBox::new(Orientation::Horizontal, 6);
            container.set_css_classes(&["find-bar"]);
            container.set_halign(gtk4::Align::End);
            container.set_valign(gtk4::Align::Start);
            container.set_margin_top(6);
            container.set_margin_end(10);

            let entry = gtk4::Entry::new();
            entry.set_placeholder_text(Some("Find in page"));
            entry.set_width_request(220);
            entry.set_css_classes(&["find-entry"]);

            let label = Label::new(None);
            label.set_css_classes(&["find-label"]);

            let prev_btn = Button::from_icon_name("go-up-symbolic");
            prev_btn.set_tooltip_text(Some("Previous match (Shift+Enter / Ctrl+Shift+G)"));
            prev_btn.set_css_classes(&["flat"]);
            let next_btn = Button::from_icon_name("go-down-symbolic");
            next_btn.set_tooltip_text(Some("Next match (Enter / Ctrl+G)"));
            next_btn.set_css_classes(&["flat"]);
            let close_btn = Button::from_icon_name("window-close-symbolic");
            close_btn.set_css_classes(&["flat"]);

            container.append(&entry);
            container.append(&label);
            container.append(&prev_btn);
            container.append(&next_btn);
            container.append(&close_btn);

            self.overlay.add_overlay(&container);

            *self.find_bar.borrow_mut() = Some(FindBar {
                container: container.clone(),
                entry: entry.clone(),
            });

            let me_weak = self.self_weak.clone();
            entry.connect_activate(move |entry| {
                let Some(me) = me_weak.upgrade() else { return };
                let text = entry.text().to_string();
                if text.is_empty() {
                    return;
                }
                if let Some(t) = me.current_tab() {
                    t.find(&text, true);
                }
            });
            let me_weak2 = self.self_weak.clone();
            entry.connect_changed(move |entry| {
                let Some(me) = me_weak2.upgrade() else { return };
                let text = entry.text().to_string();
                if let Some(t) = me.current_tab() {
                    if text.is_empty() {
                        t.find_done();
                    } else {
                        t.find(&text, true);
                    }
                }
            });
            let me_weak3 = self.self_weak.clone();
            next_btn.connect_clicked(move |_| {
                if let Some(me) = me_weak3.upgrade() {
                    me.find_next();
                }
            });
            let me_weak4 = self.self_weak.clone();
            prev_btn.connect_clicked(move |_| {
                if let Some(me) = me_weak4.upgrade() {
                    me.find_prev();
                }
            });
            let me_weak5 = self.self_weak.clone();
            close_btn.connect_clicked(move |_| {
                if let Some(me) = me_weak5.upgrade() {
                    me.close_find_bar();
                }
            });

            // Esc closes when the entry has focus.
            let esc = EventControllerKey::new();
            let me_weak6 = self.self_weak.clone();
            esc.connect_key_pressed(move |_ctrl, key, _kc, _st| {
                if key == Key::Escape {
                    if let Some(me) = me_weak6.upgrade() {
                        me.close_find_bar();
                        return gtk4::glib::Propagation::Stop;
                    }
                }
                gtk4::glib::Propagation::Proceed
            });
            entry.add_controller(esc);
        }

        if let Some(fb) = self.find_bar.borrow().as_ref() {
            fb.container.set_visible(true);
            fb.entry.grab_focus();
        }
    }

    fn close_find_bar(&self) {
        if let Some(fb) = self.find_bar.borrow().as_ref() {
            if let Some(t) = self.current_tab() {
                t.find_done();
            }
            fb.container.set_visible(false);
        }
        if let Some(win) = self.window.default_widget() {
            let _ = win;
        }
    }

    fn find_next(&self) {
        if let Some(t) = self.current_tab() {
            t.find_next("");
        }
    }

    fn find_prev(&self) {
        if let Some(t) = self.current_tab() {
            t.find_prev("");
        }
    }

    // ── Fullscreen (video player + F11) ────────────────────────────────────

    fn set_video_fullscreen(&self, active: bool) {
        let w = &self.window;
        if active && !self.fullscreen.get() {
            let width = w.default_width();
            let height = w.default_height();
            self.pre_fullscreen.set((width, height));
            self.fullscreen.set(true);
            unsafe {
                ffi::gtk_window_fullscreen(w.as_ptr() as *mut std::os::raw::c_void);
            }
        } else if !active && self.fullscreen.get() {
            self.fullscreen.set(false);
            unsafe {
                ffi::gtk_window_unfullscreen(w.as_ptr() as *mut std::os::raw::c_void);
            }
        }
    }

    fn toggle_fullscreen_window(&self) {
        self.set_video_fullscreen(!self.fullscreen.get());
    }

    // ── Session save / restore ─────────────────────────────────────────────

    fn session_path() -> std::path::PathBuf {
        crate::adblock::data_dir_shared().join("session.json")
    }

    /// Persist open tab URIs (called on tab open/close/navigation).
    fn save_session(&self) {
        let uris: Vec<String> = self
            .tabs
            .borrow()
            .iter()
            .map(|t| t.session_uri())
            .filter(|u| {
                !u.is_empty()
                    && u != "about:newtab"
                    && !u.starts_with("data:")
                    && u != "about:blank"
            })
            .collect();
        let json = serde_json::to_string(&uris).unwrap_or_else(|_| "[]".into());
        let path = Self::session_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, json.as_bytes());
    }

    /// Restore the last session's tabs, if any. Falls back to one new tab.
    fn restore_session(&self) {
        let uris: Vec<String> = std::fs::read_to_string(Self::session_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let uris: Vec<String> = uris
            .into_iter()
            .filter(|u| !u.is_empty() && !u.starts_with("data:"))
            .collect();
        if uris.is_empty() {
            self.add_tab(true);
            return;
        }
        eprintln!("[session] restoring {} tab(s)", uris.len());
        for uri in &uris {
            let tab = self.add_tab(false);
            tab.navigate_to(uri);
        }
    }

    // ── Downloads ──────────────────────────────────────────────────────────

    fn downloads_dir() -> std::path::PathBuf {
        std::env::var_os("XDG_DOWNLOAD_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(|h| std::path::PathBuf::from(h).join("Downloads"))
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            })
    }

    /// Wire `download-started` once, on the shared network session. The
    /// handler lives on the session (app-wide), so any live webview works
    /// as the hook source; the closure only carries a Weak.
    fn wire_session_downloads(&self) {
        let Some(tab) = self.current_tab() else {
            eprintln!("[download] no webview to hook session downloads");
            return;
        };
        let me_weak = self.self_weak.clone();
        tab.webview.connect_session_downloads(move |download| {
            if let Some(me) = me_weak.upgrade() {
                me.on_download_started(download);
            }
        });
    }

    /// Wire the download button: click opens the full downloads page,
    /// right-click opens the quick popover.
    fn wire_download_ui(&self, btn: &Button) {
        btn.set_tooltip_text(Some("Downloads (right-click: quick list)"));
        let me_weak = self.self_weak.clone();
        btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.toggle_downloads_page();
            }
        });
        let right = GestureClick::new();
        right.set_button(gtk4::gdk::BUTTON_SECONDARY);
        let me_weak2 = self.self_weak.clone();
        right.connect_pressed(move |_, _, _, _| {
            if let Some(me) = me_weak2.upgrade() {
                me.show_downloads_popover();
            }
        });
        btn.add_controller(right);
    }

    /// Toggle the full-page downloads view (Brave-style native overlay).
    fn toggle_downloads_page(&self) {
        if self.active_downloads_page.borrow().is_some() {
            self.close_downloads_page();
        } else {
            self.open_downloads_page();
        }
    }

    /// Show the full-page downloads view covering the content area.
    fn open_downloads_page(&self) {
        if self.active_downloads_page.borrow().is_some() {
            self.rebuild_downloads_page();
            return;
        }
        let backdrop = GtkBox::new(Orientation::Vertical, 0);
        backdrop.set_hexpand(true);
        backdrop.set_vexpand(true);
        backdrop.set_halign(gtk4::Align::Fill);
        backdrop.set_valign(gtk4::Align::Fill);
        backdrop.set_css_classes(&["downloads-page-backdrop"]);

        // Card fills most of the window.
        let card = GtkBox::new(Orientation::Vertical, 0);
        card.set_css_classes(&["downloads-page-card"]);
        card.set_hexpand(true);
        card.set_vexpand(true);
        card.set_margin_top(24);
        card.set_margin_bottom(24);
        card.set_margin_start(32);
        card.set_margin_end(32);
        backdrop.append(&card);

        // Header: title + close.
        let header = GtkBox::new(Orientation::Horizontal, 12);
        header.set_css_classes(&["downloads-page-header"]);
        let title = Label::new(Some("Downloads"));
        title.set_css_classes(&["downloads-page-title"]);
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let close_btn = Button::from_icon_name("window-close-symbolic");
        close_btn.set_css_classes(&["settings-modal-close-btn"]);
        close_btn.set_valign(gtk4::Align::Center);
        close_btn.set_focus_on_click(false);
        header.append(&title);
        header.append(&close_btn);
        card.append(&header);

        // Toolbar: search + Clear all.
        let toolbar = GtkBox::new(Orientation::Horizontal, 8);
        toolbar.set_css_classes(&["downloads-page-toolbar"]);
        let search = gtk4::Entry::new();
        search.set_placeholder_text(Some("Search download history"));
        search.set_hexpand(true);
        search.set_valign(gtk4::Align::Center);
        search.set_css_classes(&["downloads-page-search"]);
        let clear_btn = Button::with_label("Clear all");
        clear_btn.set_css_classes(&["settings-dropdown-btn"]);
        clear_btn.set_valign(gtk4::Align::Center);
        clear_btn.set_halign(gtk4::Align::End);
        toolbar.append(&search);
        toolbar.append(&clear_btn);
        card.append(&toolbar);

        // Scrollable rows area.
        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();
        let list = GtkBox::new(Orientation::Vertical, 0);
        list.set_css_classes(&["downloads-page-list"]);
        scroll.set_child(Some(&list));
        card.append(&scroll);

        self.overlay.add_overlay(&backdrop);

        // Close wiring.
        let me_close = self.self_weak.clone();
        close_btn.connect_clicked(move |_| {
            if let Some(me) = me_close.upgrade() {
                me.close_downloads_page();
            }
        });
        let me_gesture = self.self_weak.clone();
        let card_c = card.clone();
        let backdrop_c = backdrop.clone();
        let g = GestureClick::new();
        g.connect_pressed(move |_, _, x, y| {
            if let Some(me) = me_gesture.upgrade() {
                if let Some(rect) = card_c.compute_bounds(&backdrop_c) {
                    let pt = gtk4::graphene::Point::new(x as f32, y as f32);
                    if !rect.contains_point(&pt) {
                        me.close_downloads_page();
                    }
                }
            }
        });
        backdrop.add_controller(g);
        let me_esc = self.self_weak.clone();
        let key = gtk4::EventControllerKey::new();
        key.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                if let Some(me) = me_esc.upgrade() {
                    me.close_downloads_page();
                }
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        backdrop.add_controller(key);

        // Search + Clear wiring.
        let me_search = self.self_weak.clone();
        search.connect_changed(move |_| {
            if let Some(me) = me_search.upgrade() {
                me.rebuild_downloads_page();
            }
        });
        let me_clear = self.self_weak.clone();
        clear_btn.connect_clicked(move |_| {
            if let Some(me) = me_clear.upgrade() {
                config::clear_download_entries();
                me.rebuild_downloads_page();
            }
        });

        *self.active_downloads_page.borrow_mut() = Some(backdrop);
        // Stash the list + search widgets so ticks/search can rebuild rows.
        *self.downloads_page_list.borrow_mut() = Some(list);
        *self.downloads_page_search.borrow_mut() = Some(search);
        self.rebuild_downloads_page();
    }

    fn close_downloads_page(&self) {
        if let Some(page) = self.active_downloads_page.borrow_mut().take() {
            self.overlay.remove_overlay(&page);
            *self.downloads_page_list.borrow_mut() = None;
            *self.downloads_page_search.borrow_mut() = None;
        }
    }

    /// Rebuild the rows in the open downloads page (live + history).
    fn rebuild_downloads_page(&self) {
        let Some(list) = self.downloads_page_list.borrow().as_ref().map(|l| l.clone()) else {
            return;
        };
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let filter = self
            .downloads_page_search
            .borrow()
            .as_ref()
            .map(|e| e.text().to_string().to_lowercase())
            .unwrap_or_default();
        let matches = |name: &str, dest: &str| {
            filter.is_empty()
                || name.to_lowercase().contains(&filter)
                || dest.to_lowercase().contains(&filter)
        };

        let mut rows = 0;

        // Active downloads on top.
        let live: Vec<(String, String, f64, u64)> = self.downloads.borrow().iter().map(|d| {
            let pct = unsafe { ffi::webkit_download_get_estimated_progress(d.download) } * 100.0;
            (d.name.clone(), d.dest.clone(), pct, d.id)
        }).collect();
        if !live.is_empty() {
            list.append(&Self::downloads_section_label("Active"));
            for (name, _dest, pct, id) in live {
                let bar = gtk4::LevelBar::builder()
                    .min_value(0.0)
                    .max_value(100.0)
                    .value(pct)
                    .valign(gtk4::Align::Center)
                    .build();
                bar.set_width_request(140);
                let (row, _) = Self::download_row(
                    &name,
                    &format!("{pct:.0}% — downloading"),
                    Some(&bar),
                    id,
                    self.self_weak.clone(),
                );
                list.append(&row);
                rows += 1;
            }
        }

        // Persisted history grouped by day.
        let entries = config::load_download_entries();
        let mut last_label = String::new();
        for e in entries.iter().rev() {
            if !matches(&e.name, &e.dest) {
                continue;
            }
            let label = Self::day_label(e.when);
            if label != last_label {
                list.append(&Self::downloads_section_label(&label));
                last_label = label;
            }
            list.append(&Self::history_row(e, self.self_weak.clone()));
            rows += 1;
        }

        if rows == 0 {
            let empty = Label::new(Some("No downloads yet"));
            empty.set_css_classes(&["downloads-empty"]);
            empty.set_vexpand(true);
            list.append(&empty);
        }
    }

    fn downloads_section_label(text: &str) -> Label {
        let l = Label::new(Some(text));
        l.set_css_classes(&["downloads-section-label"]);
        l.set_xalign(0.0);
        l.set_margin_top(14);
        l.set_margin_bottom(4);
        l.set_margin_start(4);
        l
    }

    /// History row with Open / Show-in-folder / Remove actions.
    fn history_row(e: &config::DownloadEntry, me_weak: Weak<BrowserWindow>) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 10);
        row.set_css_classes(&["downloads-page-row"]);
        row.set_margin_top(4);
        row.set_margin_bottom(4);

        let icon = gtk4::Image::from_icon_name("application-x-executable-symbolic");
        icon.set_css_classes(&["downloads-file-icon"]);
        icon.set_valign(gtk4::Align::Center);
        row.append(&icon);

        let info = GtkBox::new(Orientation::Vertical, 1);
        info.set_hexpand(true);
        info.set_valign(gtk4::Align::Center);
        let title = Label::new(Some(&e.name));
        title.set_css_classes(&["downloads-row-title"]);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        let sub = Label::new(Some(&e.dest));
        sub.set_css_classes(&["downloads-row-sub"]);
        sub.set_xalign(0.0);
        sub.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        info.append(&title);
        info.append(&sub);
        row.append(&info);

        let open_btn = Button::from_icon_name("document-open-symbolic");
        open_btn.set_tooltip_text(Some("Open file"));
        open_btn.set_css_classes(&["downloads-cancel-btn"]);
        open_btn.set_valign(gtk4::Align::Center);
        let dest = e.dest.clone();
        open_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("xdg-open").arg(&dest).spawn();
        });
        row.append(&open_btn);

        let folder_btn = Button::from_icon_name("folder-open-symbolic");
        folder_btn.set_tooltip_text(Some("Show in folder"));
        folder_btn.set_css_classes(&["downloads-cancel-btn"]);
        folder_btn.set_valign(gtk4::Align::Center);
        let dir = std::path::PathBuf::from(&e.dest);
        folder_btn.connect_clicked(move |_| {
            if let Some(parent) = dir.parent() {
                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
            }
        });
        row.append(&folder_btn);

        let rm_btn = Button::from_icon_name("window-close-symbolic");
        rm_btn.set_tooltip_text(Some("Remove from list"));
        rm_btn.set_css_classes(&["downloads-cancel-btn"]);
        rm_btn.set_valign(gtk4::Align::Center);
        let dest = e.dest.clone();
        let when = e.when;
        rm_btn.connect_clicked(move |_| {
            config::remove_download_entry(&dest, when);
            if let Some(me) = me_weak.upgrade() {
                me.rebuild_downloads_page();
            }
        });
        row.append(&rm_btn);

        row
    }

    /// "Today" / "Yesterday" / "September 27, 2026" from unix seconds.
    fn day_label(when: u64) -> String {
        let secs = i64::try_from(when).unwrap_or(0);
        let days = secs.div_euclid(86400);
        let now_days = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64 / 86400)
            .unwrap_or(days);
        let (y, m, d) = Self::civil_from_days(days);
        let month = [
            "January", "February", "March", "April", "May", "June", "July",
            "August", "September", "October", "November", "December",
        ][(m - 1).clamp(0, 11) as usize];
        if days == now_days {
            "Today".to_string()
        } else if days == now_days - 1 {
            "Yesterday".to_string()
        } else {
            format!("{month} {d}, {y}")
        }
    }

    /// Days-since-epoch → (year, month, day). Howard Hinnant's civil calendar.
    fn civil_from_days(z: i64) -> (i64, i64, i64) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    /// The folder downloads go to: user setting, else XDG/~/Downloads.
    fn effective_download_dir(&self) -> std::path::PathBuf {
        let configured = self.settings.borrow().download_dir.clone();
        if !configured.is_empty() {
            std::path::PathBuf::from(configured)
        } else {
            Self::downloads_dir()
        }
    }

    /// One row inside the downloads popover: (row, optional cancel button).
    fn download_row(
        name: &str,
        detail: &str,
        progress: Option<&gtk4::LevelBar>,
        cancel_id: u64,
        me_weak: Weak<BrowserWindow>,
    ) -> (GtkBox, Option<Button>) {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        row.set_margin_top(4);
        row.set_margin_bottom(4);
        row.set_margin_start(10);
        row.set_margin_end(10);

        let icon = gtk4::Image::from_icon_name(if progress.is_some() {
            "document-save-symbolic"
        } else {
            "document-open-recent-symbolic"
        });
        icon.set_valign(gtk4::Align::Center);
        row.append(&icon);

        let info = GtkBox::new(Orientation::Vertical, 1);
        info.set_hexpand(true);
        info.set_valign(gtk4::Align::Center);
        let title = Label::new(Some(name));
        title.set_css_classes(&["downloads-row-title"]);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::Start);
        info.append(&title);
        let sub = Label::new(Some(detail));
        sub.set_css_classes(&["downloads-row-sub"]);
        sub.set_xalign(0.0);
        sub.set_ellipsize(gtk4::pango::EllipsizeMode::Start);
        info.append(&sub);
        if let Some(bar) = progress {
            info.append(bar);
        }
        row.append(&info);

        let mut cancel = None;
        if cancel_id != 0 {
            let btn = Button::from_icon_name("process-stop-symbolic");
            btn.set_tooltip_text(Some("Cancel download"));
            btn.set_css_classes(&["downloads-cancel-btn"]);
            btn.set_valign(gtk4::Align::Center);
            btn.connect_clicked(move |_| {
                if let Some(me) = me_weak.upgrade() {
                    me.cancel_download(cancel_id);
                }
            });
            cancel = Some(btn.clone());
            row.append(&btn);
        }
        (row, cancel)
    }

    /// Tag a popover with the active theme.
    ///
    /// A popover is a separate toplevel surface, so it never inherits the
    /// `dark` class we set on the main window — left untagged it renders with
    /// the light GTK defaults, which is why menus came up as pale boxes with
    /// washed-out text on a dark page.
    fn theme_popover(&self, pop: &gtk4::Popover) {
        if self.is_dark_active() {
            pop.add_css_class("dark");
        } else {
            pop.add_css_class("light");
        }
    }

    /// Popover listing live downloads + today's history, with an
    /// "Open download page" link. Refreshed live while downloads run.
    fn show_downloads_popover(&self) {
        // Toggle: an open popover closes instead of rebuilding.
        if let Some(pop) = self.downloads_popover.borrow().as_ref() {
            pop.popdown();
            return;
        }
        let pop = gtk4::Popover::new();
        self.theme_popover(&pop);
        pop.set_has_arrow(false);
        pop.set_parent(&self.downloads_btn);
        *self.downloads_popover.borrow_mut() = Some(pop.clone());
        {
            let me2 = self.self_weak.clone();
            let pop2 = pop.clone();
            pop.connect_closed(move |_| {
                if pop2.parent().is_some() {
                    pop2.unparent();
                }
                if let Some(me) = me2.upgrade() {
                    *me.downloads_popover.borrow_mut() = None;
                    *me.downloads_popover_rows.borrow_mut() = None;
                }
            });
        }
        let box_ = GtkBox::new(Orientation::Vertical, 0);
        box_.set_css_classes(&["downloads-popover"]);
        box_.set_width_request(340);

        let header = GtkBox::new(Orientation::Horizontal, 8);
        let title = Label::new(Some("Downloads"));
        title.set_css_classes(&["downloads-header"]);
        title.set_xalign(0.0);
        title.set_hexpand(true);
        header.append(&title);
        let page_btn = Button::from_icon_name("view-fullscreen-symbolic");
        page_btn.set_tooltip_text(Some("Open full download page"));
        page_btn.set_css_classes(&["downloads-cancel-btn"]);
        let me_page = self.self_weak.clone();
        page_btn.connect_clicked(move |_| {
            if let Some(me) = me_page.upgrade() {
                me.close_downloads_popover();
                me.toggle_downloads_page();
            }
        });
        header.append(&page_btn);
        box_.append(&header);

        let rows_box = GtkBox::new(Orientation::Vertical, 0);
        box_.append(&rows_box);
        // Stash so progress ticks can rebuild the rows live.
        *self.downloads_popover_rows.borrow_mut() = Some(rows_box.clone());
        self.fill_downloads_rows(&rows_box);

        let open_dir = Button::with_label("Open Downloads Folder");
        open_dir.set_css_classes(&["downloads-open-dir"]);
        open_dir.set_halign(gtk4::Align::Fill);
        let me_weak = self.self_weak.clone();
        let pop2 = pop.clone();
        open_dir.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                let dir = me.effective_download_dir();
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
            }
            pop2.popdown();
        });
        box_.append(&open_dir);

        pop.set_child(Some(&box_));
        pop.present();
    }

    fn close_downloads_popover(&self) {
        if let Some(pop) = self.downloads_popover.borrow().as_ref() {
            pop.popdown();
        }
        *self.downloads_popover.borrow_mut() = None;
        *self.downloads_popover_rows.borrow_mut() = None;
    }

    /// Fill/re-fill a downloads rows container (popover body or page list).
    fn fill_downloads_rows(&self, rows_box: &GtkBox) {
        while let Some(child) = rows_box.first_child() {
            rows_box.remove(&child);
        }
        let mut rows = 0;
        let live: Vec<(String, f64, u64)> = self.downloads.borrow().iter().map(|d| {
            let pct = unsafe { ffi::webkit_download_get_estimated_progress(d.download) } * 100.0;
            (d.name.clone(), pct, d.id)
        }).collect();
        for (name, pct, id) in live {
            let bar = gtk4::LevelBar::builder()
                .min_value(0.0)
                .max_value(100.0)
                .value(pct)
                .valign(gtk4::Align::Center)
                .build();
            let (row, _) = Self::download_row(
                &name,
                &format!("{pct:.0}% — downloading"),
                Some(&bar),
                id,
                self.self_weak.clone(),
            );
            rows_box.append(&row);
            rows += 1;
        }
        let history: Vec<config::DownloadEntry> =
            config::load_download_entries().into_iter().rev().take(5).collect();
        for h in history {
            let (row, _) = Self::download_row(&h.name, &h.dest, None, 0, Weak::new());
            rows_box.append(&row);
            rows += 1;
        }
        if rows == 0 {
            let empty = Label::new(Some("No downloads yet"));
            empty.set_css_classes(&["downloads-empty"]);
            empty.set_margin_top(8);
            empty.set_margin_bottom(8);
            rows_box.append(&empty);
        }
    }

    /// Rebuild popover rows + page list + persistent pill after any
    /// download state change.
    fn refresh_download_ui(&self) {
        if let Some(rows) = self.downloads_popover_rows.borrow().as_ref().map(|r| r.clone()) {
            self.fill_downloads_rows(&rows);
        }
        if self.active_downloads_page.borrow().is_some() {
            self.rebuild_downloads_page();
        }
        self.update_download_pill();
    }

    /// Persistent bottom-right card while any download is active: filename,
    /// transferred/total size, a progress bar and a cancel button. Updated in
    /// place on every poll tick so it never flickers.
    fn update_download_pill(&self) {
        let live = self.downloads.borrow();
        if live.is_empty() {
            drop(live);
            if let Some(p) = self.download_pill.borrow_mut().take() {
                self.overlay.remove_overlay(&p.root);
            }
            return;
        }
        let d = &live[0];
        let id = d.id;
        let name = d.name.clone();
        let extra = live.len() - 1;
        let fraction = unsafe { ffi::webkit_download_get_estimated_progress(d.download) };
        let received = unsafe { ffi::webkit_download_get_received_data_length(d.download) };
        let total = unsafe {
            let resp = ffi::webkit_download_get_response(d.download);
            if resp.is_null() {
                0
            } else {
                ffi::webkit_uri_response_get_content_length(resp)
            }
        };
        drop(live);

        // Keep the widget and only update the text/bar: rebuilding it each
        // tick made the card jump and leak overlays.
        let existing = self
            .download_pill
            .borrow_mut()
            .as_ref()
            .map(|p| (p.root.clone(), p.title.clone(), p.meta.clone(), p.bar.clone()));
        if let Some((_root, title, meta, bar)) = existing {
            title.set_text(&name);
            meta.set_text(&Self::download_progress_text(fraction, received, total, extra));
            bar.set_fraction(fraction.clamp(0.0, 1.0));
            return;
        }

        let root = GtkBox::new(Orientation::Vertical, 8);
        root.set_css_classes(&["download-pill"]);
        root.set_halign(gtk4::Align::End);
        root.set_valign(gtk4::Align::End);
        root.set_margin_end(18);
        root.set_margin_bottom(18);

        // Row 1: icon + name + cancel.
        let top = GtkBox::new(Orientation::Horizontal, 10);
        top.set_css_classes(&["download-pill-top"]);
        let icon = gtk4::Image::from_icon_name("folder-download-symbolic");
        icon.set_css_classes(&["download-pill-icon"]);
        icon.set_valign(gtk4::Align::Start);
        icon.set_pixel_size(18);

        let name_box = GtkBox::new(Orientation::Vertical, 2);
        name_box.set_hexpand(true);
        let title = Label::new(Some(&name));
        title.set_css_classes(&["download-pill-title"]);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        let meta = Label::new(Some(&Self::download_progress_text(
            fraction, received, total, extra,
        )));
        meta.set_css_classes(&["download-pill-meta"]);
        meta.set_xalign(0.0);
        meta.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        name_box.append(&title);
        name_box.append(&meta);

        let cancel = Button::from_icon_name("window-close-symbolic");
        cancel.set_tooltip_text(Some("Cancel download"));
        cancel.set_css_classes(&["download-pill-cancel"]);
        cancel.set_valign(gtk4::Align::Center);
        cancel.set_focus_on_click(false);
        let me_weak = self.self_weak.clone();
        cancel.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.cancel_download(id);
            }
        });

        top.append(&icon);
        top.append(&name_box);
        top.append(&cancel);

        // Row 2: determinate progress bar.
        let bar = ProgressBar::new();
        bar.set_css_classes(&["download-pill-bar"]);
        bar.set_valign(gtk4::Align::Center);
        bar.set_hexpand(true);
        bar.set_fraction(fraction.clamp(0.0, 1.0));

        root.append(&top);
        root.append(&bar);
        self.overlay.add_overlay(&root);
        *self.download_pill.borrow_mut() = Some(DownloadPill {
            root,
            title,
            meta,
            bar,
        });
    }

    /// "1.2 MB of 8.4 MB · 42%" (+ N more) with graceful degradation when
    /// WebKit can't tell us the total size or the byte count yet.
    fn download_progress_text(fraction: f64, received: u64, total: u64, extra: usize) -> String {
        let pct = (fraction * 100.0).clamp(0.0, 100.0);
        let mut text = match (received, total) {
            (r, t) if t > 0 => format!(
                "{} of {} · {pct:.0}%",
                Self::human_bytes(r),
                Self::human_bytes(t)
            ),
            (r, 0) if r > 0 => format!("{} · {pct:.0}%", Self::human_bytes(r)),
            _ => format!("{pct:.0}%"),
        };
        if extra > 0 {
            text.push_str(&format!(" · +{extra} more"));
        }
        text
    }

    fn human_bytes(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut v = bytes as f64;
        let mut unit = 0;
        while v >= 1024.0 && unit < UNITS.len() - 1 {
            v /= 1024.0;
            unit += 1;
        }
        if unit == 0 {
            format!("{bytes} B")
        } else {
            format!("{v:.1} {}", UNITS[unit])
        }
    }

    /// Cancel a live download and drop it from tracking.
    fn cancel_download(&self, id: u64) {
        // Close the "where to save" dialog first: `dlg.close()` emits
        // `response`, whose handler calls back into cancel_download, and the
        // download must still be findable at that point.
        self.close_save_dialog(id);
        let dl = self
            .downloads
            .borrow_mut()
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.download);
        if let Some(dl) = dl {
            unsafe { ffi::webkit_download_cancel(dl) };
        }
        // Take before retaining: dropping ActiveDownload releases our strong
        // ref, cancelling just stops the transfer.
        let pos = self.downloads.borrow_mut().iter().position(|d| d.id == id);
        let owned = pos.map(|pos| self.downloads.borrow_mut().remove(pos));
        if let Some(d) = owned {
            unsafe { ffi::g_object_unref(d.download as *mut ffi::GObject) };
        }
        self.refresh_download_ui();
        self.flash_download_pill("Download cancelled");
    }

    /// Brief toast in the bottom-right corner ("saved", "cancelled", ...),
    /// stacked above the live-download card so the two never overlap.
    fn flash_download_pill(&self, text: &str) {
        let toast = GtkBox::new(Orientation::Horizontal, 8);
        toast.set_css_classes(&["download-toast"]);
        let lbl = Label::new(Some(text));
        lbl.set_css_classes(&["download-toast-text"]);
        lbl.set_xalign(0.0);
        lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        toast.append(&lbl);
        toast.set_halign(gtk4::Align::End);
        toast.set_valign(gtk4::Align::End);
        toast.set_margin_end(18);
        // Sits above the download card when one is showing.
        toast.set_margin_bottom(18);
        self.overlay.add_overlay(&toast);
        gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(2800), move || {
            toast.unparent();
        });
    }

    /// Called when the network session reports a new download. Registers it
    /// and hooks `decide-destination` — the GTK4 signal that asks the app
    /// where to save. Without a handler WebKit silently uses its default
    /// (~Downloads) and our UI never learns about the download.
    fn on_download_started(&self, download: *mut ffi::WebKitDownload) {
        if download.is_null() {
            return;
        }
        unsafe {
            let response = ffi::webkit_download_get_response(download);
            let uri = if response.is_null() {
                String::new()
            } else {
                let p = ffi::webkit_uri_response_get_uri(response);
                if p.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
                }
            };
            let name = if response.is_null() {
                Self::download_filename_from_uri(&uri)
            } else {
                let suggested = ffi::webkit_uri_response_get_suggested_filename(response);
                if suggested.is_null() {
                    Self::download_filename_from_uri(&uri)
                } else {
                    let s =
                        std::ffi::CStr::from_ptr(suggested).to_string_lossy().into_owned();
                    if s.is_empty() {
                        Self::download_filename_from_uri(&uri)
                    } else {
                        s
                    }
                }
            };

            let id = self.next_download_id.get();
            self.next_download_id.set(id + 1);
            // `download-started` loans the object — take a strong ref so the
            // pointer survives while we poll it. Released at every removal site.
            ffi::g_object_ref(download as *mut ffi::GObject);
            self.downloads.borrow_mut().push(ActiveDownload {
                id,
                name: name.clone(),
                dest: String::new(), // decided via decide-destination
                download,
            });
            self.connect_download_lifecycle(download, id);
            self.connect_decide_destination(download, id, name);
            self.poll_download(id);
        }
    }

    /// Hook `decide-destination`: WebKit pauses the download and waits for
    /// set_destination or cancel once the handler returns TRUE. Ask-mode
    /// opens the async picker here; default mode assigns immediately.
    fn connect_decide_destination(
        &self,
        download: *mut ffi::WebKitDownload,
        id: u64,
        _suggested: String,
    ) {
        // NOTE: `data` must be a thin pointer. `Box<dyn Fn(..)>` is a *fat*
        // pointer; casting it straight to `*mut c_void` silently drops the
        // vtable, and the trampoline then re-reads garbage as the vtable and
        // jumps to a wild address. So the closure is boxed twice: the data
        // pointer addresses a `DecideCb` slot (a sized type, so thin), and
        // dereferencing it yields the real fat Box intact.
        type DecideCb = Box<dyn Fn(*const std::os::raw::c_char) + 'static>;
        unsafe extern "C" fn trampoline(
            _dl: *mut c_void,
            suggested: *const std::os::raw::c_char,
            data: *mut c_void,
        ) -> i32 {
            let slot = &*(data as *const DecideCb);
            slot(suggested);
            1 // TRUE — handled; WebKit waits for set_destination / cancel
        }
        let me_weak = self.self_weak.clone();
        let cb: DecideCb = Box::new(move |suggested_ptr| {
            let Some(me) = me_weak.upgrade() else { return };
            let name = if suggested_ptr.is_null() {
                String::new()
            } else {
                // SAFETY: WebKit guarantees a valid NUL-terminated string
                // for the lifetime of the decide-destination emission.
                unsafe { std::ffi::CStr::from_ptr(suggested_ptr) }
                    .to_string_lossy()
                    .into_owned()
            };
            let name = if name.is_empty() {
                me.downloads
                    .borrow()
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "download".into())
            } else {
                name
            };
            if me.settings.borrow().ask_download_location {
                // Ask every time — async picker; WebKit stays paused until
                // set_destination or cancel. Only the id crosses into the
                // dialog closure: the raw WebKitDownload pointer can dangle
                // if the download dies while the dialog is open.
                me.ask_download_destination(id, name);
            } else {
                let dir = me.effective_download_dir();
                let _ = std::fs::create_dir_all(&dir);
                me.assign_download_destination(id, Self::unique_dest(dir.join(&name)));
            }
        });
        unsafe {
            let slot = Box::new(cb);
            ffi::g_signal_connect_data(
                download as *mut ffi::GObject,
                c"decide-destination".as_ptr(),
                Some(std::mem::transmute(
                    trampoline as unsafe extern "C" fn(
                        *mut c_void,
                        *const std::os::raw::c_char,
                        *mut c_void,
                    ) -> i32,
                )),
                Box::into_raw(slot) as *mut c_void,
                Some(crate::webview::destroy_notify::<DecideCb>),
                0,
            );
        }
    }

    /// Uniquify a destination path with " (1)", " (2)"… suffixes.
    fn unique_dest(dest: std::path::PathBuf) -> std::path::PathBuf {
        if !dest.exists() {
            return dest;
        }
        let dir = dest.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let stem = dest
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        let ext = dest
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        let mut n = 1;
        loop {
            let candidate = dir.join(format!("{stem} ({n}){ext}"));
            if !candidate.exists() {
                return candidate;
            }
            n += 1;
        }
    }

    /// The live `WebKitDownload` for `id`, or None if it already finished.
    /// Every async path (the save dialog, the poll tick) resolves the pointer
    /// through this rather than capturing it: the `g_object_ref` we take at
    /// `download-started` is dropped on the fail/cancel paths, so a pointer
    /// captured earlier can dangle and crash WebKit.
    fn download_ptr(&self, id: u64) -> Option<*mut ffi::WebKitDownload> {
        self.downloads
            .borrow()
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.download)
    }

    /// Finalize the destination for a download and register it.
    fn assign_download_destination(&self, id: u64, dest: std::path::PathBuf) {
        let Some(download) = self.download_ptr(id) else {
            // Download went away while we were deciding (user cancelled, or
            // the transfer failed) — nothing to point at a file any more.
            return;
        };
        let c_dest = std::ffi::CString::new(dest.to_string_lossy().as_ref())
            .unwrap_or_default();
        unsafe {
            ffi::webkit_download_set_destination(download, c_dest.as_ptr());
        }
        let file_name = dest
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        if let Some(d) = self.downloads.borrow_mut().iter_mut().find(|d| d.id == id) {
            d.dest = dest.to_string_lossy().into_owned();
            d.name = file_name.clone();
        }
        self.flash_download_pill(&format!("⬇ {file_name}"));
        self.refresh_download_ui();
    }

    /// "Save file" dialog. Accept → assign destination; dismiss → cancel the
    /// download entirely.
    ///
    /// Deliberately an in-process `FileChooserDialog`, not `FileDialog` or
    /// `FileChooserNative`: both of those are only xdg-desktop-portal wrappers,
    /// and on a session with no portal running they never show a window —
    /// `FileChooserNative` reports Accept immediately (silently saving into the
    /// cwd) and `FileDialog` returns "Dismissed by user" straight away. This
    /// is a plain GTK widget, so it always opens and always waits for a real
    /// answer.
    #[allow(deprecated)]
    fn ask_download_destination(&self, id: u64, suggested: String) {
        use gtk4::prelude::FileChooserExt;

        let dialog = gtk4::FileChooserDialog::builder()
            .title("Save Download")
            .action(gtk4::FileChooserAction::Save)
            .transient_for(&self.window)
            .modal(true)
            .build();
        dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
        dialog.add_button("Save", gtk4::ResponseType::Accept);
        dialog.set_default_response(gtk4::ResponseType::Accept);
        // Open on the configured download folder so the common case is one
        // keypress (Enter) rather than a full folder hunt.
        let start_dir = self.effective_download_dir();
        if start_dir.is_dir() {
            let _ = dialog.set_current_folder(Some(&gtk4::gio::File::for_path(start_dir)));
        }
        dialog.set_current_name(&suggested);
        dialog.set_css_classes(&self.dialog_css_classes());

        let me_weak = self.self_weak.clone();
        // `handled` makes the response handler idempotent: close_save_dialog()
        // can re-enter through response, and a second pass would cancel the
        // download we just handed a destination to.
        let handled = Cell::new(false);
        dialog.connect_response(move |d, resp| {
            if handled.replace(true) {
                return;
            }
            let Some(me) = me_weak.upgrade() else {
                d.close();
                return;
            };
            me.pending_save_dialogs
                .borrow_mut()
                .retain(|(did, _)| *did != id);
            // Must close explicitly: dropping our last reference does not
            // destroy a *mapped* toplevel, so the dialog would otherwise stay
            // on screen for the rest of the session and re-ask for every
            // subsequent download.
            d.close();
            if resp == gtk4::ResponseType::Accept {
                if let Some(path) = d.file().and_then(|f| f.path()) {
                    // Resolves the WebKitDownload by id — safe even if the
                    // transfer died while the dialog was open.
                    me.assign_download_destination(id, path);
                    return;
                }
            }
            // Dismissed → the user doesn't want this file.
            me.cancel_download(id);
        });
        // Keep the dialog alive while it's up (a dropped visible dialog is a
        // use-after-free) and so we can close it if the download dies.
        self.pending_save_dialogs
            .borrow_mut()
            .push((id, dialog.clone()));
        dialog.show();
    }

    /// Close and forget any save dialog still open for `id`. Called from the
    /// fail/cancel paths so a dialog can't outlive its download.
    fn close_save_dialog(&self, id: u64) {
        let mut pending = self.pending_save_dialogs.borrow_mut();
        pending.retain(|(did, dlg)| {
            if *did == id {
                dlg.close();
                false
            } else {
                true
            }
        });
    }

    /// Theme classes for standalone toplevels (dialogs). They are not
    /// descendants of the main window, so they need the class set by hand —
    /// same reason as `theme_popover`.
    fn dialog_css_classes(&self) -> Vec<&'static str> {
        if self.is_dark_active() {
            vec!["dark"]
        } else {
            vec!["light"]
        }
    }

    /// Connect `finished` / `failed` so completion is event-driven (works
    /// even when progress estimation stalls at <100%).
    fn connect_download_lifecycle(&self, download: *mut ffi::WebKitDownload, id: u64) {
        // Same double-box rule as `decide-destination`: the pointer handed to
        // C must be thin, so it addresses a slot holding the fat closure box.
        type LifecycleCb = Box<dyn Fn() + 'static>;
        unsafe extern "C" fn finished_trampoline(_dl: *mut c_void, data: *mut c_void) {
            let slot = &*(data as *const LifecycleCb);
            slot();
        }
        unsafe extern "C" fn failed_trampoline(
            _dl: *mut c_void,
            _err: *mut c_void,
            data: *mut c_void,
        ) {
            let slot = &*(data as *const LifecycleCb);
            slot();
        }
        unsafe {
            // finished → success
            let me = self.self_weak.clone();
            let cb: LifecycleCb = Box::new(move || {
                if let Some(me) = me.upgrade() {
                    me.finish_download(id);
                }
            });
            let slot = Box::new(cb);
            ffi::g_signal_connect_data(
                download as *mut ffi::GObject,
                c"finished".as_ptr(),
                Some(std::mem::transmute(
                    finished_trampoline
                        as unsafe extern "C" fn(*mut c_void, *mut c_void),
                )),
                Box::into_raw(slot) as *mut c_void,
                Some(crate::webview::destroy_notify::<LifecycleCb>),
                0,
            );
            // failed → error path (incl. user cancellation)
            let me = self.self_weak.clone();
            let cb: LifecycleCb = Box::new(move || {
                if let Some(me) = me.upgrade() {
                    me.fail_download(id);
                }
            });
            let slot = Box::new(cb);
            ffi::g_signal_connect_data(
                download as *mut ffi::GObject,
                c"failed".as_ptr(),
                Some(std::mem::transmute(
                    failed_trampoline
                        as unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void),
                )),
                Box::into_raw(slot) as *mut c_void,
                Some(crate::webview::destroy_notify::<LifecycleCb>),
                0,
            );
        }
    }

    /// Download completed → persist to history, notify.
    fn finish_download(&self, id: u64) {
        let pos = self.downloads.borrow_mut().iter().position(|d| d.id == id);
        let removed = pos.map(|pos| {
            let d = self.downloads.borrow_mut().remove(pos);
            unsafe { ffi::g_object_unref(d.download as *mut ffi::GObject) };
            (d.name.clone(), d.dest.clone())
        });
        if let Some((name, dest)) = removed {
            if !dest.is_empty() {
                let when = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                config::record_download_entry(config::DownloadEntry {
                    name: name.clone(),
                    dest: dest.clone(),
                    uri: String::new(),
                    when,
                });
                self.completed_downloads.set(self.completed_downloads.get() + 1);
                self.flash_download_pill(&format!("✓ {name} saved"));
            }
        }
        self.refresh_download_ui();
    }

    /// Download failed (or was cancelled) → drop from tracking.
    fn fail_download(&self, id: u64) {
        // Don't leave a "Save Download" dialog on screen for a download that
        // no longer exists — answering it would hit a freed WebKitDownload.
        self.close_save_dialog(id);
        let pos = self.downloads.borrow_mut().iter().position(|d| d.id == id);
        let removed = pos.map(|pos| {
            let d = self.downloads.borrow_mut().remove(pos);
            unsafe { ffi::g_object_unref(d.download as *mut ffi::GObject) };
            d.name.clone()
        });
        if let Some(name) = removed {
            self.flash_download_pill(&format!("✗ {name} failed"));
        }
        self.refresh_download_ui();
    }

    fn download_filename_from_uri(uri: &str) -> String {
        let tail = uri.rsplit('/').next().unwrap_or("download");
        let cleaned: String = tail
            .split(['?', '#'])
            .next()
            .unwrap_or("download")
            .to_string();
        if cleaned.is_empty() {
            "download".to_string()
        } else {
            cleaned
        }
    }

    /// Poll download progress to drive the live pill/popover/page. Completion
    /// itself is event-driven via finished/failed signals.
    fn poll_download(&self, id: u64) {
        let me_weak = self.self_weak.clone();
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
            let Some(me) = me_weak.upgrade() else {
                return gtk4::glib::ControlFlow::Break;
            };
            if !me.downloads.borrow().iter().any(|d| d.id == id) {
                return gtk4::glib::ControlFlow::Break;
            }
            me.refresh_download_ui();
            gtk4::glib::ControlFlow::Continue
        });
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
            self.remove_tab_row(&row);
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
        self.save_session();
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
        container.set_width_request(120);

        // Favicon slot: real site icon when available, colored letter tile otherwise.
        let favicon_letter = Label::new(Some("N"));
        favicon_letter.set_css_classes(&["tab-favicon", "fav-c0"]);
        favicon_letter.set_size_request(20, 20);
        favicon_letter.set_valign(gtk4::Align::Center);
        favicon_letter.set_halign(gtk4::Align::Center);

        let favicon_image = gtk4::Image::new();
        favicon_image.set_pixel_size(16);
        favicon_image.set_valign(gtk4::Align::Center);
        favicon_image.set_halign(gtk4::Align::Center);

        let favicon_stack = gtk4::Stack::new();
        favicon_stack.set_css_classes(&["tab-favicon-slot"]);
        favicon_stack.set_transition_type(gtk4::StackTransitionType::None);
        favicon_stack.add_named(&favicon_letter, Some("letter"));
        favicon_stack.add_named(&favicon_image, Some("icon"));
        favicon_stack.set_visible_child_name("letter");
        favicon_stack.set_valign(gtk4::Align::Center);
        container.append(&favicon_stack);

        let title_label = Label::new(Some("New Tab"));
        title_label.set_xalign(0.0);
        title_label.set_hexpand(true);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(16);
        title_label.set_single_line_mode(true);
        container.append(&title_label);

        let close_btn = Button::from_icon_name("window-close-symbolic");
        close_btn.set_size_request(16, 16);
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

        let row = Rc::new(Self {
            container,
            title_label,
            close_btn,
            favicon_letter,
            favicon_image,
            favicon_stack,
            has_icon: Cell::new(false),
            tab: Rc::downgrade(&tab),
        });
        row.update_favicon(&tab);
        row
    }

    /// Refresh the letter tile from the tab's title/URL (fallback when no icon).
    fn update_favicon(&self, tab: &Tab) {
        let title = tab.title.borrow().clone();
        let letter = title
            .chars()
            .find(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "N".to_string());
        self.favicon_letter.set_text(&letter);

        let mut hash: u32 = 5381;
        for b in tab.url.borrow().bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u32);
        }
        let idx = (hash as usize) % 6;
        self.favicon_letter.remove_css_class("fav-c0");
        self.favicon_letter.remove_css_class("fav-c1");
        self.favicon_letter.remove_css_class("fav-c2");
        self.favicon_letter.remove_css_class("fav-c3");
        self.favicon_letter.remove_css_class("fav-c4");
        self.favicon_letter.remove_css_class("fav-c5");
        self.favicon_letter.add_css_class(&format!("fav-c{idx}"));

        if !self.has_icon.get() {
            self.favicon_stack.set_visible_child_name("letter");
        }
    }

    /// Called on navigation: the old icon is stale, drop back to the letter tile.
    fn reset_favicon(&self) {
        self.has_icon.set(false);
        self.favicon_stack.set_visible_child_name("letter");
    }

    /// Show the real site favicon from raw RGBA pixels.
    fn set_favicon_rgba(&self, data: &crate::tab::FaviconData) {
        if data.width <= 0 || data.height <= 0 || data.rgba.is_empty() {
            return;
        }
        let bytes = gtk4::glib::Bytes::from(&data.rgba);
        let texture = gtk4::gdk::MemoryTexture::new(
            data.width,
            data.height,
            gtk4::gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &bytes,
            (data.width as usize) * 4,
        );
        self.favicon_image.set_paintable(Some(&texture));
        self.favicon_image.set_pixel_size(16);
        self.has_icon.set(true);
        self.favicon_stack.set_visible_child_name("icon");
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

    fn wire_settings(&self, btn: &Button) {
        let me_weak = self.self_weak.clone();
        btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.open_settings_dialog();
            }
        });
    }

    pub fn close_settings_dialog(&self) {
        if let Some(backdrop) = self.active_settings_modal.borrow_mut().take() {
            self.overlay.remove_overlay(&backdrop);
        }
    }

    pub fn open_settings_dialog(&self) {
        if self.active_settings_modal.borrow().is_some() {
            self.close_settings_dialog();
            return;
        }

        let backdrop = GtkBox::new(Orientation::Vertical, 0);
        backdrop.set_hexpand(true);
        backdrop.set_vexpand(true);
        backdrop.set_halign(gtk4::Align::Fill);
        backdrop.set_valign(gtk4::Align::Fill);
        backdrop.set_css_classes(&["settings-modal-backdrop"]);

        let root_box = GtkBox::new(Orientation::Horizontal, 0);
        root_box.set_css_classes(&["settings-modal-dialog"]);
        root_box.set_halign(gtk4::Align::Center);
        root_box.set_valign(gtk4::Align::Center);
        root_box.set_width_request(820);
        root_box.set_height_request(560);

        // Clicking the backdrop outside the dialog card closes the modal
        let gesture_backdrop = GestureClick::new();
        let me_weak = self.self_weak.clone();
        let root_box_clone = root_box.clone();
        let backdrop_clone = backdrop.clone();
        gesture_backdrop.connect_pressed(move |_, _, x, y| {
            if let Some(rect) = root_box_clone.compute_bounds(&backdrop_clone) {
                let pt = gtk4::graphene::Point::new(x as f32, y as f32);
                if !rect.contains_point(&pt) {
                    if let Some(me) = me_weak.upgrade() {
                        me.close_settings_dialog();
                    }
                }
            }
        });
        backdrop.add_controller(gesture_backdrop);

        // ── Settings Sidebar ─────────────────────────────────────────────────
        let settings_sidebar = GtkBox::new(Orientation::Vertical, 2);
        settings_sidebar.set_css_classes(&["settings-modal-sidebar"]);
        settings_sidebar.set_width_request(216);

        let title_label = Label::new(Some("Settings"));
        title_label.set_css_classes(&["settings-modal-title"]);
        title_label.set_xalign(0.0);
        settings_sidebar.append(&title_label);

        // Sidebar nav items helper
        let make_nav_item = |icon_name: &str, label: &str, active: bool| -> Button {
            let btn = Button::new();
            let b_box = GtkBox::new(Orientation::Horizontal, 10);
            let icon = gtk4::Image::from_icon_name(icon_name);
            icon.set_icon_size(gtk4::IconSize::Inherit);
            icon.set_css_classes(&["settings-nav-icon"]);
            let lbl = Label::new(Some(label));
            lbl.set_xalign(0.0);
            b_box.append(&icon);
            b_box.append(&lbl);
            btn.set_child(Some(&b_box));
            if active {
                btn.set_css_classes(&["settings-modal-nav-btn", "active"]);
            } else {
                btn.set_css_classes(&["settings-modal-nav-btn"]);
            }
            btn.set_focus_on_click(false);
            btn
        };

        let btn_appearance =
            make_nav_item("preferences-desktop-theme-symbolic", "Appearance", true);
        let btn_general = make_nav_item("preferences-system-symbolic", "General", false);
        let btn_privacy = make_nav_item("security-high-symbolic", "Privacy & Security", false);
        let btn_shortcuts = make_nav_item("input-keyboard-symbolic", "Shortcuts", false);

        settings_sidebar.append(&btn_appearance);
        settings_sidebar.append(&btn_general);
        settings_sidebar.append(&btn_privacy);
        settings_sidebar.append(&btn_shortcuts);

        // ── Right Main Area ──────────────────────────────────────────────────
        let main_right_box = GtkBox::new(Orientation::Vertical, 0);
        main_right_box.set_hexpand(true);
        main_right_box.set_vexpand(true);
        main_right_box.set_css_classes(&["settings-modal-content-area"]);

        // Header with Page title & circular Close button
        let top_header = GtkBox::new(Orientation::Horizontal, 12);
        top_header.set_css_classes(&["settings-modal-top-header"]);

        let page_title_label = Label::new(Some("Appearance"));
        page_title_label.set_css_classes(&["settings-modal-page-title"]);
        page_title_label.set_hexpand(true);
        page_title_label.set_xalign(0.0);

        let close_dialog_btn = Button::from_icon_name("window-close-symbolic");
        close_dialog_btn.set_size_request(30, 30);
        close_dialog_btn.set_valign(gtk4::Align::Center);
        close_dialog_btn.set_focus_on_click(false);
        close_dialog_btn.set_tooltip_text(Some("Close"));
        close_dialog_btn.set_css_classes(&["settings-modal-close-btn"]);
        let me_weak = self.self_weak.clone();
        close_dialog_btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                me.close_settings_dialog();
            }
        });

        top_header.append(&page_title_label);
        top_header.append(&close_dialog_btn);
        main_right_box.append(&top_header);

        // Content Stack
        let content_stack = Stack::new();
        content_stack.set_vexpand(true);
        content_stack.set_hexpand(true);
        content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

        // ── Appearance Page (Default) ────────────────────────────────────────
        let app_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let app_page = GtkBox::new(Orientation::Vertical, 12);
        app_page.set_css_classes(&["settings-page-body"]);

        let sec_layout_label = Label::new(Some("Browser Layout"));
        sec_layout_label.set_css_classes(&["settings-group-header"]);
        sec_layout_label.set_xalign(0.0);
        app_page.append(&sec_layout_label);

        let layout_group = GtkBox::new(Orientation::Vertical, 0);
        layout_group.set_css_classes(&["settings-group-card"]);

        let row_layout = GtkBox::new(Orientation::Horizontal, 12);
        row_layout.set_css_classes(&["settings-group-row"]);
        let layout_info = GtkBox::new(Orientation::Vertical, 2);
        layout_info.set_hexpand(true);
        let layout_title = Label::new(Some("Chrome style"));
        layout_title.set_css_classes(&["settings-row-title"]);
        layout_title.set_xalign(0.0);
        let layout_subtitle = Label::new(Some("Show only the topbar or the vertical sidebar"));
        layout_subtitle.set_css_classes(&["settings-row-subtitle"]);
        layout_subtitle.set_xalign(0.0);
        layout_info.append(&layout_title);
        layout_info.append(&layout_subtitle);

        let layout_switcher = GtkBox::new(Orientation::Horizontal, 3);
        layout_switcher.set_css_classes(&["settings-layout-switcher"]);
        layout_switcher.set_valign(gtk4::Align::Center);
        let layout_topbar = Button::with_label("Topbar");
        layout_topbar.set_css_classes(&["settings-layout-btn"]);
        layout_topbar.set_focus_on_click(false);
        let layout_sidebar = Button::with_label("Sidebar");
        layout_sidebar.set_css_classes(&["settings-layout-btn"]);
        layout_sidebar.set_focus_on_click(false);
        if self.chrome_layout.get() == ChromeLayout::Topbar {
            layout_topbar.add_css_class("active");
        } else {
            layout_sidebar.add_css_class("active");
        }

        let me_weak = self.self_weak.clone();
        let topbar_button = layout_topbar.clone();
        let sidebar_button = layout_sidebar.clone();
        layout_topbar.connect_clicked(move |_| {
            topbar_button.add_css_class("active");
            sidebar_button.remove_css_class("active");
            if let Some(me) = me_weak.upgrade() {
                me.set_chrome_layout(ChromeLayout::Topbar);
            }
        });
        let me_weak = self.self_weak.clone();
        let topbar_button = layout_topbar.clone();
        let sidebar_button = layout_sidebar.clone();
        layout_sidebar.connect_clicked(move |_| {
            topbar_button.remove_css_class("active");
            sidebar_button.add_css_class("active");
            if let Some(me) = me_weak.upgrade() {
                me.set_chrome_layout(ChromeLayout::Sidebar);
            }
        });

        layout_switcher.append(&layout_topbar);
        layout_switcher.append(&layout_sidebar);
        row_layout.append(&layout_info);
        row_layout.append(&layout_switcher);
        layout_group.append(&row_layout);
        app_page.append(&layout_group);

        // Section 1: Theme & Style
        let sec_theme_label = Label::new(Some("Theme & Style"));
        sec_theme_label.set_css_classes(&["settings-group-header"]);
        sec_theme_label.set_xalign(0.0);
        app_page.append(&sec_theme_label);

        let theme_group_card = GtkBox::new(Orientation::Vertical, 0);
        theme_group_card.set_css_classes(&["settings-group-card"]);

        let row_theme = GtkBox::new(Orientation::Horizontal, 12);
        row_theme.set_css_classes(&["settings-group-row"]);
        let th_info_box = GtkBox::new(Orientation::Vertical, 2);
        th_info_box.set_hexpand(true);
        let th_title = Label::new(Some("Color Theme"));
        th_title.set_css_classes(&["settings-row-title"]);
        th_title.set_xalign(0.0);
        let th_sub = Label::new(Some("Select light, dark, or match system look"));
        th_sub.set_css_classes(&["settings-row-subtitle"]);
        th_sub.set_xalign(0.0);
        th_info_box.append(&th_title);
        th_info_box.append(&th_sub);

        // Segmented theme buttons: System / Light / Dark
        let theme_seg_box = GtkBox::new(Orientation::Horizontal, 2);
        theme_seg_box.set_css_classes(&["settings-segmented-group"]);
        theme_seg_box.set_valign(gtk4::Align::Center);

        let btn_theme_sys = Button::with_label("System");
        btn_theme_sys.set_css_classes(&["settings-segmented-btn"]);
        btn_theme_sys.set_focus_on_click(false);

        let btn_theme_light = Button::with_label("Light");
        btn_theme_light.set_css_classes(&["settings-segmented-btn"]);
        btn_theme_light.set_focus_on_click(false);

        let btn_theme_dark = Button::with_label("Dark");
        btn_theme_dark.set_css_classes(&["settings-segmented-btn"]);
        btn_theme_dark.set_focus_on_click(false);

        match self.theme_pref.get() {
            ThemePreference::System => btn_theme_sys.add_css_class("active"),
            ThemePreference::Light => btn_theme_light.add_css_class("active"),
            ThemePreference::Dark => btn_theme_dark.add_css_class("active"),
        }

        let me_weak = self.self_weak.clone();
        let b_sys = btn_theme_sys.clone();
        let b_lgt = btn_theme_light.clone();
        let b_drk = btn_theme_dark.clone();
        btn_theme_sys.connect_clicked(move |_| {
            b_sys.add_css_class("active");
            b_lgt.remove_css_class("active");
            b_drk.remove_css_class("active");
            if let Some(me) = me_weak.upgrade() {
                me.set_theme_preference(ThemePreference::System);
            }
        });

        let me_weak = self.self_weak.clone();
        let b_sys = btn_theme_sys.clone();
        let b_lgt = btn_theme_light.clone();
        let b_drk = btn_theme_dark.clone();
        btn_theme_light.connect_clicked(move |_| {
            b_sys.remove_css_class("active");
            b_lgt.add_css_class("active");
            b_drk.remove_css_class("active");
            if let Some(me) = me_weak.upgrade() {
                me.set_theme_preference(ThemePreference::Light);
            }
        });

        let me_weak = self.self_weak.clone();
        let b_sys = btn_theme_sys.clone();
        let b_lgt = btn_theme_light.clone();
        let b_drk = btn_theme_dark.clone();
        btn_theme_dark.connect_clicked(move |_| {
            b_sys.remove_css_class("active");
            b_lgt.remove_css_class("active");
            b_drk.add_css_class("active");
            if let Some(me) = me_weak.upgrade() {
                me.set_theme_preference(ThemePreference::Dark);
            }
        });

        theme_seg_box.append(&btn_theme_sys);
        theme_seg_box.append(&btn_theme_light);
        theme_seg_box.append(&btn_theme_dark);

        row_theme.append(&th_info_box);
        row_theme.append(&theme_seg_box);
        theme_group_card.append(&row_theme);
        app_page.append(&theme_group_card);

        // Section 2: Sidebar
        let sec_sidebar_label = Label::new(Some("Sidebar Options"));
        sec_sidebar_label.set_css_classes(&["settings-group-header"]);
        sec_sidebar_label.set_xalign(0.0);
        app_page.append(&sec_sidebar_label);

        let sidebar_group_box = GtkBox::new(Orientation::Vertical, 0);
        sidebar_group_box.set_css_classes(&["settings-group-card"]);

        // Enable Left Sidebar Row
        let row_sb_enable = GtkBox::new(Orientation::Horizontal, 12);
        row_sb_enable.set_css_classes(&["settings-group-row"]);
        let sb_en_info = GtkBox::new(Orientation::Vertical, 2);
        sb_en_info.set_hexpand(true);
        let sb_en_title = Label::new(Some("Show sidebar content"));
        sb_en_title.set_css_classes(&["settings-row-title"]);
        sb_en_title.set_xalign(0.0);
        let sb_en_sub = Label::new(Some("Use the vertical sidebar with pinned and normal tabs"));
        sb_en_sub.set_css_classes(&["settings-row-subtitle"]);
        sb_en_sub.set_xalign(0.0);
        sb_en_info.append(&sb_en_title);
        sb_en_info.append(&sb_en_sub);
        let sb_enable_switch = gtk4::Switch::new();
        sb_enable_switch.set_active(self.sidebar_enabled.get());
        sb_enable_switch.set_valign(gtk4::Align::Center);
        let me_weak = self.self_weak.clone();
        sb_enable_switch.connect_state_set(move |_, state| {
            if let Some(me) = me_weak.upgrade() {
                me.set_sidebar_enabled(state);
            }
            gtk4::glib::Propagation::Proceed
        });
        row_sb_enable.append(&sb_en_info);
        row_sb_enable.append(&sb_enable_switch);
        sidebar_group_box.append(&row_sb_enable);

        sidebar_group_box.append(&gtk4::Separator::new(Orientation::Horizontal));

        // Auto-hide Left Sidebar Row
        let row_sb_autohide = GtkBox::new(Orientation::Horizontal, 12);
        row_sb_autohide.set_css_classes(&["settings-group-row"]);
        let sb_ah_info = GtkBox::new(Orientation::Vertical, 2);
        sb_ah_info.set_hexpand(true);
        let sb_ah_title = Label::new(Some("Auto-hide left sidebar"));
        sb_ah_title.set_css_classes(&["settings-row-title"]);
        sb_ah_title.set_xalign(0.0);
        let sb_ah_sub = Label::new(Some("Switch between topbar and sidebar with Ctrl+B"));
        sb_ah_sub.set_css_classes(&["settings-row-subtitle"]);
        sb_ah_sub.set_xalign(0.0);
        sb_ah_info.append(&sb_ah_title);
        sb_ah_info.append(&sb_ah_sub);
        let sb_ah_switch = gtk4::Switch::new();
        sb_ah_switch.set_active(true);
        sb_ah_switch.set_valign(gtk4::Align::Center);
        row_sb_autohide.append(&sb_ah_info);
        row_sb_autohide.append(&sb_ah_switch);
        sidebar_group_box.append(&row_sb_autohide);

        app_page.append(&sidebar_group_box);

        // Section 3: Browser Chrome
        let sec_chrome_label = Label::new(Some("Browser Chrome"));
        sec_chrome_label.set_css_classes(&["settings-group-header"]);
        sec_chrome_label.set_xalign(0.0);
        app_page.append(&sec_chrome_label);

        let chrome_group_box = GtkBox::new(Orientation::Vertical, 0);
        chrome_group_box.set_css_classes(&["settings-group-card"]);

        // Auto-hide Topbar Row
        let row_topbar = GtkBox::new(Orientation::Horizontal, 12);
        row_topbar.set_css_classes(&["settings-group-row"]);
        let tb_info_box = GtkBox::new(Orientation::Vertical, 2);
        tb_info_box.set_hexpand(true);
        let tb_title = Label::new(Some("Auto-hide top navigation bar"));
        tb_title.set_css_classes(&["settings-row-title"]);
        tb_title.set_xalign(0.0);
        let tb_sub = Label::new(Some(
            "Hides topbar on web pages and reveals on top edge hover or Ctrl+L",
        ));
        tb_sub.set_css_classes(&["settings-row-subtitle"]);
        tb_sub.set_xalign(0.0);
        tb_info_box.append(&tb_title);
        tb_info_box.append(&tb_sub);
        let tb_switch = gtk4::Switch::new();
        tb_switch.set_active(true);
        tb_switch.set_valign(gtk4::Align::Center);
        row_topbar.append(&tb_info_box);
        row_topbar.append(&tb_switch);
        chrome_group_box.append(&row_topbar);

        chrome_group_box.append(&gtk4::Separator::new(Orientation::Horizontal));

        // Compact tab pills row
        let row_tabs = GtkBox::new(Orientation::Horizontal, 12);
        row_tabs.set_css_classes(&["settings-group-row"]);
        let tab_info_box = GtkBox::new(Orientation::Vertical, 2);
        tab_info_box.set_hexpand(true);
        let tab_title = Label::new(Some("Compact tab pills"));
        tab_title.set_css_classes(&["settings-row-title"]);
        tab_title.set_xalign(0.0);
        let tab_sub = Label::new(Some(
            "Display tabs as compact floating pills with pin support",
        ));
        tab_sub.set_css_classes(&["settings-row-subtitle"]);
        tab_sub.set_xalign(0.0);
        tab_info_box.append(&tab_title);
        tab_info_box.append(&tab_sub);
        let tab_switch = gtk4::Switch::new();
        tab_switch.set_active(true);
        tab_switch.set_valign(gtk4::Align::Center);
        row_tabs.append(&tab_info_box);
        row_tabs.append(&tab_switch);
        chrome_group_box.append(&row_tabs);

        app_page.append(&chrome_group_box);

        app_scroll.set_child(Some(&app_page));
        content_stack.add_named(&app_scroll, Some("appearance"));

        // ── General Page ─────────────────────────────────────────────────────
        let gen_page = GtkBox::new(Orientation::Vertical, 12);
        gen_page.set_css_classes(&["settings-page-body"]);

        let gen_sec_label = Label::new(Some("Search & Navigation"));
        gen_sec_label.set_css_classes(&["settings-group-header"]);
        gen_sec_label.set_xalign(0.0);
        gen_page.append(&gen_sec_label);

        let gen_group = GtkBox::new(Orientation::Vertical, 0);
        gen_group.set_css_classes(&["settings-group-card"]);

        let row_search = GtkBox::new(Orientation::Horizontal, 12);
        row_search.set_css_classes(&["settings-group-row"]);
        let s_info = GtkBox::new(Orientation::Vertical, 2);
        s_info.set_hexpand(true);
        let s_title = Label::new(Some("Default Search Engine"));
        s_title.set_css_classes(&["settings-row-title"]);
        s_title.set_xalign(0.0);
        let s_sub = Label::new(Some("Search provider for the URL and new tab search bar"));
        s_sub.set_css_classes(&["settings-row-subtitle"]);
        s_sub.set_xalign(0.0);
        s_info.append(&s_title);
        s_info.append(&s_sub);
        let s_btn = Button::with_label("DuckDuckGo ▾");
        s_btn.set_css_classes(&["settings-dropdown-btn"]);
        s_btn.set_valign(gtk4::Align::Center);
        row_search.append(&s_info);
        row_search.append(&s_btn);
        gen_group.append(&row_search);

        gen_page.append(&gen_group);

        // ── General → Downloads ──────────────────────────────────────────
        let dl_sec_label = Label::new(Some("Downloads"));
        dl_sec_label.set_css_classes(&["settings-group-header"]);
        dl_sec_label.set_xalign(0.0);
        gen_page.append(&dl_sec_label);

        let dl_group = GtkBox::new(Orientation::Vertical, 0);
        dl_group.set_css_classes(&["settings-group-card"]);

        // Folder row
        let row_dl_dir = GtkBox::new(Orientation::Horizontal, 12);
        row_dl_dir.set_css_classes(&["settings-group-row"]);
        let dl_info = GtkBox::new(Orientation::Vertical, 2);
        dl_info.set_hexpand(true);
        let dl_title = Label::new(Some("Download folder"));
        dl_title.set_css_classes(&["settings-row-title"]);
        dl_title.set_xalign(0.0);
        let dl_dir_display = gtk4::Label::new(None);
        let dl_dir_display_clone = dl_dir_display.clone();
        dl_dir_display.set_css_classes(&["settings-row-subtitle"]);
        dl_dir_display.set_ellipsize(gtk4::pango::EllipsizeMode::Start);
        dl_dir_display.set_xalign(0.0);
        dl_info.append(&dl_title);
        dl_info.append(&dl_dir_display_clone);

        let dl_dir_btn = Button::with_label("Change…");
        dl_dir_btn.set_css_classes(&["settings-dropdown-btn"]);
        dl_dir_btn.set_valign(gtk4::Align::Center);
        let me_weak = self.self_weak.clone();
        let win = self.window.clone();
        let dl_dir_label2 = dl_dir_display.clone();
        dl_dir_btn.connect_clicked(move |_| {
            if let Some(me) = me_weak.upgrade() {
                let dialog = gtk4::FileChooserNative::builder()
                    .title("Choose Download Folder")
                    .action(gtk4::FileChooserAction::SelectFolder)
                    .transient_for(&win)
                    .modal(true)
                    .build();
                let (tx, rx) = std::sync::mpsc::channel::<Option<std::path::PathBuf>>();
                let loop_ = gtk4::glib::MainLoop::new(None, false);
                let loop_quit = loop_.clone();
                dialog.connect_response(move |d, resp| {
                    let result = if resp == gtk4::ResponseType::Accept {
                        d.file().and_then(|f| f.path())
                    } else {
                        None
                    };
                    let _ = tx.send(result);
                    loop_quit.quit();
                });
                dialog.show();
                loop_.run();
                if let Ok(Some(dir)) = rx.try_recv() {
                    me.set_download_dir(dir.to_string_lossy().as_ref());
                    dl_dir_label2.set_text(&dir.to_string_lossy());
                }
            }
        });
        // Show current folder (setting or default).
        {
            let cur = self.effective_download_dir();
            dl_dir_display.set_text(&cur.to_string_lossy());
        }
        row_dl_dir.append(&dl_info);
        row_dl_dir.append(&dl_dir_btn);
        dl_group.append(&row_dl_dir);

        dl_group.append(&gtk4::Separator::new(Orientation::Horizontal));

        // Ask-every-time row
        let row_dl_ask = GtkBox::new(Orientation::Horizontal, 12);
        row_dl_ask.set_css_classes(&["settings-group-row"]);
        let dl_ask_info = GtkBox::new(Orientation::Vertical, 2);
        dl_ask_info.set_hexpand(true);
        let dl_ask_title = Label::new(Some("Ask where to save each download"));
        dl_ask_title.set_css_classes(&["settings-row-title"]);
        dl_ask_title.set_xalign(0.0);
        let dl_ask_sub = Label::new(Some("Show the file picker for every download"));
        dl_ask_sub.set_css_classes(&["settings-row-subtitle"]);
        dl_ask_sub.set_xalign(0.0);
        dl_ask_info.append(&dl_ask_title);
        dl_ask_info.append(&dl_ask_sub);
        let dl_ask_switch = gtk4::Switch::new();
        dl_ask_switch.set_active(self.settings.borrow().ask_download_location);
        dl_ask_switch.set_valign(gtk4::Align::Center);
        let me_weak = self.self_weak.clone();
        dl_ask_switch.connect_state_set(move |_, state| {
            if let Some(me) = me_weak.upgrade() {
                me.set_ask_download_location(state);
            }
            gtk4::glib::Propagation::Proceed
        });
        row_dl_ask.append(&dl_ask_info);
        row_dl_ask.append(&dl_ask_switch);
        dl_group.append(&row_dl_ask);

        gen_page.append(&dl_group);
        content_stack.add_named(&gen_page, Some("general"));

        // ── Privacy Page ─────────────────────────────────────────────────────
        let priv_page = GtkBox::new(Orientation::Vertical, 12);
        priv_page.set_css_classes(&["settings-page-body"]);

        let priv_sec_label = Label::new(Some("Security & Protection"));
        priv_sec_label.set_css_classes(&["settings-group-header"]);
        priv_sec_label.set_xalign(0.0);
        priv_page.append(&priv_sec_label);

        let priv_group = GtkBox::new(Orientation::Vertical, 0);
        priv_group.set_css_classes(&["settings-group-card"]);

        let row_track = GtkBox::new(Orientation::Horizontal, 12);
        row_track.set_css_classes(&["settings-group-row"]);
        let p_info = GtkBox::new(Orientation::Vertical, 2);
        p_info.set_hexpand(true);
        let p_title = Label::new(Some("Enhanced Tracking Protection"));
        p_title.set_css_classes(&["settings-row-title"]);
        p_title.set_xalign(0.0);
        let p_sub = Label::new(Some("Block known trackers and third-party cookies"));
        p_sub.set_css_classes(&["settings-row-subtitle"]);
        p_sub.set_xalign(0.0);
        p_info.append(&p_title);
        p_info.append(&p_sub);
        let p_switch = gtk4::Switch::new();
        p_switch.set_active(true);
        p_switch.set_valign(gtk4::Align::Center);
        row_track.append(&p_info);
        row_track.append(&p_switch);
        priv_group.append(&row_track);

        priv_page.append(&priv_group);
        content_stack.add_named(&priv_page, Some("privacy"));

        // ── Shortcuts Page ───────────────────────────────────────────────────
        let sc_page = GtkBox::new(Orientation::Vertical, 12);
        sc_page.set_css_classes(&["settings-page-body"]);

        let sc_sec_label = Label::new(Some("Keyboard Shortcuts"));
        sc_sec_label.set_css_classes(&["settings-group-header"]);
        sc_sec_label.set_xalign(0.0);
        sc_page.append(&sc_sec_label);

        let sc_group = GtkBox::new(Orientation::Vertical, 0);
        sc_group.set_css_classes(&["settings-group-card"]);

        let make_shortcut_row = |name: &str, shortcut: &str| -> GtkBox {
            let row = GtkBox::new(Orientation::Horizontal, 12);
            row.set_css_classes(&["settings-group-row"]);
            let lbl = Label::new(Some(name));
            lbl.set_hexpand(true);
            lbl.set_xalign(0.0);
            lbl.set_css_classes(&["settings-row-title"]);
            let sc_badge = Label::new(Some(shortcut));
            sc_badge.set_css_classes(&["settings-shortcut-badge"]);
            row.append(&lbl);
            row.append(&sc_badge);
            row
        };

        sc_group.append(&make_shortcut_row("Toggle Left Sidebar", "Ctrl + B"));
        sc_group.append(&gtk4::Separator::new(Orientation::Horizontal));
        sc_group.append(&make_shortcut_row("Open Settings", "Ctrl + ,"));
        sc_group.append(&gtk4::Separator::new(Orientation::Horizontal));
        sc_group.append(&make_shortcut_row("Focus Address Bar", "Ctrl + L"));
        sc_group.append(&gtk4::Separator::new(Orientation::Horizontal));
        sc_group.append(&make_shortcut_row("New Tab", "Ctrl + T"));
        sc_group.append(&gtk4::Separator::new(Orientation::Horizontal));
        sc_group.append(&make_shortcut_row("Close Tab", "Ctrl + W"));
        sc_group.append(&gtk4::Separator::new(Orientation::Horizontal));
        sc_group.append(&make_shortcut_row("Pin / Unpin Tab", "Ctrl + P"));

        sc_page.append(&sc_group);
        content_stack.add_named(&sc_page, Some("shortcuts"));

        // ── Nav Button Switching ─────────────────────────────────────────────
        let all_nav_btns = vec![
            (btn_appearance, "appearance", "Appearance"),
            (btn_general, "general", "General"),
            (btn_privacy, "privacy", "Privacy & Security"),
            (btn_shortcuts, "shortcuts", "Shortcuts"),
        ];

        let nav_buttons_list: Vec<Button> =
            all_nav_btns.iter().map(|(b, _, _)| b.clone()).collect();

        for (btn, tag, title) in all_nav_btns {
            let stack = content_stack.clone();
            let all_btns = nav_buttons_list.clone();
            let cur_btn = btn.clone();
            let p_lbl = page_title_label.clone();
            let title_str = title.to_string();
            let tag_str = tag.to_string();

            btn.connect_clicked(move |_| {
                for b in &all_btns {
                    b.remove_css_class("active");
                }
                cur_btn.add_css_class("active");
                p_lbl.set_text(&title_str);
                stack.set_visible_child_name(&tag_str);
            });
        }

        main_right_box.append(&content_stack);

        root_box.append(&settings_sidebar);
        root_box.append(&main_right_box);

        backdrop.append(&root_box);

        self.overlay.add_overlay(&backdrop);
        *self.active_settings_modal.borrow_mut() = Some(backdrop);
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
        /* ── Base Window ──────────────────────────────────────────────── */
        window, window.light {
            background-color: #f2f2f7;
            color: #1c1c1e;
        }

        /* ── Dark Mode Root ───────────────────────────────────────────── */
        window.dark {
            background-color: #141416;
            color: #f2f2f7;
        }

        /* ── Chrome panel & Sidebar panel ─────────────────────────────── */
        .chrome-panel {
            background: transparent;
        }

        .sidebar-panel {
            background: transparent;
            margin: 8px;
        }

        /* ── Left Sidebar (Zen-style) ────────────────────────────────── */
        .sidebar, window.light .sidebar {
            background-color: #f2f2f7;
            border: 1px solid rgba(0,0,0,0.08);
            border-radius: 12px;
            box-shadow: 0 8px 24px rgba(0,0,0,0.08);
            padding: 8px 8px 10px 8px;
        }

        window.dark .sidebar {
            background-color: #16161b;
            border: 1px solid rgba(255,255,255,0.07);
            border-radius: 14px;
            box-shadow: 0 10px 32px rgba(0,0,0,0.45);
        }

        .sidebar-header {
            min-height: 34px;
            padding: 4px 2px 12px;
        }

        .sidebar-title {
            font-size: 13.5px;
            font-weight: 700;
            letter-spacing: 0.2px;
            color: #1c1c1e;
        }

        /* Zen-style bottom bar */
        .sidebar-workspace-pill {
            min-height: 28px;
            padding: 4px 12px;
            border: 0;
            border-radius: 8px;
            color: #475569;
            background: transparent;
            background-image: none;
            box-shadow: none;
            font-size: 12.5px;
            font-weight: 700;
        }

        .sidebar-workspace-pill label {
            color: inherit;
            font-size: inherit;
            font-weight: inherit;
        }

        .sidebar-workspace-pill:hover {
            background: rgba(0,0,0,0.06);
            color: #1c1c1e;
        }

        .sidebar-icon-btn {
            min-width: 28px;
            min-height: 28px;
            padding: 4px;
            border: 0;
            border-radius: 7px;
            color: #64748b;
            background: transparent;
            background-image: none;
            box-shadow: none;
        }

        .sidebar-icon-btn:hover {
            color: #1c1c1e;
            background: rgba(0,0,0,0.07);
        }

        /* Favicon letter tiles */
        .tab-favicon {
            min-width: 20px;
            min-height: 20px;
            border-radius: 6px;
            font-size: 11px;
            font-weight: 700;
            color: #ffffff;
        }

        .fav-c0 { background-color: #6366f1; }
        .fav-c1 { background-color: #0ea5e9; }
        .fav-c2 { background-color: #10b981; }
        .fav-c3 { background-color: #f59e0b; }
        .fav-c4 { background-color: #ef4444; }
        .fav-c5 { background-color: #8b5cf6; }

        .sidebar-content {
            padding: 0 0 4px;
        }

        .sidebar-section-label {
            font-size: 10.5px;
            font-weight: 700;
            color: #64748b;
            margin: 6px 8px 2px;
            letter-spacing: 0.4px;
        }

        .sidebar-pinned-tabs,
        .sidebar-tabs {
            padding: 0;
        }

        .sidebar-tab-row,
        .sidebar-pinned-tab-row {
            min-height: 34px;
            min-width: 0;
            padding: 5px 8px;
            border-radius: 8px;
            border: 0;
            background: transparent;
            color: #334155;
        }

        .sidebar-pinned-tab-row {
            min-height: 34px;
        }

        .sidebar-tab-row:hover,
        .sidebar-pinned-tab-row:hover {
            background: rgba(0,0,0,0.06);
        }

        .sidebar-tab-row.active-tab,
        .sidebar-pinned-tab-row.active-tab {
            background: rgba(0,0,0,0.09);
            box-shadow: none;
        }

        .sidebar-tab-row label,
        .sidebar-pinned-tab-row label {
            color: #334155;
            font-size: 12.5px;
            font-weight: 600;
        }

        .sidebar-tab-row .tab-favicon,
        .sidebar-pinned-tab-row .tab-favicon {
            margin-right: 4px;
        }

        .sidebar-tab-row .tab-pin,
        .sidebar-pinned-tab-row .tab-pin {
            min-width: 0;
            min-height: 0;
            padding: 0;
            opacity: 0;
            background: transparent;
            background-image: none;
            border: 0;
            box-shadow: none;
        }

        .sidebar-tab-row .tab-close,
        .sidebar-pinned-tab-row .tab-close {
            min-width: 18px;
            min-height: 18px;
            padding: 2px;
            opacity: 0;
            color: #64748b;
            background: transparent;
            background-image: none;
            border: 0;
            box-shadow: none;
            border-radius: 999px;
        }

        .sidebar-tab-row:hover .tab-close,
        .sidebar-tab-row.active-tab .tab-close,
        .sidebar-pinned-tab-row:hover .tab-close,
        .sidebar-pinned-tab-row.active-tab .tab-close {
            opacity: 0.75;
        }

        .sidebar-new-tab-btn {
            min-height: 34px;
            margin-top: 2px;
            padding: 6px 10px;
            border: 0;
            border-radius: 8px;
            color: #475569;
            background: transparent;
            background-image: none;
            box-shadow: none;
        }

        .sidebar-new-tab-btn > image {
            opacity: 0.75;
        }

        .sidebar-new-tab-btn label {
            font-size: 12.5px;
            font-weight: 600;
            color: inherit;
        }

        .sidebar-new-tab-btn:hover {
            color: #1c1c1e;
            background: rgba(0,0,0,0.06);
        }

        window.dark .sidebar-title {
            color: #f2f2f7;
        }

        window.dark .sidebar-workspace-pill {
            color: #a6adbd;
            background: transparent;
        }

        window.dark .sidebar-workspace-pill:hover {
            color: #ffffff;
            background: rgba(255,255,255,0.07);
        }

        window.dark .sidebar-icon-btn {
            color: #a6adbd;
        }

        window.dark .sidebar-icon-btn:hover {
            color: #ffffff;
            background: rgba(255,255,255,0.10);
        }

        window.dark .tab-favicon {
            color: rgba(255,255,255,0.92);
        }

        window.dark .sidebar-section-label {
            color: #8b94a7;
        }

        window.dark .sidebar-tab-row,
        window.dark .sidebar-pinned-tab-row {
            background: transparent;
            color: #b6bcc9;
        }

        window.dark .sidebar-tab-row:hover,
        window.dark .sidebar-pinned-tab-row:hover {
            background: rgba(255,255,255,0.07);
        }

        window.dark .sidebar-tab-row.active-tab,
        window.dark .sidebar-pinned-tab-row.active-tab {
            background: rgba(255,255,255,0.11);
            color: #ffffff;
        }

        window.dark .sidebar-tab-row label,
        window.dark .sidebar-pinned-tab-row label {
            color: #e6e6eb;
        }

        window.dark .sidebar-tab-row .tab-pin,
        window.dark .sidebar-pinned-tab-row .tab-pin {
            color: #a6adbd;
        }

        window.dark .sidebar-tab-row .tab-pin:hover,
        window.dark .sidebar-pinned-tab-row .tab-pin:hover {
            background: rgba(255,255,255,0.10);
            color: #ffffff;
        }

        window.dark .sidebar-tab-row .tab-close:hover,
        window.dark .sidebar-pinned-tab-row .tab-close:hover {
            background: rgba(255,255,255,0.10);
        }

        window.dark .sidebar-new-tab-btn {
            color: #a6adbd;
            background: transparent;
        }

        window.dark .sidebar-new-tab-btn > image {
            opacity: 0.8;
        }

        window.dark .sidebar-new-tab-btn label {
            color: inherit;
        }

        window.dark .sidebar-new-tab-btn:hover {
            color: #ffffff;
            background: rgba(255,255,255,0.07);
        }

        .sidebar-bottom {
            padding-top: 6px;
            padding-left: 2px;
        }

        .sidebar-settings-btn, window.light .sidebar-settings-btn {
            min-width: 28px;
            min-height: 28px;
            padding: 4px;
            border-radius: 6px;
            color: #48484a;
            background: transparent;
            border: 0;
            box-shadow: none;
        }

        .sidebar-settings-btn:hover, window.light .sidebar-settings-btn:hover {
            color: #1c1c1e;
            background: rgba(0,0,0,0.07);
        }

        window.dark .sidebar-settings-btn {
            color: rgba(255,255,255,0.60);
            background: transparent;
        }

        window.dark .sidebar-settings-btn:hover {
            color: rgba(255,255,255,0.95);
            background: rgba(255,255,255,0.10);
        }

        /* ── Topbar ──────────────────────────────────────────────────── */
        .topbar, window.light .topbar {
            min-height: 44px;
            padding: 0 12px;
            background-color: #f7f7f8;
            box-shadow: 0 1px 6px rgba(0,0,0,0.08);
            border-bottom: 1px solid rgba(0,0,0,0.08);
        }

        window.dark .topbar {
            background-color: #1a1a1e;
            box-shadow: 0 1px 8px rgba(0,0,0,0.35);
            border-bottom: 1px solid rgba(255,255,255,0.08);
        }

        /* ── Traffic-light dots ────────────────────────────────── */
        .traffic-dots {
            margin-right: 10px;
        }

        .traffic-dot {
            min-width:  12px;
            min-height: 12px;
            padding: 0;
            margin: 0;
            border-radius: 999px;
            border: 0;
            box-shadow: inset 0 0 0 0.5px rgba(0,0,0,0.20);
        }
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

        .nav-group button, window.light .nav-group button {
            min-width:  26px;
            min-height: 26px;
            padding: 3px;
            border-radius: 6px;
            color: #48484a;
            background: transparent;
            border: 0;
            box-shadow: none;
        }

        .nav-group button:hover, window.light .nav-group button:hover {
            color: #1c1c1e;
            background: rgba(0,0,0,0.07);
        }

        window.dark .nav-group button {
            color: rgba(255,255,255,0.60);
            background: transparent;
        }

        window.dark .nav-group button:hover {
            color: rgba(255,255,255,0.95);
            background: rgba(255,255,255,0.10);
        }

        /* ── Tab strip & Scroll ────────────────────────────────── */
        .tab-scroll {
            background: transparent;
        }

        .tab-scroll scrollbar,
        .tab-scroll scrollbar trough {
            opacity: 0;
            border: none;
            background: transparent;
        }

        .tab-strip {
            margin: 0 2px;
            background: transparent;
        }

        .tab-row, window.light .tab-row {
            min-height: 28px;
            min-width: 30px;
            padding: 0 6px;
            border-radius: 8px;
            background: transparent;
            color: #636366;
            transition: background 100ms ease, color 100ms ease;
        }

        .tab-row label, window.light .tab-row label {
            color: #636366;
            font-size: 12px;
            font-weight: 400;
        }

        .tab-row:hover, window.light .tab-row:hover {
            background: rgba(0,0,0,0.05);
            color: #1c1c1e;
        }

        .tab-row:hover label, window.light .tab-row:hover label {
            color: #1c1c1e;
        }

        .tab-row.active-tab, window.light .tab-row.active-tab {
            background-color: #ffffff;
            color: #1c1c1e;
            box-shadow: 0 1px 3px rgba(0,0,0,0.08), 0 0 1px rgba(0,0,0,0.12);
        }

        .tab-row.active-tab label, window.light .tab-row.active-tab label {
            color: #1c1c1e;
            font-weight: 500;
        }

        .tab-row.pinned-tab, window.light .tab-row.pinned-tab {
            padding: 0 4px;
            min-width: 28px;
            border-radius: 8px;
            background: rgba(0,0,0,0.04);
            color: #636366;
        }

        .tab-row.pinned-tab label, window.light .tab-row.pinned-tab label {
            font-size: 11px;
            font-weight: 600;
            color: #636366;
        }

        .tab-row.pinned-tab.active-tab, window.light .tab-row.pinned-tab.active-tab {
            background-color: #ffffff;
            color: #1c1c1e;
        }

        .tab-row.pinned-tab.active-tab label, window.light .tab-row.pinned-tab.active-tab label {
            color: #1c1c1e;
        }

        window.dark .tab-row {
            background: transparent;
            color: rgba(255,255,255,0.65);
        }

        window.dark .tab-row label {
            color: rgba(255,255,255,0.65);
        }

        window.dark .tab-row:hover {
            background: rgba(255,255,255,0.08);
            color: rgba(255,255,255,0.95);
        }

        window.dark .tab-row:hover label {
            color: rgba(255,255,255,0.95);
        }

        window.dark .tab-row.active-tab {
            background-color: #2c2c34;
            color: #ffffff;
            box-shadow: 0 1px 3px rgba(0,0,0,0.30);
        }

        window.dark .tab-row.active-tab label {
            color: #ffffff;
        }

        window.dark .tab-row.pinned-tab {
            background: rgba(255,255,255,0.06);
            color: rgba(255,255,255,0.50);
        }

        window.dark .tab-row.pinned-tab label {
            color: rgba(255,255,255,0.50);
        }

        window.dark .tab-row.pinned-tab.active-tab {
            background-color: #2c2c34;
            color: #ffffff;
        }

        window.dark .tab-row.pinned-tab.active-tab label {
            color: #ffffff;
        }

        .tab-row .tab-close, window.light .tab-row .tab-close {
            padding: 0;
            min-width:  16px;
            min-height: 16px;
            border-radius: 999px;
            opacity: 0;
            color: #8e8e93;
            background: transparent;
            border: 0;
            box-shadow: none;
        }

        .tab-row:hover .tab-close,
        .tab-row.active-tab .tab-close {
            opacity: 0.70;
        }

        .tab-row .tab-close:hover, window.light .tab-row .tab-close:hover {
            opacity: 1.0;
            color: #1c1c1e;
            background: rgba(0,0,0,0.08);
        }

        window.dark .tab-row .tab-close {
            color: rgba(255,255,255,0.60);
        }

        window.dark .tab-row .tab-close:hover {
            color: #ffffff;
            background: rgba(255,255,255,0.18);
        }

        .tab-row .tab-pin {
            padding: 0;
            min-width: 0;
            min-height: 0;
            opacity: 0;
            background: transparent;
            background-image: none;
            border: 0;
            box-shadow: none;
        }

        /* ── URL bar (Compact left-side address bar) ───────────── */
        .urlbar, window.light .urlbar {
            min-height: 30px;
            min-width: 200px;
            padding: 0 14px;
            border-radius: 9px;
            background-color: rgba(118, 118, 128, 0.10);
            color: #202124;
            border: 1px solid rgba(60, 60, 67, 0.10);
            box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55);
            font-size: 12.5px;
            font-weight: 400;
            letter-spacing: 0.1px;
        }

        .sidebar-urlbar {
            margin: 2px 0 12px;
            border-radius: 10px;
            min-height: 32px;
        }

        .urlbar text, window.light .urlbar text {
            padding: 0;
            border: 0;
            background: transparent;
            color: #202124;
            box-shadow: none;
            font-size: 12.5px;
            font-weight: 400;
        }

        .urlbar > image {
            opacity: 0.55;
            margin-left: 2px;
            margin-right: 2px;
        }

        window.dark .urlbar > image {
            opacity: 0.65;
        }

        .urlbar:hover, window.light .urlbar:hover {
            background-color: rgba(118, 118, 128, 0.14);
            border-color: rgba(60, 60, 67, 0.16);
        }

        .urlbar:hover text, window.light .urlbar:hover text {
            background: transparent;
            color: #202124;
        }

        .urlbar:focus, window.light .urlbar:focus {
            background-color: #ffffff;
            border-color: rgba(0, 122, 255, 0.48);
            box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.10), inset 0 1px 0 rgba(255, 255, 255, 0.80);
        }

        .urlbar:focus text, window.light .urlbar:focus text {
            background: transparent;
            color: #111214;
        }

        window.dark .urlbar {
            background-color: rgba(255, 255, 255, 0.055);
            color: #e6e6eb;
            border-color: rgba(255, 255, 255, 0.075);
            box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.025);
        }

        window.dark .urlbar text {
            background: transparent;
            color: #e6e6eb;
        }

        window.dark .urlbar:hover {
            background-color: rgba(255, 255, 255, 0.075);
            border-color: rgba(255, 255, 255, 0.11);
        }

        window.dark .urlbar:hover text {
            background: transparent;
            color: #f4f4f7;
        }

        window.dark .urlbar:focus {
            background-color: #202027;
            border-color: rgba(94, 145, 255, 0.62);
            box-shadow: 0 0 0 3px rgba(70, 120, 255, 0.14), inset 0 1px 0 rgba(255, 255, 255, 0.035);
        }

        window.dark .urlbar:focus text {
            background: transparent;
            color: #ffffff;
        }

        /* ── Progress bar ──────────────────────────────────────── */
        .progress-thin {
            min-height: 2px;
        }

        /* ── New-tab button ────────────────────────────────────── */
        .browser-menu-button, window.light .browser-menu-button {
            min-width: 28px;
            min-height: 28px;
            padding: 4px;
            border: 0;
            border-radius: 8px;
            color: #636366;
            background: transparent;
        }

        .browser-menu-button:hover, window.light .browser-menu-button:hover {
            color: #1c1c1e;
            background: rgba(0,0,0,0.07);
        }

        window.dark .browser-menu-button {
            color: rgba(255,255,255,0.60);
        }

        window.dark .browser-menu-button:hover {
            color: #ffffff;
            background: rgba(255,255,255,0.10);
        }

        .new-tab-button, window.light .new-tab-button {
            min-width:  22px;
            min-height: 22px;
            padding: 2px;
            margin-left: 2px;
            opacity: 0.60;
            border-radius: 6px;
            background: transparent;
            border: 0;
            box-shadow: none;
            color: #636366;
        }

        .new-tab-button:hover, window.light .new-tab-button:hover {
            opacity: 1.0;
            color: #1c1c1e;
            background: rgba(0,0,0,0.07);
        }

        window.dark .new-tab-button {
            color: rgba(255,255,255,0.60);
        }

        window.dark .new-tab-button:hover {
            color: #ffffff;
            background: rgba(255,255,255,0.10);
        }

        /* ── Settings Modal Backdrop & Dialog (In-App Overlay) ─────────── */
        .settings-modal-backdrop {
            background-color: rgba(15, 23, 42, 0.32);
        }

        window.dark .settings-modal-backdrop {
            background-color: rgba(3, 5, 12, 0.70);
        }

        .settings-modal-dialog, window.light .settings-modal-dialog {
            background-color: #fbfcfe;
            color: #172033;
            border-radius: 18px;
            border: 1px solid rgba(15, 23, 42, 0.14);
            box-shadow: 0 32px 80px rgba(15, 23, 42, 0.30), 0 8px 24px rgba(15, 23, 42, 0.12);
        }

        .settings-modal-sidebar, window.light .settings-modal-sidebar {
            background-color: #f3f5f9;
            border-right: 1px solid #e2e7f0;
            border-top-left-radius: 18px;
            border-bottom-left-radius: 18px;
            padding: 16px 10px;
        }

        .settings-modal-title, .settings-modal-title label,
        window.light .settings-modal-title, window.light .settings-modal-title label {
            font-size: 15px;
            font-weight: 700;
            color: #475569;
            margin: 0 8px 10px;
            letter-spacing: 0.2px;
        }

        .settings-modal-nav-btn, window.light .settings-modal-nav-btn {
            min-height: 34px;
            padding: 5px 10px;
            border-radius: 10px;
            background: transparent;
            background-image: none;
            color: #64748b;
            border: 0;
            box-shadow: none;
            font-size: 13.5px;
            font-weight: 600;
            transition: background-color 120ms ease, color 120ms ease;
        }

        .settings-modal-nav-btn label, window.light .settings-modal-nav-btn label {
            padding: 0;
            border: 0;
            background: transparent;
            box-shadow: none;
            color: inherit;
            font-size: 13.5px;
            font-weight: inherit;
        }

        .settings-modal-nav-btn:hover, window.light .settings-modal-nav-btn:hover {
            background-color: #e8edf5;
            color: #1e293b;
        }

        .settings-modal-nav-btn:hover label, window.light .settings-modal-nav-btn:hover label {
            background: transparent;
            box-shadow: none;
            color: inherit;
        }

        .settings-modal-nav-btn.active, window.light .settings-modal-nav-btn.active {
            background-color: rgba(59, 130, 246, 0.11);
            color: #2563eb;
            font-weight: 600;
        }

        .settings-modal-nav-btn.active label, window.light .settings-modal-nav-btn.active label {
            background: transparent;
            box-shadow: none;
            color: inherit;
            font-weight: inherit;
        }

        .settings-nav-icon, window.light .settings-nav-icon {
            opacity: 0.90;
            color: #64748b;
        }

        .settings-modal-nav-btn.active .settings-nav-icon,
        window.light .settings-modal-nav-btn.active .settings-nav-icon {
            opacity: 1.0;
            color: #2563eb;
        }

        .settings-modal-content-area, window.light .settings-modal-content-area {
            background-color: #fbfcfe;
            padding: 0;
            border-top-right-radius: 18px;
            border-bottom-right-radius: 18px;
        }

        .settings-modal-top-header, window.light .settings-modal-top-header {
            min-height: 56px;
            padding: 12px 22px 11px;
            background-color: rgba(255, 255, 255, 0.48);
            border-bottom: 1px solid #e2e7f0;
        }

        .settings-modal-page-title, .settings-modal-page-title label,
        window.light .settings-modal-page-title, window.light .settings-modal-page-title label {
            font-size: 18px;
            font-weight: 700;
            color: #172033;
            letter-spacing: -0.2px;
        }

        .settings-modal-close-btn, window.light .settings-modal-close-btn {
            min-width: 30px;
            min-height: 30px;
            padding: 0;
            border-radius: 999px;
            background-color: #e8ecf3;
            background-image: none;
            color: #64748b;
            border: 0;
            box-shadow: none;
        }

        .settings-modal-close-btn:hover, window.light .settings-modal-close-btn:hover {
            background-color: #fee2e2;
            background-image: none;
            color: #dc2626;
        }

        .settings-page-body {
            padding: 16px 22px 20px;
        }

        /* Group Headers and Cards */
        .settings-group-header, .settings-group-header label,
        window.light .settings-group-header, window.light .settings-group-header label {
            font-size: 11px;
            font-weight: 700;
            color: #64748b;
            margin-top: 2px;
            margin-bottom: 4px;
            text-transform: uppercase;
            letter-spacing: 0.8px;
        }

        .settings-group-card, window.light .settings-group-card {
            background-color: #ffffff;
            border: 1px solid #e1e6ef;
            border-radius: 12px;
            box-shadow: 0 2px 8px rgba(15, 23, 42, 0.04);
        }

        .settings-group-row {
            min-height: 46px;
            padding: 8px 14px;
        }

        .settings-row-title, .settings-row-title label,
        window.light .settings-row-title, window.light .settings-row-title label {
            font-size: 13.5px;
            font-weight: 600;
            color: #1e293b;
        }

        .settings-row-subtitle, .settings-row-subtitle label,
        window.light .settings-row-subtitle, window.light .settings-row-subtitle label {
            font-size: 12.25px;
            color: #64748b;
        }

        .settings-dropdown-btn, window.light .settings-dropdown-btn {
            min-height: 30px;
            background-color: #f8fafc;
            background-image: none;
            border: 1px solid #cbd5e1;
            border-radius: 9px;
            color: #334155;
            font-size: 12px;
            font-weight: 600;
            padding: 4px 10px;
            box-shadow: none;
        }

        .settings-dropdown-btn label, window.light .settings-dropdown-btn label {
            padding: 0;
            border: 0;
            background: transparent;
            box-shadow: none;
            color: inherit;
            font-size: 12px;
            font-weight: inherit;
        }

        .settings-dropdown-btn:hover, window.light .settings-dropdown-btn:hover {
            background-color: #f1f5f9;
            border-color: #94a3b8;
            color: #0f172a;
        }

        .settings-dropdown-btn:hover label, window.light .settings-dropdown-btn:hover label {
            background: transparent;
            box-shadow: none;
            color: inherit;
        }

        .settings-shortcut-badge, .settings-shortcut-badge label,
        window.light .settings-shortcut-badge, window.light .settings-shortcut-badge label {
            background-color: #f1f5f9;
            background-image: none;
            border: 1px solid #dbe2ec;
            border-radius: 8px;
            padding: 3px 8px;
            font-size: 11.5px;
            font-weight: 600;
            color: #334155;
        }

        .settings-layout-switcher {
            background: #edf1f6;
            border: 1px solid #dbe2ec;
            border-radius: 10px;
            padding: 3px;
        }

        .settings-layout-btn,
        window.light .settings-layout-btn {
            min-height: 30px;
            padding: 5px 14px;
            border: 0;
            border-radius: 7px;
            color: #64748b;
            background: transparent;
            font-size: 12.5px;
            font-weight: 600;
        }

        .settings-layout-btn:hover,
        window.light .settings-layout-btn:hover {
            color: #1e293b;
            background: rgba(255,255,255,0.62);
        }

        .settings-layout-btn.active,
        window.light .settings-layout-btn.active {
            color: #2563eb;
            background: #ffffff;
            box-shadow: 0 2px 6px rgba(15,23,42,0.10);
        }

        window.dark .settings-layout-switcher {
            background: #1a1a21;
            border-color: #30303b;
        }

        window.dark .settings-layout-btn {
            color: #929aab;
        }

        window.dark .settings-layout-btn.active {
            color: #a8c0ff;
            background: #303747;
        }

        /* Segmented Theme Switcher */
        .settings-segmented-group, window.light .settings-segmented-group {
            background-color: #edf1f6;
            border-radius: 10px;
            padding: 3px;
            border: 1px solid #dbe2ec;
        }

        .settings-segmented-btn, window.light .settings-segmented-btn {
            min-height: 26px;
            padding: 4px 12px;
            border-radius: 7px;
            border: 0;
            background: transparent;
            color: #64748b;
            font-size: 12.5px;
            font-weight: 600;
            box-shadow: none;
            transition: background-color 120ms ease, color 120ms ease;
        }

        .settings-segmented-btn label, window.light .settings-segmented-btn label {
            padding: 0;
            border: 0;
            background: transparent;
            box-shadow: none;
            color: inherit;
            font-size: 12.5px;
            font-weight: inherit;
        }

        .settings-segmented-btn:hover, window.light .settings-segmented-btn:hover {
            color: #1e293b;
            background-color: rgba(255, 255, 255, 0.62);
        }

        .settings-segmented-btn:hover label, window.light .settings-segmented-btn:hover label {
            background: transparent;
            box-shadow: none;
            color: inherit;
        }

        .settings-segmented-btn.active, window.light .settings-segmented-btn.active {
            background-color: #ffffff;
            color: #2563eb;
            font-weight: 600;
            box-shadow: 0 2px 6px rgba(15, 23, 42, 0.10);
        }

        .settings-segmented-btn.active label, window.light .settings-segmented-btn.active label {
            background: transparent;
            box-shadow: none;
            color: inherit;
            font-weight: inherit;
        }

        separator, window.light separator,
        .settings-group-card separator, window.light .settings-group-card separator {
            min-height: 1px;
            background-color: #e5eaf1;
            border: 0;
        }

        /* ── Dark Mode for Settings Modal ──────────────────────────────── */
        window.dark .settings-modal-dialog,
        .settings-modal-dialog.dark {
            background-color: #1d1d24;
            color: #f4f4f7;
            border: 1px solid rgba(255, 255, 255, 0.11);
            box-shadow: 0 32px 80px rgba(0, 0, 0, 0.72), 0 8px 24px rgba(0, 0, 0, 0.38);
        }

        window.dark .settings-modal-sidebar,
        .settings-modal-sidebar.dark {
            background-color: #17171d;
            border-right: 1px solid #2b2b35;
            border-top-left-radius: 18px;
            border-bottom-left-radius: 18px;
        }

        window.dark .settings-modal-title,
        window.dark .settings-modal-title label,
        .settings-modal-title.dark,
        .settings-modal-title.dark label {
            color: #a6adbd;
        }

        window.dark .settings-modal-nav-btn,
        .settings-modal-nav-btn.dark {
            color: #9ba3b4;
            background: transparent;
        }

        window.dark .settings-modal-nav-btn label,
        .settings-modal-nav-btn.dark label {
            color: inherit;
            background: transparent;
            box-shadow: none;
        }

        window.dark .settings-modal-nav-btn:hover,
        .settings-modal-nav-btn.dark:hover {
            background-color: #242630;
            color: #f4f4f7;
        }

        window.dark .settings-modal-nav-btn:hover label,
        .settings-modal-nav-btn.dark:hover label {
            color: inherit;
            background: transparent;
            box-shadow: none;
        }

        window.dark .settings-modal-nav-btn.active,
        .settings-modal-nav-btn.dark.active {
            background-color: rgba(70, 120, 255, 0.14);
            color: #8fb0ff;
        }

        window.dark .settings-modal-nav-btn.active label,
        .settings-modal-nav-btn.dark.active label {
            color: inherit;
            background: transparent;
            box-shadow: none;
        }

        window.dark .settings-nav-icon,
        .settings-nav-icon.dark {
            color: #8d95a7;
        }

        window.dark .settings-modal-nav-btn.active .settings-nav-icon,
        .settings-modal-nav-btn.dark.active .settings-nav-icon {
            color: #8fb0ff;
        }

        window.dark .settings-modal-content-area,
        .settings-modal-content-area.dark {
            background-color: #1d1d24;
            border-top-right-radius: 18px;
            border-bottom-right-radius: 18px;
        }

        window.dark .settings-modal-top-header,
        .settings-modal-top-header.dark {
            background-color: rgba(255, 255, 255, 0.018);
            border-bottom: 1px solid #2b2b35;
        }

        window.dark .settings-modal-page-title,
        window.dark .settings-modal-page-title label,
        .settings-modal-page-title.dark,
        .settings-modal-page-title.dark label {
            color: #f7f7fa;
        }

        window.dark .settings-modal-close-btn,
        .settings-modal-close-btn.dark {
            background-color: #2a2e39;
            background-image: none;
            color: #aab1c2;
        }

        window.dark .settings-modal-close-btn:hover,
        .settings-modal-close-btn.dark:hover {
            background-color: rgba(239, 68, 68, 0.16);
            background-image: none;
            color: #fca5a5;
        }

        window.dark .settings-group-header,
        window.dark .settings-group-header label,
        .settings-group-header.dark,
        .settings-group-header.dark label {
            color: #7f899d;
        }

        window.dark .settings-group-card,
        .settings-group-card.dark {
            background-color: #24242d;
            border-color: #30303b;
            box-shadow: 0 4px 14px rgba(0, 0, 0, 0.18);
        }

        window.dark .settings-row-title,
        window.dark .settings-row-title label,
        .settings-row-title.dark,
        .settings-row-title.dark label {
            color: #f1f2f6;
        }

        window.dark .settings-row-subtitle,
        window.dark .settings-row-subtitle label,
        .settings-row-subtitle.dark,
        .settings-row-subtitle.dark label {
            color: #929aab;
        }

        window.dark .settings-dropdown-btn,
        .settings-dropdown-btn.dark {
            background-color: #2a2a34;
            background-image: none;
            border: 1px solid #3c3c49;
            color: #e8e9ee;
        }

        window.dark .settings-dropdown-btn label,
        .settings-dropdown-btn.dark label {
            background: transparent;
            box-shadow: none;
            color: inherit;
        }

        window.dark .settings-dropdown-btn:hover,
        .settings-dropdown-btn.dark:hover {
            background-color: #32323e;
            background-image: none;
            border-color: #505064;
            color: #ffffff;
        }

        window.dark .settings-dropdown-btn:hover label,
        .settings-dropdown-btn.dark:hover label {
            background: transparent;
            box-shadow: none;
            color: inherit;
        }

        window.dark .settings-shortcut-badge,
        window.dark .settings-shortcut-badge label,
        .settings-shortcut-badge.dark,
        .settings-shortcut-badge.dark label {
            background-color: #2a2a34;
            background-image: none;
            border: 1px solid #3c3c49;
            color: #c8cdd8;
        }

        window.dark .settings-segmented-group,
        .settings-modal-dialog.dark .settings-segmented-group {
            background-color: #1a1a21;
            border-color: #30303b;
        }

        window.dark .settings-segmented-btn,
        .settings-modal-dialog.dark .settings-segmented-btn {
            color: #929aab;
        }

        window.dark .settings-segmented-btn label,
        .settings-modal-dialog.dark .settings-segmented-btn label {
            color: inherit;
            background: transparent;
            box-shadow: none;
        }

        window.dark .settings-segmented-btn:hover,
        .settings-modal-dialog.dark .settings-segmented-btn:hover {
            color: #f4f4f7;
            background-color: rgba(255, 255, 255, 0.055);
        }

        window.dark .settings-segmented-btn:hover label,
        .settings-modal-dialog.dark .settings-segmented-btn:hover label {
            color: inherit;
            background: transparent;
            box-shadow: none;
        }

        window.dark .settings-segmented-btn.active,
        .settings-modal-dialog.dark .settings-segmented-btn.active {
            background-color: #303747;
            color: #a8c0ff;
            box-shadow: 0 2px 8px rgba(0, 0, 0, 0.30);
        }

        window.dark .settings-segmented-btn.active label,
        .settings-modal-dialog.dark .settings-segmented-btn.active label {
            color: inherit;
            background: transparent;
            box-shadow: none;
        }

        window.dark separator,
        .settings-group-card.dark separator {
            background-color: #30303b;
            border: 0;
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
