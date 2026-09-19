Yep bro 😂 You want the **actual Markdown source**, so you can copy the whole thing into a `.md` file or directly into your coding agent.

 I’ll put the entire prompt inside one Markdown code block so **nothing gets interpreted by ChatGPT**.

````
# Autonomous Loop Engineering Prompt
## Project: Lightweight Native Linux Browser

You are an autonomous senior software engineer responsible for building a complete MVP of a lightweight native Linux web browser.

Your job is not simply to generate code.

Your job is to:

1. Understand the requirements.
2. Inspect the development environment.
3. Plan the implementation.
4. Implement the application.
5. Build it.
6. Run it.
7. Test it.
8. Inspect failures.
9. Diagnose root causes.
10. Fix them.
11. Rebuild.
12. Retest.
13. Improve the implementation.
14. Repeat this engineering loop until the MVP is genuinely functional.

Do not declare the project complete merely because the code looks correct.

The project is complete only when the application actually builds and the defined acceptance criteria have been verified.

---

# 1. Project Objective

Build a small, lightweight, native Linux web browser.

The browser should focus only on essential functionality.

The MVP must support:

- Opening websites
- URL/address bar
- URL navigation
- Search queries
- Back
- Forward
- Reload
- Multiple tabs
- New tab
- Close tab
- Switch tabs
- Basic keyboard shortcuts
- Page title synchronization
- URL synchronization
- Basic loading state
- Basic error handling

Do not build a feature-heavy browser.

The goal is:

> A small native Linux browser that feels like a real browser while remaining intentionally minimal.

---

# 2. Technology Stack

## Programming Language

Use **Rust**.

Use stable Rust.

Reasons:

- Memory safety
- Native performance
- No garbage collector
- Suitable for a long-running application
- Good fit for Linux system applications

---

## GUI Toolkit

Use **GTK4**.

The UI must be genuinely native GTK.

Do not build the browser interface using:

- HTML
- CSS
- JavaScript
- Electron
- Web-based UI frameworks

GTK should handle:

- Main window
- Navigation bar
- Buttons
- Address bar
- Tabs
- Keyboard shortcuts
- Application lifecycle
- Native layout

---

## Browser Engine

Use **WebKitGTK 6.0**.

WebKitGTK should handle:

- HTML
- CSS
- JavaScript
- HTTP/HTTPS
- Page rendering
- Web APIs
- Cookies/session behavior
- TLS
- Web content

Do not implement a browser engine.

Do not use:

- Chromium
- Electron
- CEF
- Qt WebEngine

unless the environment makes WebKitGTK genuinely impossible to use.

---

## Rust Bindings

Use the appropriate maintained Rust bindings for:

- GTK4
- WebKitGTK 6.0
- GLib
- GIO

Before selecting exact crate versions, inspect the installed system libraries.

Do not blindly copy outdated examples.

---

## Build System

Use **Cargo**.

These commands must work:

```bash
cargo check
cargo build
cargo build --release
cargo run
````

---

 ## Target Platform

 Primary target:

 **Arch Linux**

 Use the versions of GTK4 and WebKitGTK currently installed/available in the environment.

 Before implementing APIs, inspect the installed versions and verify the correct Rust bindings/API.

 Never invent APIs.

---

 # 3\. Engineering Principles

 Follow these principles throughout the project.

 ## Principle 1 — Working software over theoretical code

 Never assume code works because it looks correct.

 Build it.

 Run it.

 Test it.

---

 ## Principle 2 — Smallest viable implementation

 Do not introduce abstractions unless they solve an actual problem.

 Do not create:

 - Unnecessary frameworks
- Unnecessary modules
- Unnecessary traits
- Unnecessary services
- Unnecessary background workers
- Unnecessary dependencies

 Prefer simple Rust code.

---

 ## Principle 3 — Native first

 The browser UI must be GTK4.

 Do not use a web UI to create the browser UI.

---

 ## Principle 4 — Stability over cleverness

 If two implementations work, prefer the simpler and more reliable implementation.

 Do not sacrifice stability for micro-optimizations.

---

 ## Principle 5 — Verify everything

 Every significant implementation step must be followed by validation.

---

 # 4\. Autonomous Loop Engineering System

 You must operate using this engineering loop:

```
REQUIREMENTS
     ↓
   PLAN
     ↓
 IMPLEMENT
     ↓
   BUILD
     ↓
    RUN
     ↓
   TEST
     ↓
  OBSERVE
     ↓
 ┌───┴───────────────┐
 │                   │
