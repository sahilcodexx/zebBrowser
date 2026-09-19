Lightweight Native Linux Browser — MVP
Goal

Build a lightweight, native web browser for Linux, primarily targeting Arch Linux.

The browser should be intentionally minimal. The goal is to provide a fast, clean browser with only the essential functionality:

Open websites

Navigate backward/forward

Reload pages

Enter URLs/search queries

Multiple tabs

Create and close tabs

Switch between tabs

Basic keyboard shortcuts

Clean native Linux UI

Do not implement unnecessary browser features in the MVP.

1. Technology Stack
Programming Language

Rust

Use stable Rust.

Reasons:

Memory safety

Good performance

Native compiled binary

No garbage collector

Suitable for long-running applications

Good fit for system-level Linux applications

GUI Toolkit

GTK4

Use GTK4 for the native application UI.

GTK should be responsible for:

Main application window

Header/navigation bar

Buttons

URL/address entry

Tab interface

Layout

Keyboard shortcuts

Application lifecycle

Do NOT create the browser UI using HTML/CSS/JavaScript.

The UI must be a real native GTK application.

Browser Engine

WebKitGTK 6.0

Use WebKitGTK as the embedded browser/rendering engine.

WebKitGTK should handle:

HTML

CSS

JavaScript

HTTP/HTTPS

Page rendering

Web APIs

Cookies/session behavior

TLS

Web content

Do not implement a custom browser engine.

Do not embed Chromium.

Do not use Electron.

Rust GTK/WebKit Bindings

Use the appropriate Rust bindings for:

GTK4

WebKitGTK 6.0

GLib/GIO

Prefer actively maintained official/standard Rust GTK ecosystem bindings.

Keep the dependency list minimal.

Build System

Use:

Cargo

The project should build with:

cargo build


and run with:

cargo run


For release builds:

cargo build --release

2. Target Platform

Primary target:

Linux

Initial development/testing environment:

Arch Linux

The application should use native Linux/GTK technologies rather than platform abstraction layers such as Electron.

Do not optimize for Windows/macOS in the MVP.

3. MVP UI

The browser should have one main window.

Example layout:

┌──────────────────────────────────────────────────────────┐
│  ←  →  ⟳   [ https://example.com                 ]  +   │
├──────────────────────────────────────────────────────────┤
│  Example       GitHub        YouTube              +     │
├──────────────────────────────────────────────────────────┤
│                                                          │
│                                                          │
│                    WebKitWebView                         │
│                                                          │
│                                                          │
└──────────────────────────────────────────────────────────┘


The exact visual design can be improved later.

Prioritize functionality over visual polish.

4. Navigation Controls

The browser should have:

Back

Navigate to the previous page.

Keyboard shortcut:

Alt + Left

Forward

Navigate to the next page.

Keyboard shortcut:

Alt + Right

Reload

Reload the current page.

Keyboard shortcut:

Ctrl + R


Also support:

F5


if convenient.

5. Address Bar

Provide a native GTK text entry for URLs.

Examples:

https://example.com
https://github.com
https://news.ycombinator.com


Pressing:

Enter


should navigate the current tab.

URL Handling

If the entered text looks like a valid URL:

example.com


convert it to:

https://example.com


If the user enters a search query instead of a URL, perform a web search.

For example:

rust gtk tutorial


can be converted into a configurable search-engine URL.

Use a simple default search engine.

The search engine should be easy to change later.

6. Tabs

Tabs are a core MVP feature.

Use GTK's tab functionality or an appropriate GTK4 tab widget.

Each tab should contain its own:

WebKitWebView


Example:

Window
│
├── Tab 1
│   └── WebKitWebView
│
├── Tab 2
│   └── WebKitWebView
│
└── Tab 3
    └── WebKitWebView

New Tab

Provide a + button.

Keyboard shortcut:

Ctrl + T


Opening a new tab should display a simple start page or empty page.

Close Tab

Each tab should have a close button.

Keyboard shortcut:

Ctrl + W


If the last tab is closed, either:

Create a new empty tab, or

Close the browser window.

Prefer keeping one tab open.

Switch Tabs

Support:

Ctrl + Tab


and preferably:

Ctrl + Shift + Tab


Also support:

Ctrl + 1
Ctrl + 2
Ctrl + 3
...


for switching directly to tabs where practical.

7. Page Loading

The UI should provide basic loading feedback.

For example:

[██████████░░░░░░░░]


or a GTK progress indicator.

The address bar should update when navigation occurs.

When navigating:

https://example.com/page


the address bar should display the current URL.

8. Page Title

When a page changes its title:

Example Website


update the corresponding browser tab title.

For example:

┌───────────────────────────┐
│ Example Website       ×   │
└───────────────────────────┘


If a page has no useful title, use a reasonable fallback.

9. Basic Keyboard Shortcuts

Implement:

Ctrl + T       New tab
Ctrl + W       Close tab
Ctrl + R       Reload
Ctrl + L       Focus address bar
Ctrl + Tab     Next tab
Ctrl + Shift + Tab
               Previous tab

Alt + Left     Back
Alt + Right    Forward

Escape         Stop loading / clear relevant UI state


Also allow:

Enter


in the address bar to navigate.

10. Focus Behavior

When:

Ctrl + L


is pressed:

Focus the address bar.

Select the existing URL.

Allow the user to immediately type a new URL.

After navigation, focus should normally return to the webpage.

11. Context Menu

A minimal context menu may be implemented if WebKitGTK makes it straightforward.

Possible MVP actions:

Back
Forward
Reload
Copy Link
Open Link in New Tab


However, this is secondary.

Do not spend significant development time on advanced context menus during the MVP.

12. Error Handling

If a webpage fails to load, display a simple native/browser error page.

Example:

Unable to load this page

https://example.com

The webpage could not be loaded.

[ Try Again ]


Do not crash the entire browser because one page fails.

13. Security

Use WebKitGTK's normal security mechanisms.

Do not disable:

TLS certificate validation

Same-origin protections

Web security

Sandboxing/security features provided by WebKitGTK

Do not implement custom HTTPS handling.

The browser should rely on WebKitGTK for normal web security behavior.

14. Architecture

Keep the code modular but don't over-engineer it.

Suggested structure:

light-browser/
│
├── Cargo.toml
├── Cargo.lock
├── README.md
│
└── src/
    ├── main.rs
    ├── app.rs
    ├── window.rs
    ├── browser.rs
    ├── tab.rs
    ├── navigation.rs
    └── config.rs


Possible responsibilities:

main.rs

Application entry point.

app.rs

GTK application initialization and lifecycle.

window.rs

Main browser window.

Responsible for:

Window

Header bar

Navigation controls

Address bar

Tab container

browser.rs

Browser/WebKit integration.

Responsible for:

Creating WebKitWebView

Loading URLs

Navigation

Loading state

tab.rs

Tab management.

Responsible for:

Creating tabs

Closing tabs

Switching tabs

Tab titles

Tab-specific WebKitWebView

navigation.rs

Navigation behavior.

Responsible for:

URL parsing

Search queries

Back

Forward

Reload

config.rs

Minimal configuration.

For the MVP this could contain:

Home page

Search engine URL

Avoid implementing a large configuration system.

15. Performance Goals

The browser should prioritize low overhead.

Avoid:

Electron

Chromium Embedded Framework

Node.js

Web-based UI

JavaScript application frameworks

Large unnecessary dependencies

Background services

Constant polling loops

The browser UI should be event-driven.

Do not create unnecessary threads.

Do not continuously poll WebKit state.

Use GTK/GLib signals/events appropriately.

16. Memory Considerations

The application should be lightweight, but do not make unrealistic claims about total RAM usage.

WebKitGTK is a full modern browser engine, so websites themselves may consume substantial memory.

Optimize the browser's own overhead rather than attempting unsafe memory tricks.

Do not sacrifice stability or security just to reduce memory usage.

17. Configuration

For the MVP, keep configuration minimal.

Possible defaults:

Homepage:
https://example.com

Search engine:
Configurable later


Do not build a settings window yet.

Configuration can initially be represented by a small Rust configuration module.

18. Features Explicitly NOT in MVP

Do NOT implement these yet:

Bookmarks

History UI

Downloads manager

Extensions

Developer tools

Password manager

Autofill

Sync

Accounts

Profiles

Private/incognito mode

Ad blocker

Custom JavaScript injection

Themes

Plugin system

Browser extensions API

Complex settings UI

Session restoration

Crash reporting

Telemetry

Cloud sync

These may be considered after the basic browser is stable.

19. Application Identity

Use a proper Linux application ID, for example:

com.example.LightBrowser


This can be changed later.

The application should eventually include:

.desktop
icon
application metadata


but these are secondary to the functional MVP.

20. Development Philosophy

Keep the implementation simple.

The priority order is:

Browser launches.

A webpage renders.

URL navigation works.

Back/forward/reload work.

Tabs work.

Keyboard shortcuts work.

Loading/title/address-bar synchronization works.

Error handling works.

Polish and optimization.

Do not prematurely implement advanced browser functionality.

Avoid unnecessary abstractions.

Prefer straightforward Rust code that is easy to understand and modify.

21. Definition of Done

The MVP is complete when the following workflow works reliably:

Launch browser
      ↓
Browser opens with one tab
      ↓
Enter https://example.com
      ↓
Press Enter
      ↓
Website loads
      ↓
Open another tab
      ↓
Navigate to another website
      ↓
Switch between tabs
      ↓
Use Back / Forward
      ↓
Reload page
      ↓
Close tab
      ↓
Create another tab


The application should remain responsive throughout.

A crash in one webpage must not unnecessarily crash the browser application.

22. Important Implementation Rule

Before writing code, verify the exact currently supported Rust bindings/API versions for:

GTK4

WebKitGTK 6.0

GLib/GIO

Do not invent APIs.

If an API has changed between versions, use the API available in the installed Arch Linux packages.

The resulting project should compile on a current Arch Linux installation with the required development packages installed.

23. First Milestone

The first implementation should be deliberately small.

Create:

MainWindow
    │
    ├── Navigation bar
    │     ├── Back
    │     ├── Forward
    │     ├── Reload
    │     └── Address Entry
    │
    └── Tab container
          └── WebKitWebView


Get this working first.

Then implement multiple tabs.

Do not implement anything outside the MVP until these features are functional.