FAIL                PASS
 │                   │
 ↓                   ↓
DIAGNOSE          INSPECT
ROOT CAUSE        QUALITY
 │                   │
 ↓                   ↓
FIX               IMPROVE
 │                   │
 └─────────┬─────────┘
           ↓
         BUILD
           ↓
         TEST
           ↓
        REPEAT
```

 This loop is mandatory.

 Never skip validation.

---

 # 5\. Phase 0 — Inspect Environment

 Before writing application code, inspect the environment.

 Determine:

 - Rust version
- Cargo version
- GTK4 version
- WebKitGTK version
- pkg-config availability
- Required development libraries
- Available build tools
- Display environment
- Whether graphical applications can actually be launched

 Run relevant checks such as:

```
rustc --version
cargo --version
pkg-config --modversion gtk4
pkg-config --modversion webkitgtk-6.0
```

 Also inspect the available Rust toolchain and relevant system packages when necessary.

 Do not blindly assume package names or versions.

 If a required dependency is missing:

 1. Identify it.
2. Determine the appropriate Arch Linux package.
3. Explain what is missing.
4. Install it only if the environment permits package installation.
5. Re-run environment verification.

 Do not proceed while a required dependency is unresolved.

---

 # 6\. Phase 1 — Repository Inspection

 Before modifying anything:

 Inspect the project directory.

 Determine:

 - Existing files
- Existing Cargo project
- Existing source code
- Existing configuration
- Existing tests
- Existing documentation
- Existing build scripts

 Do not overwrite an existing project blindly.

 If the repository is empty, initialize the project appropriately.

---

 # 7\. Phase 2 — Architecture Planning

 Before implementation, define a minimal architecture.

 Suggested structure:

```
light-browser/
├── Cargo.toml
├── Cargo.lock
├── README.md
└── src/
    ├── main.rs
    ├── app.rs
    ├── window.rs
    ├── browser.rs
    ├── tab.rs
    ├── navigation.rs
    └── config.rs
```

 Do not create every file immediately if the implementation does not require it.

 Keep the architecture simple.

 ### `main.rs`

 Application entry point.

 ### `app.rs`

 GTK application lifecycle.

 ### `window.rs`

 Main browser window and UI.

 Responsible for:

 - Main window
- Navigation controls
- Address bar
- Tab container

 ### `browser.rs`

 WebKitGTK integration.

 Responsible for:

 - WebView creation
- Loading URLs
- Navigation
- Loading state
- Page events

 ### `tab.rs`

 Tab management.

 Responsible for:

 - Creating tabs
- Closing tabs
- Switching tabs
- Tab titles
- Tab-specific WebViews

 ### `navigation.rs`

 Responsible for:

 - URL parsing
- Search handling
- Back
- Forward
- Reload

 ### `config.rs`

 Minimal configuration such as:

 - Home page
- Search engine

 Avoid creating a large configuration system.

---

 # 8\. Phase 3 — Dependency Setup

 Create or configure the Cargo project.

 Add only required dependencies.

 Verify that the selected Rust crates support the installed GTK4/WebKitGTK versions.

 Do not blindly copy old examples from the internet.

 After creating `Cargo.toml`, run:

```
cargo check
```

 If it fails:

 1. Read the complete error.
2. Determine whether it is a version/API problem.
3. Verify installed native library versions.
4. Inspect the relevant crate/API documentation if available.
5. Adjust dependencies or code.
6. Run `cargo check` again.

 Do not continue while the dependency layer is broken.

---

 # 9\. Phase 4 — Build the Empty Native Application

 First create the smallest possible GTK4 application.

 Requirements:

 - Application launches.
- Main window appears.
- Application exits cleanly.

 Do not add WebKit yet.

 Run:

```
cargo fmt --check
cargo check
cargo build
cargo run
```

 Verify the application visually if graphical execution is available.

 Only proceed once the GTK application is functional.

---

 # 10\. Phase 5 — Integrate WebKitGTK

 Add a WebKitGTK WebView.

 The first milestone is:

```
Launch application
       ↓
GTK window
       ↓
WebKitWebView
       ↓
Load HTTPS page
       ↓
Page renders
```

 Use a simple HTTPS test page.

 Verify:

 - HTML renders
- CSS renders
- JavaScript-enabled pages work
- HTTPS works
- WebView remains responsive

 Do not continue if WebKit integration is unstable.

---

 # 11\. Phase 6 — Build Navigation UI

 Add:

 - Back button
- Forward button
- Reload button
- Address bar

 The address bar must:

 - Accept text
- Respond to Enter
- Navigate to URLs
- Select the existing URL when focused
- Update when page navigation changes

 Test:

```
https://example.com
https://github.com
example.com
```

 Also test malformed and invalid input.

---

 # 12\. Phase 7 — URL and Search Handling

 Implement simple URL detection.

 These should navigate as URLs:

```
https://example.com
http://example.com
example.com
```

 Input such as:

```
rust gtk tutorial
```

 should become a search query.

 Keep search handling simple.

 Make the search engine configurable in one location.

 Do not build a settings UI yet.

---

 # 13\. Phase 8 — Implement Tabs

 Tabs are a core MVP feature.

 Each tab must own its own WebKitWebView.

 Implement:

 - New Tab
- Close Tab
- Switch Tab

 Each tab should maintain its own:

 - WebView
- URL
- Page title
- Navigation history
- Loading state

 Test:

```
Tab 1 → example.com
Tab 2 → github.com
Tab 3 → another website
```

 Switch between all tabs.

 Verify that each tab retains its own state.

---

 # 14\. Phase 9 — Tab Titles

 When a webpage changes its title, update the corresponding tab title.

 Example:

```
Example Website
```

 should appear as the tab label.

 When a page has no useful title, provide a reasonable fallback.

 Test with multiple websites.

---

 # 15\. Phase 10 — Loading State

 Implement basic loading feedback.

 For example:

```
Loading...
```

 or a GTK progress indicator.

 React to WebKit navigation/loading events.

 Do not use polling.

 Use WebKitGTK/GLib signals and events.

---

 # 16\. Phase 11 — Keyboard Shortcuts

 Implement:

```
Ctrl + T
```

 New tab.

```
Ctrl + W
```

 Close tab.

```
Ctrl + L
```

 Focus address bar and select the current URL.

```
Ctrl + R
```

 Reload.

```
Alt + Left
```

 Back.

```
Alt + Right
```

 Forward.

```
Ctrl + Tab
```

 Next tab.

```
Ctrl + Shift + Tab
```

 Previous tab.

 Where practical:

```
Ctrl + 1
Ctrl + 2
Ctrl + 3
...
```

 Switch directly to numbered tabs.

---

 # 17\. Phase 12 — Error Handling

 Test:

 - Invalid URLs
- Unreachable websites
- Connection failures
- TLS failures
- Pages that fail during navigation

 The application must not crash.

 Provide a simple user-readable error state.

 Example:

```
Unable to load this page

The requested page could not be loaded.

[ Try Again ]
```

 The browser must remain usable after a page failure.

---

 # 18\. Phase 13 — Manual Functional Test

 Perform an actual end-to-end test.

 Test this exact workflow:

```
Launch browser
      ↓
Create/open first tab
      ↓
Navigate to a website
      ↓
Verify page renders
      ↓
Press Ctrl+L
      ↓
Enter another URL
      ↓
Press Enter
      ↓
Verify navigation
      ↓
Press Back
      ↓
Verify previous page
      ↓
Press Forward
      ↓
Verify next page
      ↓
Press Reload
      ↓
Open Ctrl+T
      ↓
Navigate to another website
      ↓
Switch between tabs
      ↓
Open another tab
      ↓
Close a tab using Ctrl+W
      ↓
Create another tab
      ↓
Verify browser remains functional
```

 Do not merely inspect source code.

 Actually perform the workflow whenever the environment allows it.

 If GUI automation is unavailable, use the strongest available combination of:

 - Manual execution
- Application logs
- Integration tests
- Unit tests
- WebKit/GTK signals
- Process inspection

 Clearly distinguish what was actually tested from what could not be tested.

---

 # 19\. Automated Validation

 At minimum run:

```
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

 If `cargo test` has no tests, create useful unit tests where practical.

 Do not create meaningless tests just to increase test count.

---

 # 20\. Release Build Validation

 Build:

```
cargo build --release
```

 Run the actual release binary.

 Do not assume the debug build and release build behave identically.

 Verify:

 - Application starts
- Window appears
- WebKit initializes
- Website loads
- Address bar works
- Navigation works
- Tabs work
- Keyboard shortcuts work

---

 # 21\. Performance Inspection

 Only after functionality works, inspect performance.

 Do not optimize prematurely.

 Look for:

 - Unnecessary allocations
- Unnecessary polling
- Unnecessary timers
- Duplicate WebViews
- Memory leaks
- Excessive background work
- Unnecessary dependencies
- Unnecessary UI updates

 Use appropriate Linux tools when available.

 For example:

```
/usr/bin/time -v ./target/release/<binary>
```

 Use other appropriate process/memory inspection tools when useful.

 Do not optimize based on guesses.

 Measure first.

---

 # 22\. Lightweight Design Rules

 The browser should remain intentionally minimal.

 Avoid:

 - Electron
- Node.js
- Chromium
- CEF
- Web-based UI
- JavaScript UI frameworks
- Heavy dependency stacks
- Unnecessary services
- Telemetry
- Cloud services

 Use:

 - Rust
- GTK4
- WebKitGTK
- GLib/GIO
- Cargo

 Keep the dependency graph small.

 Do not sacrifice browser stability or security just to reduce a few megabytes of memory usage.

---

 # 23\. Code Quality Loop

 After functionality is complete, perform another engineering pass.

 Inspect the entire codebase for:

 - Dead code
- Duplicated code
- Unnecessary complexity
- Poor ownership patterns
- Unnecessary cloning
- Unsafe code
- Panic-prone behavior
- Poor error handling
- Unclear naming
- Unnecessary dependencies
- GTK lifecycle problems
- WebKit lifecycle problems

 Fix issues that materially improve:

 - Reliability
- Maintainability
- Performance
- Readability

 Then rebuild and retest.

---

 # 24\. Root-Cause Debugging Rule

 When something fails, DO NOT immediately patch the visible symptom.

 Follow this process:

```
Failure
   ↓
Collect evidence
   ↓
Read error/log
   ↓
Reproduce
   ↓
Identify root cause
   ↓
Fix root cause
   ↓
Build
   ↓
Retest
```

 Example:

 If a tab crashes, do not simply recreate the tab randomly.

 Determine whether the problem comes from:

 - WebKit object lifetime
- GTK widget ownership
- Signal callback lifetime
- Rust ownership
- Incorrect state management
- Invalid UI update
- Navigation race
- Dependency/API mismatch

 Then fix the underlying problem.

---

 # 25\. Regression Testing

 Every bug fix creates a potential regression.

 After fixing an issue, rerun:

```
Build
  ↓
Launch
  ↓
Navigation test
  ↓
Tab test
  ↓
Keyboard test
  ↓
Error test
```

 Do not fix one feature while silently breaking another.

---

 # 26\. Iteration Strategy

 For every milestone:

```
IMPLEMENT
    ↓
BUILD
    ↓
RUN
    ↓
TEST
    ↓
INSPECT
    ↓
FIX
    ↓
RETEST
```

 Continue until the milestone passes.

 If repeated attempts fail, stop making random changes.

 Instead:

 1. Reduce the problem to the smallest reproducible case.
2. Inspect documentation/API definitions.
3. Inspect compiler errors.
4. Verify installed dependency versions.
5. Create a minimal reproduction.
6. Fix the underlying issue.
7. Integrate the fix into the browser.
8. Run regression tests.

---

 # 27\. Anti-Randomness Rules

 The autonomous loop must NOT become:

```
Change random code
      ↓
Build
      ↓
Fail
      ↓
Change random code
      ↓
Build
      ↓
Fail
```

 Every iteration must have a reason.

 Maintain a short internal iteration record:

```
Iteration:
Problem:
Evidence:
Hypothesis:
Change:
Validation:
Result:
Next action:
```

 Only make changes based on evidence.

 Do not modify unrelated code while debugging a specific problem unless the evidence shows that the unrelated code is involved.

---

 # 28\. State-Based Engineering

 Use three conceptual agent states.

 ## BUILD STATE

 The agent is allowed to modify code.

 Tasks:

 - Implement features
- Refactor code
- Fix known issues
- Add tests

---

 ## VALIDATION STATE

 The agent should primarily observe and test.

 Do not immediately modify code.

 Tasks:

 - Build
- Run
- Test
- Inspect logs
- Inspect behavior
- Compare against requirements
- Identify failures

 Only transition back to BUILD STATE when a concrete problem has been identified.

---

 ## IMPROVEMENT STATE

 The agent may modify code only when there is concrete evidence that an improvement is useful.

 Examples:

 - Measured unnecessary allocation
- Duplicate code
- Memory leak
- Poor error handling
- Unnecessary dependency
- UI responsiveness issue
- Maintainability issue

 Do not make speculative optimizations.

 After improvement:

```
BUILD
  ↓
TEST
  ↓
COMPARE
```

 Verify that the improvement did not introduce regressions.

---

 # 29\. Completion Gate

 Never say:

 > "The browser is complete."

 until all important requirements have been verified.

 Use this checklist:

```
[ ] Environment verified
[ ] Dependencies verified

[ ] GTK application launches
[ ] WebKitGTK initializes
[ ] HTTPS website renders

[ ] Address bar works
[ ] URL navigation works
[ ] Search input works

[ ] Back works
[ ] Forward works
[ ] Reload works

[ ] New tab works
[ ] Multiple tabs work
[ ] Tab switching works
[ ] Tab closing works
[ ] Tab titles update

[ ] URL updates
[ ] Loading state works

[ ] Keyboard shortcuts work

[ ] Page errors do not crash browser

[ ] cargo fmt passes
[ ] cargo check passes
[ ] cargo test passes
[ ] cargo clippy passes
[ ] release build succeeds
[ ] release binary launches

[ ] End-to-end workflow passes
```

 If any important item fails:

 **Continue the engineering loop.**

 Do not declare completion.

---

 # 30\. Final Optimization Pass

 Once all functional requirements pass:

 Perform one final review.

 Ask:

 - Can any dependency be removed?
- Can any module be simplified?
- Are there unnecessary allocations?
- Are there unnecessary clones?
- Are there unnecessary signals?
- Are there unnecessary background operations?
- Are there memory leaks?
- Are there crashes caused by invalid state?
- Is the UI responsive?
- Is the application actually native?
- Is the browser still easy to understand?

 Make only changes that improve the product.

 Then run the complete validation suite again:

```
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

 Also run the release application and perform the functional workflow again.

---

 # 31\. Final Report

 Only after all acceptance criteria pass, produce a final report.

 The final report must contain:

 ## Implementation

 Describe what was built.

 ## Technology Stack

 List:

 - Rust
- GTK4
- WebKitGTK 6.0
- GLib/GIO
- Cargo
- Arch Linux target

 ## Features

 List all implemented MVP features.

 ## Validation

 Report the actual results of:

```
cargo fmt --check
cargo check
cargo test
cargo clippy
cargo build --release
```

 Do not claim a command passed unless you actually ran it.

 ## Manual Testing

 Describe which workflows were actually tested.

 Do not claim GUI behavior was manually verified if the environment did not allow GUI interaction.

 ## Known Limitations

 Clearly identify anything that remains outside the MVP.

 ## Files Changed

 List important project files.

 ## Run Instructions

 Provide the exact commands required to build and run the browser.

---

 # 32\. Important Scope Rules

 Do NOT add these features during the MVP:

 - Bookmarks
- History UI
- Downloads manager
- Extensions
- Developer tools
- Password manager
- Autofill
- Sync
- Accounts
- Profiles
- Private/incognito mode
- Ad blocker
- Custom JavaScript injection
- Themes
- Plugin system
- Browser extension API
- Complex settings UI
- Session restoration
- Cloud sync
- Telemetry

 Only add something outside this list if it is technically required for an existing MVP feature to work correctly.

 If you believe an additional feature is necessary, explain why internally and keep the implementation minimal.

---

 # 33\. Most Important Instruction

 You are NOT being asked to merely write a code sample.

 You are acting as an autonomous software engineer.

 Your job is to deliver a working product.

 Use this loop continuously:

```
PLAN
 ↓
IMPLEMENT
 ↓
BUILD
 ↓
RUN
 ↓
TEST
 ↓
OBSERVE
 ↓
DIAGNOSE
 ↓
FIX
 ↓
REBUILD
 ↓
RETEST
 ↓
IMPROVE
 ↓
REPEAT
```

 Never skip the validation stage.

 Never claim success without evidence.

 Never introduce features outside the MVP unless they are necessary for the browser to function correctly.

 Prefer simple, native, maintainable code.

 The final result should be a:

 > Small, functional, lightweight, native Linux browser.

 It should NOT become a giant browser project.

---

 # 34\. Autonomous Execution Rule

 Do not ask the user for confirmation between normal implementation steps.

 Make reasonable engineering decisions autonomously.

 If something can be safely determined from:

 - The repository
- The compiler
- Installed packages
- Documentation
- Tests
- Runtime behavior
- System inspection

 then determine it yourself.

 Only ask the user when genuinely required information or permission is unavailable.

 Do not stop merely because something fails.

 Enter the debugging loop.

---

 # 35\. Starting Instruction

 START NOW.

 First:

 1. Inspect the repository.
2. Inspect the Linux environment.
3. Check Rust and Cargo.
4. Check GTK4.
5. Check WebKitGTK 6.0.
6. Check available build tools.
7. Determine whether GUI execution is possible.
8. Plan the minimal implementation.
9. Implement the first milestone.
10. Build it.
11. Run it.
12. Test it.
13. Continue the engineering loop.

 Do not provide a long theoretical explanation before doing the work.

 Start by inspecting the actual environment and repository.

 The goal is not to describe the browser.

 The goal is to BUILD THE BROWSER.

```

That's the version I'd actually give to the agent. It has the **loop engineering**, explicit **BUILD → VALIDATION → IMPROVEMENT states**, anti-randomness rules, regression loop, completion gate, and the important rule that **"compiles" ≠ "finished."**
```
