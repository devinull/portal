# Portal Stage 2 Implementation Plan

Status: Ready for implementation

Scope: Stage 2 — TUI and lightweight binary distribution

Primary product reference: `C:\Files\Vaults\Ghost\Dev\Notebook\portal-prd.md`

Stage 1 execution reference: `docs/plans/stage-1-implementation.md`

## 1. Objective

Add a fast, keyboard-driven terminal interface to Portal without changing the
behavior of the existing CLI commands or weakening the `portal-core` / frontend
boundary.

Stage 2 is complete when:

- Running `portal` with no arguments opens the TUI.
- Existing CLI commands continue to work exactly as they do in Stage 1.
- The TUI discovers desktop peers without blocking terminal input.
- The selected device remains selected across refreshes and sort changes by
  stable Tailscale node ID.
- The user can connect or open file transfer from the selected device.
- Devices can be sorted by recent use, name, or online status.
- The TUI uses the PRD's five-token theme model and a restrained default theme.
- The interface remains useful in wide, medium, and small terminals.
- Release archives and lightweight per-user installers are available for
  Windows, macOS, and Linux.
- Formatting, Clippy, tests, release builds, and cross-platform CI pass without
  warnings.

The resulting product still has one executable named `portal`. There is no
separate `portal-tui` command.

## 2. Authoritative Inputs and Verified Starting State

An implementing agent must read the following files completely before making
changes:

1. `C:\Files\Vaults\Ghost\Dev\Notebook\portal-prd.md`
2. `docs/plans/stage-1-implementation.md`
3. This plan
4. `README.md` and `.github/workflows/check.yml`
5. Every current Cargo manifest and Rust source file in the repository

The following state was verified while writing this plan:

- Stage 1 is committed at `daf1fdb`.
- The working tree is clean.
- The `Check` GitHub Actions workflow passes on Windows, macOS, and Linux.
- The workspace contains `portal-core` and `portal-cli`.
- The binary target is already named `portal`.
- `Portal::snapshot_with_warnings` exposes unformatted devices and warnings.
- Devices contain stable node IDs and last-connected Unix timestamps.
- `Portal::launch_with_warnings` handles both connect and transfer actions.
- Core launch code starts external commands directly with `std::process::Command`
  and never uses a shell.
- Mobile Tailscale peers are already excluded by the core discovery parser.
- Ratatui 0.30 provides the Crossterm backend, terminal lifecycle helpers,
  `TableState`, and `TestBackend` required by this plan.

Primary terminal-library references:

- <https://ratatui.rs/installation/>
- <https://docs.rs/ratatui/latest/ratatui/init/index.html>
- <https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html>
- <https://docs.rs/crossterm/latest/crossterm/event/index.html>

Do not replace working Stage 1 code merely to make it resemble examples from a
TUI tutorial.

## 3. User-Facing Contract

### 3.1 Invocation

The command contract becomes:

```text
portal                         # open the TUI
portal <name>                  # shorthand CLI connect
portal connect <name>          # explicit CLI connect
portal share <name>            # CLI file transfer
portal transfer <name>         # alias for share
portal init <name>             # CLI local provisioning
portal provision <name>        # alias for init
portal list                    # human-readable CLI list
portal list --json             # scriptable CLI list
```

Additional rules:

- `portal --config <path>` opens the TUI using that config file.
- `portal --dry-run` without an action or target returns a concise error before
  terminal initialization. The TUI has no dry-run mode.
- `portal --help` and `portal --version` remain normal Clap output and must not
  initialize the terminal.
- Provisioning remains a CLI-only operation in Stage 2.

### 3.2 TUI keys

Use exactly this initial key map:

| Key | Behavior |
|---|---|
| `Up` or `k` | Select the previous device |
| `Down` or `j` | Select the next device |
| `Enter` | Connect to the selected device |
| `s` | Open file transfer for the selected device |
| `r` | Request an immediate discovery refresh |
| `Tab` | Cycle recent → name → status → recent sorting |
| `q` or `Esc` | Exit Portal |

Selection stops at the first and last row; it does not wrap. Ignore unrelated
keys. Do not add mouse support in this stage.

On Windows, Crossterm may emit press, repeat, and release events. Handle press
and repeat events, and ignore release events. Navigation key repeat should feel
normal when a key is held down.

### 3.3 Action behavior

- `Enter` calls `Portal::launch_with_warnings(Action::Connect, device)`.
- `s` calls `Portal::launch_with_warnings(Action::Transfer, device)`.
- An offline device remains actionable, matching Stage 1 behavior.
- Display returned warnings in the status line.
- A successful launch triggers a refresh so the persisted last-connected value
  and recent sort order appear in the TUI.
- A failed launch leaves the TUI open and displays the error.
- Pressing an action key with no selected device performs no launch and displays
  `No device selected`.

Never put a nickname, hostname, note, tag, or other user-controlled string into
an executable argument. Continue to rely on the existing core action builder,
which passes only the verified mode flag and preferred Tailscale IP.

## 4. Included Scope

### 4.1 TUI frontend

- A `portal-tui` library crate.
- Bare invocation routed from `portal-cli` into `portal-tui`.
- Ratatui with its Crossterm backend.
- Alternate-screen terminal lifecycle with automatic restoration.
- A device table and selected-device detail panel.
- Keyboard navigation and actions from section 3.2.
- Periodic discovery on a standard worker thread.
- Immediate manual refresh.
- Stable selection across snapshot replacement.
- Recent, alphabetical, and status sort modes.
- In-interface discovery, action, and warning messages.
- Responsive rendering and a minimum-size message.

### 4.2 Theme foundation

- The five PRD tokens: background, foreground, accent, muted, and signal.
- A public frontend-neutral theme type in `portal-core`.
- Strict `#RRGGBB` configuration values.
- A built-in default theme.
- Mapping from core RGB values to Ratatui colors inside `portal-tui`.

### 4.3 Distribution

- Native release archives for the initial supported targets.
- SHA-256 checksum files.
- A PowerShell per-user installer for Windows.
- A POSIX shell per-user installer for macOS and Linux.
- A tag-triggered GitHub Release workflow.
- Clear install, upgrade, uninstall, and private-repository instructions.

## 5. Explicitly Deferred

Do not add any of the following during Stage 2:

- Port forwarding.
- RustDesk command-line behavior that has not been independently verified.
- Credential entry, password storage, or `portal-vault`.
- GUI, Tauri, web views, or desktop launchers.
- Fuzzy search, substring search, or a search text box.
- Mouse support.
- Animations, spinners, FPS counters, or a render tick unrelated to state.
- Base16, Base24, or Catppuccin importers.
- User-selectable fonts.
- More than one built-in theme.
- Theme hot reload.
- Editable address-book fields inside the TUI.
- Dedicated provisioning or transfer panels.
- Notifications, clipboard integration, logging, telemetry, crash reporting, or
  update checks.
- Async Rust, Tokio, async channels, or Crossterm `EventStream`.
- A component framework, event bus, plugin system, or generic backend trait.
- A separate TUI executable.
- Self-update functionality.
- MSI, MSIX, PKG, DMG, AppImage, Flatpak, Homebrew, Winget, `.deb`, or `.rpm`
  packaging.
- ARM Windows or ARM Linux release artifacts until those targets can be tested.

Port forwarding is the first action to consider after Stage 2, but it requires
a separate RustDesk CLI verification step and is not part of this plan.

## 6. Design Principles

1. Keep `portal-core` independent of Ratatui and all terminal types.
2. Keep `portal-cli` responsible only for argument parsing and dispatch.
3. Put interactive state and rendering in `portal-tui`.
4. Use one background worker only for the blocking Tailscale snapshot call.
5. Keep terminal input and all drawing on the main thread.
6. Use standard library threads, channels, time, and errors.
7. Redraw only when visible state changes or the terminal resizes.
8. Preserve the last good device list when a later refresh fails.
9. Identify selections by stable device ID, never hostname or list index.
10. Make sorting, selection replacement, time-label formatting, and key mapping
    pure or locally testable.
11. Treat rendering as a projection of application state. Rendering must not
    perform discovery, launch commands, write state, or mutate configuration.
12. Do not invoke Tailscale or RustDesk through a shell.
13. Do not write tailnet status output, device details, or credentials to logs or
    release artifacts.
14. Prefer a short module with plain structs and functions over traits and
    framework-style abstractions.
15. Do not add an abstraction solely for a hypothetical GUI caller.

## 7. Target Repository Structure

The expected Stage 2 structure is:

```text
portal/
├── .github/
│   └── workflows/
│       ├── check.yml
│       └── release.yml
├── docs/
│   └── plans/
│       ├── stage-1-implementation.md
│       └── stage-2-implementation.md
├── portal-core/
│   ├── Cargo.toml
│   └── src/
│       ├── config.rs
│       ├── lib.rs
│       ├── model.rs
│       ├── platform.rs
│       ├── rustdesk.rs
│       ├── state.rs
│       ├── tailscale.rs
│       └── theme.rs
├── portal-cli/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── portal-tui/
│   ├── Cargo.toml
│   └── src/
│       ├── app.rs
│       ├── lib.rs
│       └── ui.rs
├── scripts/
│   ├── install.ps1
│   └── install.sh
├── Cargo.lock
├── Cargo.toml
└── README.md
```

Do not add `event.rs`, `worker.rs`, `theme.rs`, `components/`, `widgets/`, or
`utils.rs` under `portal-tui` unless one of the three planned files becomes
genuinely difficult to navigate. The initial implementation is small enough for
three files.

## 8. Dependency Budget

Add one workspace dependency:

```toml
ratatui = { version = "0.30", default-features = false, features = ["crossterm"] }
```

Use Ratatui's matching Crossterm re-export for terminal events and commands. Do
not add a separate direct `crossterm` dependency unless compilation proves the
required APIs are not re-exported.

`portal-tui` production dependencies:

- `portal-core`, by workspace path
- `ratatui`, from workspace dependencies

`portal-cli` adds:

- `portal-tui`, by workspace path

Do not add:

- `tokio`
- `async-std`
- `futures`
- `crossbeam`
- `flume`
- `anyhow`
- `color-eyre`
- `tracing`
- `log`
- `better-panic`
- `human-panic`
- `chrono`
- `time`
- `unicode-width`
- a snapshot-testing library
- a TUI template or component framework

Ratatui 0.30's `run` helper owns alternate-screen setup, raw mode, restoration,
and panic-hook restoration. Do not copy older terminal boilerplate from pre-0.30
examples.

## 9. Workspace and Crate Boundaries

### 9.1 Root workspace

Add `portal-tui` to `workspace.members`. Keep a single shared package version.
The release phase changes the workspace version from `0.1.0` to `0.2.0` only
after the Stage 2 behavior is complete.

### 9.2 `portal-core`

`portal-core` continues to own:

- Discovery.
- Configuration parsing.
- Address-book merging.
- Usage-state loading and persistence.
- Exact device resolution.
- RustDesk launch construction and execution.
- The frontend-neutral theme value model.

It must not depend on Ratatui or know about keys, tables, terminal sizes, styles,
or event loops.

### 9.3 `portal-tui`

Create a library crate, not a binary crate. Its public API is intentionally
small:

```rust
pub fn run(portal: portal_core::Portal) -> std::io::Result<()>;
```

The function creates an `Arc<Portal>` internally because the immutable Portal
facade is used by both the UI thread and the discovery worker. Do not expose the
worker or application state publicly.

### 9.4 `portal-cli`

Keep the existing binary target and all current subcommands. In the `(None,
None)` dispatch branch:

1. Reject a bare `--dry-run` with a clear invalid-input error.
2. Load `Portal` using the global config override.
3. Call `portal_tui::run(portal)`.

Load config before Ratatui initializes the terminal. A malformed config or
theme must print as an ordinary CLI error and must not briefly enter raw mode.

## 10. Frontend-Neutral Theme Model

### 10.1 Public types

Create `portal-core/src/theme.rs` with public types equivalent to:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThemeColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub accent: ThemeColor,
    pub muted: ThemeColor,
    pub signal: ThemeColor,
}
```

Exact field names may vary only if clarity improves. Preserve the five-token
contract and RGB representation.

Implement:

- `Default for Theme`.
- `FromStr for ThemeColor`, accepting exactly `#RRGGBB`.
- Serde deserialization for `ThemeColor` through `FromStr`.
- A readable parse error that includes the invalid value but no unrelated
  configuration data.
- Optionally `ThemeColor::to_hex()` if it simplifies tests or future display.

Re-export `Theme`, `ThemeColor`, and the color parse error from `portal-core`.
Add this method to `Portal`:

```rust
#[must_use]
pub fn theme(&self) -> Theme;
```

Return the small `Copy` value rather than exposing private `Config`.

### 10.2 Default palette

Use this exact default palette:

| Token | Value | Purpose |
|---|---|---|
| `background` | `#0B0F14` | Near-black blue background |
| `foreground` | `#D6DEE8` | Soft high-contrast text |
| `accent` | `#7AA2F7` | Selection and key labels |
| `muted` | `#657180` | Borders, headings, offline state |
| `signal` | `#7FD88F` | Online and successful state |

This palette is deliberately restrained: one cool accent, one success color,
no warning rainbow, and no gradients.

### 10.3 Config schema

Extend the existing config with an optional/defaulted theme table:

```toml
[theme]
background = "#0B0F14"
foreground = "#D6DEE8"
accent = "#7AA2F7"
muted = "#657180"
signal = "#7FD88F"
```

Requirements:

- A missing `[theme]` table uses `Theme::default()`.
- Any subset of the five keys overrides only those defaults.
- Unknown theme keys are rejected.
- Values without `#`, shorthand hex, non-hex characters, alpha channels, and
  extra characters are rejected.
- Theme parsing happens with the rest of config loading.
- Theme values never enter list JSON or usage state.

Use per-field Serde default functions or an equivalent small implementation so
partial theme tables inherit the correct token-specific defaults. Do not use
one generic color default for all five fields.

### 10.4 Core theme tests

Add focused tests for:

- Default theme values.
- Full custom theme.
- One-field partial override.
- Uppercase and lowercase hex digits.
- Missing `#` rejected.
- Three-digit hex rejected.
- Eight-digit hex rejected.
- Non-hex input rejected.
- Unknown theme key rejected.
- Existing configs without `[theme]` remain valid.

## 11. Application State and Pure Behavior

Place application state in `portal-tui/src/app.rs`.

### 11.1 Types

Use types equivalent to:

```rust
pub(crate) struct App {
    pub(crate) devices: Vec<Device>,
    pub(crate) table_state: TableState,
    pub(crate) sort_mode: SortMode,
    pub(crate) initial_load_complete: bool,
    pub(crate) refresh_in_flight: bool,
    pub(crate) refresh_pending: bool,
    pub(crate) message: Option<StatusMessage>,
    pub(crate) should_quit: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SortMode {
    Recent,
    Name,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MessageLevel {
    Info,
    Warning,
    Error,
}

pub(crate) struct StatusMessage {
    pub(crate) level: MessageLevel,
    pub(crate) text: String,
    pub(crate) source: MessageSource,
}
```

`MessageSource` needs only `Refresh` and `Action`. It allows a successful
refresh to clear an old refresh error without immediately erasing a useful
action result.

Do not create a Redux-style action enum or generic reducer. Plain methods on
`App` are sufficient.

### 11.2 Selection rules

- An empty device list has no selection.
- The first successful nonempty snapshot selects index zero.
- Before replacing devices, capture the selected device's stable `id` and old
  index.
- Sort the new snapshot using the active mode.
- If the selected ID still exists, select its new index.
- If it disappeared, select `min(old_index, new_len - 1)`.
- If the old list had no selection and the new list is nonempty, select zero.
- Reset the table scroll offset when replacing or re-sorting, then select the
  resolved index so Ratatui can bring it into view.
- Changing sort mode preserves the selected stable ID.

Never preserve selection by hostname, nickname, IP, or row number alone.

### 11.3 Sort rules

All sort modes require deterministic tie breakers.

**Recent**:

1. Devices with `last_connected_at` before devices with no timestamp.
2. Newer timestamps before older timestamps.
3. Case-insensitive display name.
4. Case-sensitive hostname.
5. Stable node ID.

**Name**:

1. Case-insensitive display name.
2. Case-sensitive hostname.
3. Stable node ID.

**Status**:

1. Online before offline.
2. Case-insensitive display name.
3. Case-sensitive hostname.
4. Stable node ID.

Favorites and tags do not affect sorting in Stage 2.

### 11.4 Navigation rules

- Previous on an empty list leaves selection empty.
- Next on an empty list leaves selection empty.
- Previous at index zero stays at zero.
- Next at the final index stays at the final index.
- Navigation methods must not panic if state is temporarily inconsistent.

### 11.5 Time labels

Render `last_connected_at` without adding a time dependency. Write a pure
function that accepts `timestamp` and `now` as Unix seconds:

- No timestamp: `never`
- Future timestamp or less than 60 seconds: `just now`
- Less than 60 minutes: `{n}m ago`
- Less than 24 hours: `{n}h ago`
- Otherwise: `{n}d ago`

Pass the current Unix time into rendering or the helper. Do not call the clock
inside a sorting comparator or test assertion.

### 11.6 App behavior tests

Test at minimum:

- Each sort order and every tie breaker.
- Recent sorting places never-connected devices last.
- Cycling sort modes in the specified order.
- First snapshot selects the first device.
- Stable-ID selection survives refresh reordering.
- Stable-ID selection survives nickname changes.
- Removed selection clamps to the nearest valid row.
- Empty replacement clears selection.
- Selection survives all sort-mode changes.
- Previous and next clamp without wrapping.
- Refresh error preserves the last good devices.
- A successful refresh clears only an old refresh error.
- Action messages survive an unrelated successful refresh.
- Time-label boundaries.

Use sanitized `Device` builders inside tests. Do not copy live tailnet values.

## 12. Discovery Worker and Event Loop

Place terminal lifecycle, worker setup, and the event loop in
`portal-tui/src/lib.rs`.

### 12.1 Worker topology

Use:

- `Arc<Portal>` shared read-only between main and worker threads.
- One `std::sync::mpsc::sync_channel::<()>(1)` for refresh requests.
- One `std::sync::mpsc::channel` for refresh results.
- One named standard thread, such as `portal-discovery`.

The worker loop is equivalent to:

```text
while a refresh request can be received:
    call portal.snapshot_with_warnings()
    send Snapshot or a formatted error back to the UI thread
exit when all request senders are dropped
```

Do not poll Tailscale on more than one thread. Do not create a thread per
refresh. Do not send `Device` references across threads; send owned snapshots.

### 12.2 Refresh scheduling

Use these constants:

```text
REFRESH_INTERVAL = 5 seconds
EVENT_POLL_MAX = 100 milliseconds
```

Behavior:

1. Start the worker and request the first snapshot before entering the event
   loop.
2. Draw immediately with `Discovering devices...`.
3. While a refresh is running, set `refresh_in_flight`.
4. If another refresh is requested while one is running, set
   `refresh_pending` rather than enqueueing duplicate work.
5. When a result arrives, apply it and schedule the next automatic refresh for
   five seconds after completion.
6. If `refresh_pending` was set, send one immediate follow-up refresh and clear
   the flag.
7. A refresh error keeps the prior devices and displays an error.
8. Automatic retries continue after errors.

Use `try_send`, `try_recv`, or carefully guarded sends so the UI thread never
blocks waiting for channel capacity.

### 12.3 Crossterm event rules

Use Ratatui's Crossterm re-export. Call `event::poll` and `event::read` only from
the main UI thread; Crossterm explicitly requires polling and reading to remain
on the same thread.

Handle:

- Key events.
- Resize events by marking the frame dirty.

Ignore:

- Mouse events.
- Focus events.
- Paste events.
- Unsupported events.

### 12.4 Redraw policy

Maintain a local `needs_redraw` boolean. Draw when:

- The TUI first opens.
- A discovery result arrives.
- A key changes selection, sort mode, status, or quit state.
- An action completes.
- A refresh begins and the header changes to `refreshing`.
- A resize event arrives.

Do not redraw continuously every 100 milliseconds. There is no animation in
Stage 2.

### 12.5 Shutdown and restoration

Use `ratatui::run` so Ratatui owns raw mode, alternate-screen setup, panic
restoration, and normal restoration.

On quit:

1. Exit the event loop.
2. Drop the refresh sender.
3. Join the worker thread after Ratatui has restored the terminal if practical.
4. Ignore only a worker panic during process shutdown; do not let it mask a
   terminal I/O error.

Do not implement custom panic hooks unless the pinned Ratatui API no longer
provides the verified restoration behavior.

## 13. Rendering and Visual Design

Place all drawing code in `portal-tui/src/ui.rs`.

### 13.1 Visual direction

The interface must feel like a focused terminal tool, not a dashboard template.

Use:

- A near-black background.
- Soft white primary text.
- One blue accent for the selected row and key labels.
- Muted slate borders and offline state.
- One green signal color for online state and success.
- Rounded single-line borders around the device list and detail panel.
- One line of breathing room between header, body, and footer where space
  permits.
- Bold text only for the product name, selected row, and selected device name.

Do not use:

- Gradients.
- Multiple border styles.
- Drop-shadow approximations.
- Large ASCII art or a logo banner.
- Emoji.
- Icons other than the status dot required by the PRD.
- Blinking, pulsing, spinning, or animated content.
- Dense boxes around every value.
- Bright warning colors outside the five-token theme.

### 13.2 Frame structure

The normal frame has:

```text
PORTAL                                      3 online · 5 devices

╭ devices ─────────────────────╮  ╭ details ────────────────────╮
│ ●  studio        windows ... │  │ studio                      │
│ ●  laptop        macos   ... │  │ online                      │
│ ●  server        linux   ... │  │ hostname  studio-pc         │
╰──────────────────────────────╯  ╰─────────────────────────────╯

↑/k ↓/j move  enter connect  s share  tab sort: recent  r refresh  q quit
```

This sketch defines hierarchy, not exact character widths.

### 13.3 Header

- Left: `PORTAL` in bold foreground or accent.
- Right: `{online} online · {total} devices`.
- While a refresh is running, append ` · refreshing` in muted text.
- Do not display the tailnet name, user identity, raw Tailscale output, or
  executable paths.

### 13.4 Device table

Use Ratatui's built-in `Table` and persistent `TableState` rather than a custom
stateful widget.

Rows use one terminal line each. The status column contains only `●`:

- Online dot: signal color.
- Offline dot: muted color.

The selected row uses:

- Accent background.
- Theme background as text foreground.
- Bold text.

The unselected table uses background and foreground normally. Column headings
are uppercase and muted.

Responsive columns:

- Wide list area: status, name, OS, address.
- Medium list area: status, name, OS.
- Narrow list area: status and name.

Do not manually slice UTF-8 strings. Let Ratatui constrain and truncate cells.
The detail panel always shows the full preferred IP when it is visible.

### 13.5 Details panel

For the selected device, show only useful address-book and connection data:

- Display name as the panel's first bold line.
- `online` or `offline` using signal or muted color.
- Hostname.
- DNS name when present.
- Preferred IP.
- OS.
- Tags when nonempty, joined with ` · `.
- `favorite: yes` only when true.
- Notes when present, wrapped naturally.
- Last connected relative label from section 11.5.

Do not show the stable node ID in the normal interface. It is an internal
selection key, not useful daily UI.

### 13.6 Empty, loading, and failure states

Inside the device panel:

- Before the first result: `Discovering devices...`
- Successful empty result: `No reachable desktop peers`
- A failed initial result: `Discovery failed — press r to retry`

Detailed error text belongs in the footer status line, not centered in the
device panel.

### 13.7 Responsive layout

Use these exact layout thresholds:

- Terminal smaller than `48 × 12`: render only
  `Portal needs at least 48 × 12. Resize the terminal or press q to quit.`
- Width at least 100 and height at least 18: list and details side by side,
  approximately 60% / 40%.
- Width below 100 and height at least 22: list above details.
- Height below 22 but at least 12: list only; hide details.

The header and footer remain visible whenever the terminal meets minimum size.
Do not panic or underflow layout constraints at any terminal size.

### 13.8 Footer

The footer has one line. Its priority is:

1. Error or warning message.
2. Informational action message.
3. Key hints.

When displaying a message, retain a compact `q quit` hint at the right if space
allows. Style:

- Error: foreground text with an `error:` prefix; do not invent a red token.
- Warning: muted or foreground text with a `warning:` prefix.
- Success/info: signal or foreground text.
- Key labels: accent.
- Descriptions: muted.

Long messages are truncated by the available area. No scrolling status log is
needed.

### 13.9 Render tests

Use Ratatui's `TestBackend`; do not add a snapshot testing crate. Assert useful
text and selected cell styles without asserting every blank cell.

Test:

- `120 × 30` wide layout with table and detail panel.
- `80 × 24` stacked layout.
- `60 × 16` list-only layout.
- Below-minimum layout.
- Loading state.
- Empty state.
- Initial discovery failure state.
- Online and offline dots receive different theme colors.
- Selected row uses accent background.
- Nickname is preferred over hostname in the table.
- Full hostname, address, tags, and notes appear in details.
- Footer key hints include the active sort mode.
- A status error replaces normal key hints.

## 14. Input and Action Dispatch

Keep key-to-command mapping separate from command execution so it can be tested
without a terminal.

A small private enum is sufficient:

```rust
enum InputCommand {
    Previous,
    Next,
    Connect,
    Transfer,
    Refresh,
    CycleSort,
    Quit,
}
```

Map a Crossterm key event to `Option<InputCommand>` with a pure function. Do not
build a generic keybinding registry.

Action execution rules:

1. Clone the selected `Device` before calling into `Portal` so mutable UI state
   and immutable device borrowing do not conflict.
2. Call the existing core launch method on the main thread. This work is limited
   to executable resolution, process spawn, and a small atomic state update.
3. Convert errors to display strings and keep the TUI running.
4. On success, set an action message.
5. Show every returned warning; when multiple warnings exist, join them with
   `; ` rather than creating a message history system.
6. Request a refresh after success.

Do not reimplement offline warnings or state persistence in `portal-tui`.

## 15. Implementation Phases

Work through these phases in order. Keep only one phase in progress at a time.
After each meaningful change, run the focused tests named by the phase. At each
phase boundary, run the full validation suite from section 19.

### Phase 0 — Baseline and prerequisites

1. Read the authoritative inputs from section 2.
2. Confirm the working tree state and preserve unrelated user changes.
3. Run the full Stage 1 validation suite.
4. Confirm the current `Check` workflow is green.
5. Confirm Ratatui 0.30 remains available and its `run`, Crossterm re-export,
   `TableState`, and `TestBackend` APIs match this plan.

Exit criterion: Stage 1 remains green and no library conflict requires a design
change.

Decision gate: ask the user only if the current supported Ratatui release has a
material API or platform conflict that would require a different runtime or
additional production dependency. Patch-level API adjustments are not a gate.

### Phase 1 — Workspace and frontend scaffold

1. Add `portal-tui` to workspace members.
2. Add the Ratatui workspace dependency from section 8.
3. Create `portal-tui/Cargo.toml` as a library crate.
4. Add minimal `lib.rs`, `app.rs`, and `ui.rs` modules that compile.
5. Add the `portal-tui` path dependency to `portal-cli` without changing bare
   invocation yet.
6. Update `Cargo.lock` through Cargo.
7. Inspect `cargo tree` and confirm Ratatui/Crossterm account for every new
   transitive dependency.

Focused validation:

```text
cargo check --workspace --all-targets
cargo tree
```

Exit criterion: the new crate compiles but has no behavior or extra framework.

### Phase 2 — Core theme model

1. Add `portal-core/src/theme.rs`.
2. Implement strict theme color parsing and defaults.
3. Extend private config with the defaulted theme table.
4. Expose `Portal::theme()` and public theme value types.
5. Add every theme/config test from section 10.4.
6. Confirm all existing Stage 1 config tests still pass unchanged.

Focused validation:

```text
cargo test -p portal-core theme
cargo test -p portal-core config
```

Exit criterion: themes are parsed and exposed without any terminal dependency
in core.

### Phase 3 — Pure TUI application state

1. Implement `App`, `SortMode`, and status-message types.
2. Implement deterministic pure sorting.
3. Implement stable-ID snapshot replacement.
4. Implement clamped navigation.
5. Implement sort cycling.
6. Implement the relative time-label helper.
7. Add all tests from sections 11.3 through 11.6.

Focused validation:

```text
cargo test -p portal-tui app
```

Exit criterion: all state transitions work without a live terminal, Tailscale,
or RustDesk.

### Phase 4 — Static responsive renderer

1. Map the core theme to Ratatui `Color::Rgb` in `ui.rs`.
2. Implement the minimum-size screen.
3. Implement header, device table, detail panel, and footer.
4. Implement wide, stacked, and list-only layouts.
5. Add loading, empty, and discovery-failure content.
6. Add render tests from section 13.9 using `TestBackend`.
7. Inspect rendered buffers at all planned sizes for clipped borders or crowded
   text.

Focused validation:

```text
cargo test -p portal-tui ui
```

Exit criterion: a static sanitized model renders cleanly at all supported sizes.

### Phase 5 — Worker and live event loop

1. Implement the one-thread snapshot worker.
2. Implement initial, timed, pending, and manual refresh requests.
3. Implement `ratatui::run` terminal lifecycle.
4. Implement main-thread Crossterm polling and resize handling.
5. Add dirty redraw behavior.
6. Preserve the last good devices on errors.
7. Ensure worker shutdown cannot leave the terminal in raw mode.
8. Do not add action keys yet beyond navigation, refresh, sort, and quit.

Focused validation:

```text
cargo test -p portal-tui
cargo run -p portal-cli -- list
```

Manual check: run the TUI against live Tailscale, resize the terminal, wait for
multiple refreshes, press `r`, change sorting, and quit. Confirm the terminal is
restored after normal exit and after a deliberately returned non-panic error.

Exit criterion: discovery is live, nonblocking, stable, and recoverable.

### Phase 6 — Actions and CLI integration

1. Implement the exact input map from section 3.2.
2. Add connect and transfer dispatch.
3. Display action errors and core warnings without leaving the TUI.
4. Request a follow-up refresh after a successful launch.
5. Replace the bare CLI help branch with TUI launch.
6. Reject bare `--dry-run` before terminal initialization.
7. Update CLI tests for bare dispatch and bare dry-run behavior.
8. Confirm every existing CLI command and alias still uses its Stage 1 path.

Focused validation:

```text
cargo test -p portal-cli
cargo test -p portal-tui
cargo run -p portal-cli -- --help
cargo run -p portal-cli -- list
```

Manual check with `portal --config <sanitized-test-config>`:

- `Enter` opens the selected device through the verified connect syntax.
- `s` opens file transfer through the verified transfer syntax.
- Offline warnings appear but do not block launch.
- Failed executable lookup appears in the footer and the TUI remains usable.
- Last-connected sorting updates after a successful launch.

Exit criterion: bare Portal is the TUI and all Stage 1 CLI behavior is preserved.

### Phase 7 — Visual polish and accessibility pass

1. Apply the exact default palette.
2. Remove any unnecessary borders, labels, or repeated information.
3. Confirm selected text has readable contrast.
4. Confirm online/offline state is readable from text in the detail panel even
   though the list uses color-only dots.
5. Test long nicknames, IPv6 addresses, long tags, and wrapped notes.
6. Test empty and error states.
7. Test every responsive threshold and one size just below each threshold.
8. Confirm no render loop consumes significant CPU while idle.
9. Confirm the TUI contains no private tailnet data in tests or screenshots.

Exit criterion: the TUI is clean, modern, minimal, and robust rather than merely
functional.

### Phase 8 — Per-user installers

Create `scripts/install.ps1` and `scripts/install.sh`.

**Windows installer requirements**:

- Default source: sibling `portal.exe` in the extracted release directory.
- Default destination:
  `%LOCALAPPDATA%\Programs\Portal\bin\portal.exe`.
- Refuse to continue if `LOCALAPPDATA` or the source binary is missing.
- Verify the sibling binary checksum before copying.
- Copy to a temporary file in the destination directory, then replace the exact
  destination.
- Add the destination directory to the user-level `PATH` only if absent,
  comparing entries case-insensitively.
- Never change the machine-level `PATH`.
- Never require administrator elevation.
- Print that existing terminal processes must be reopened.
- Run the installed binary with `--version` after copying.
- Support explicit source and destination parameters for CI testing.
- Support `-NoPathUpdate` for isolated smoke tests.
- If uninstall is supported, remove only the exact installed executable and its
  exact user-PATH entry. Preserve config and state and say so.

**macOS/Linux installer requirements**:

- Default source: sibling `portal` in the extracted release directory.
- Default destination: `${PORTAL_INSTALL_DIR:-$HOME/.local/bin}/portal`.
- Refuse to continue if `HOME` or the source binary is missing.
- Verify the sibling binary checksum before copying.
- Create only the exact destination directory.
- Install the binary with executable permissions.
- Use a temporary file followed by rename in the destination directory.
- Run the installed binary with `--version`.
- If the directory is not on `PATH`, print the exact export line and likely
  shell profile location. Do not silently edit shell startup files.
- Do not call `sudo` or install into `/usr/local/bin` automatically.
- Support a destination override for CI smoke tests.
- If uninstall is supported, remove only the exact binary. Preserve config,
  state, and the containing `~/.local/bin` directory.

Script safety requirements:

- Quote every path.
- Do not evaluate downloaded text.
- Do not accept or print GitHub tokens.
- Do not recursively delete any directory.
- Do not modify shell profiles without an explicit future option.
- A failed verification or copy leaves the old installed binary intact.

Checksum file contract:

- Windows packages contain `portal.exe.sha256`.
- macOS/Linux packages contain `portal.sha256`.
- The file contains one line in the conventional
  `<64 lowercase hex characters><two spaces><binary filename>` form.
- The installer compares the expected digest with the selected source binary
  before creating or replacing anything in the destination.
- Windows comparison is case-insensitive; Unix comparison may normalize the
  computed digest to lowercase.
- A missing, malformed, or mismatched checksum is a hard failure.

Focused validation:

- Run Windows installation into a temporary directory with `-NoPathUpdate`.
- Verify `portal --version` from the temporary installed path.
- Re-run to verify idempotent upgrade behavior.
- Test a checksum mismatch and confirm the destination is unchanged.
- Run macOS and Linux installer smoke tests in their CI jobs with a temporary
  home or destination.
- Static-review both uninstall paths before executing them.

Decision gate: ask before changing the developer machine's real user `PATH` as
part of manual validation. Temporary-directory installer tests do not require a
gate.

### Phase 9 — Release workflow

Add `.github/workflows/release.yml`, triggered only by tags matching `v*`.

Initial artifacts:

```text
portal-windows-x86_64.zip
portal-linux-x86_64.tar.gz
portal-macos-x86_64.tar.gz
portal-macos-aarch64.tar.gz
```

Each archive contains only:

- The `portal` binary for that target.
- The matching installer script.
- A checksum file for the unpacked binary.
- A short release README or the repository README if useful.

For every archive, publish a separate SHA-256 checksum file for the archive.

Workflow requirements:

1. Use GitHub-hosted native Windows, Ubuntu, and macOS runners.
2. Install stable Rust with Rustfmt and Clippy where validation runs.
3. Build Windows `x86_64-pc-windows-msvc`.
4. Build Linux `x86_64-unknown-linux-gnu`.
5. Build both `x86_64-apple-darwin` and `aarch64-apple-darwin` on macOS using
   Rust target support available on the runner.
6. Run tests before packaging.
7. Package only release binaries, never `target/`, config, state, or local data.
8. Generate SHA-256 checksums on the runner.
9. Upload archives and checksums to one GitHub Release matching the tag.
10. Use the repository's built-in `GITHUB_TOKEN` with `contents: write`; do not
    create or commit a personal token.
11. Fail if the tag version and workspace package version differ.
12. Do not publish crates to crates.io.

Use ordinary GitHub-hosted tooling rather than adding cargo-dist in Stage 2:

1. Each platform build job packages its files and uploads them with
   `actions/upload-artifact`.
2. One publish job depends on every build job and retrieves them with
   `actions/download-artifact`.
3. The publish job verifies every archive checksum again.
4. It creates one release using the preinstalled GitHub CLI and the repository
   `GITHUB_TOKEN`, passing the tag plus all archives and checksum files.
5. If a release already exists for the tag, fail instead of silently replacing
   assets. Never delete or overwrite a release automatically during a rerun.

Because the repository is private, document authenticated download with:

```text
gh release download <tag> --repo devinull/portal
```

Do not add unauthenticated curl-based bootstrap installers while release assets
remain private.

Unsigned artifact limitation:

- Stage 2 has no signing certificates.
- Document Windows reputation warnings and macOS Gatekeeper/quarantine behavior
  truthfully.
- Do not claim that unsigned archives are production-signed or notarized.
- Signing and public package-manager distribution are later release-hardening
  work.

Exit criterion: a test tag or equivalent non-public workflow run builds all
artifacts, verifies checksums, and installs each artifact in CI without exposing
private data.

### Phase 10 — Documentation and final acceptance

1. Change the workspace version to `0.2.0` if it was not already changed for a
   release test.
2. Update README summary from CLI-only to CLI/TUI.
3. Document bare invocation and the complete key map.
4. Document theme defaults and custom theme TOML.
5. Document refresh cadence and stable selection behavior.
6. Document platform installation, upgrade, uninstall, and private release
   download steps.
7. Document unsigned binary limitations.
8. Preserve Stage 1 command, configuration, security, and provisioning docs.
9. Mark macOS/Linux runtime validation honestly.
10. Run the full automated and manual validation in sections 19 and 20.
11. Inspect `cargo tree` and remove unused direct dependencies.
12. Confirm no sanitized fixture was replaced with live tailnet output.

Exit criterion: every applicable acceptance item passes and all platform-only
checks are listed rather than implied complete.

## 16. Installer and Release Version Semantics

- The workspace, CLI, TUI, and core share one semantic version.
- Stage 2's first release is `0.2.0` / tag `v0.2.0` unless an earlier tag already
  used that version.
- Re-running an installer replaces the binary in place and is the upgrade path.
- No automatic background update occurs.
- Config and state remain compatible with Stage 1.
- Uninstalling the executable does not delete config or state.
- Downgrades are not explicitly blocked, but documentation should warn that
  future config fields may not be understood by older releases.

## 17. Error and Security Requirements

### 17.1 Terminal errors

- Config errors occur before entering the TUI.
- Terminal initialization failures return to ordinary CLI stderr.
- Discovery and launch errors appear inside the running TUI.
- A failed refresh preserves the last good snapshot.
- A worker channel failure appears as a refresh error and does not panic.
- Exiting after any recoverable error restores the terminal.

### 17.2 Sensitive data

Never display or persist:

- Tailscale authentication data.
- Tailnet user IDs, public keys, or raw status JSON.
- RustDesk passwords.
- GitHub tokens.
- Environment dumps.
- Config or state contents in error messages.

The UI may display only the device information already represented by `Device`
and selected address-book metadata.

### 17.3 External commands

Portal runtime code must continue to use direct `Command` construction. No
runtime feature may invoke `cmd.exe`, PowerShell, `/bin/sh`, `bash`, or another
shell to launch Tailscale or RustDesk.

Installer scripts are themselves platform shells, but they are release tooling,
not Portal's external-action execution path. They must still quote paths and
avoid evaluated or dynamically constructed commands.

## 18. Automated Test Matrix

### 18.1 Core

- All existing Stage 1 tests.
- Theme parsing and defaults from section 10.4.
- Existing configuration remains backward compatible.

### 18.2 TUI state

- Sorting and tie breakers.
- Sort cycling.
- Navigation bounds.
- Stable-ID selection retention.
- Snapshot replacement and error retention.
- Message source behavior.
- Relative time labels.
- Key mapping, including press/repeat accepted and release ignored.

### 18.3 TUI rendering

- All responsive modes.
- Minimum-size screen.
- Loading, empty, populated, and error states.
- Selected and status color styles.
- Long sanitized content.
- Footer modes.

### 18.4 CLI

- Bare invocation routes to TUI at dispatch level.
- Bare dry-run is rejected before TUI initialization.
- Help and version remain noninteractive.
- Every existing Stage 1 parsing and rendering test remains green.

Do not create a unit test that starts a real alternate-screen terminal. Use
`TestBackend` for rendering and keep the small terminal loop covered by manual
smoke tests.

### 18.5 Installer and release

- Windows temporary-destination install and upgrade.
- macOS temporary-destination install and upgrade.
- Linux temporary-destination install and upgrade.
- Checksum mismatch rejection.
- Installed binary reports the tagged version.
- Archive contains no unexpected files.

## 19. Full Validation Suite

Run at every phase boundary from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --release
cargo tree
```

Also verify the release binary's noninteractive surfaces:

```text
target/release/portal --version
target/release/portal --help
target/release/portal list
target/release/portal list --json
```

On Windows, use `target\release\portal.exe` as appropriate.

Every direct dependency must be named in section 8 or inherited from Stage 1.
Remove unused direct dependencies before moving to the next phase.

## 20. Manual Validation

### 20.1 Windows TUI

With Tailscale authenticated and RustDesk installed:

1. Run bare `portal`.
2. Confirm the local machine and mobile peers are absent.
3. Confirm expected desktop peers appear once.
4. Confirm initial selection is visible.
5. Hold arrow keys, `j`, and `k`; verify navigation feels responsive.
6. Wait for at least three automatic refreshes.
7. Press `r` repeatedly; confirm discovery is not duplicated or blocked.
8. Change a peer's online state where this occurs naturally and verify the row
   updates without losing selection.
9. Cycle every sort mode and confirm the selected device stays selected.
10. Connect with `Enter` and verify the exact selected device opens.
11. Open transfer with `s` and verify transfer mode opens.
12. Confirm recent sorting updates after launch.
13. Select an offline peer and confirm warning plus attempted launch.
14. Resize through wide, stacked, list-only, and minimum-size states.
15. Quit with `q` and `Esc`; confirm the terminal prompt is normal.
16. Trigger a harmless discovery error with a temporary config override and
    confirm the TUI remains usable and retryable.
17. Trigger a missing RustDesk error with a temporary config and confirm the
    footer error does not close the TUI.

Do not stop the user's real Tailscale service or alter primary config merely to
produce an error.

### 20.2 Theme

Using temporary config files:

1. Run with no theme and inspect the default palette.
2. Override one token and confirm the other four retain defaults.
3. Override all tokens and confirm all visible mappings.
4. Use invalid hex and confirm Portal fails before entering alternate-screen
   mode.
5. Use an unknown theme key and confirm strict config rejection.
6. Confirm theme values do not appear in `portal list --json` or state.

### 20.3 Windows installer

1. Install into a temporary directory without PATH mutation.
2. Run the installed binary.
3. Reinstall the same version.
4. Replace it with a newer local build.
5. Test a bad checksum.
6. Inspect the exact files created.
7. With user approval, install to the real per-user destination and update user
   PATH.
8. Open a new terminal and run `portal --version` by command name.
9. Verify config and state remain after an uninstall test, if approved.

### 20.4 macOS and Linux

On each available physical or virtual system:

1. Install from the native archive into a temporary home.
2. Run `portal --version`, CLI list, and bare TUI.
3. Verify terminal restoration.
4. Verify config and state directories.
5. Verify live refresh, connect, and transfer.
6. Verify the installer's PATH guidance for the default shell.
7. Record signing, quarantine, terminal, and RustDesk permission behavior.

If systems are unavailable, CI compile/render/installer tests are required but
must be labeled compile-tested rather than runtime-validated.

### 20.5 Idle efficiency

While the TUI is idle between refreshes:

- Confirm it does not visibly redraw.
- Confirm it does not use a full CPU core.
- Confirm only one Tailscale status process runs per refresh.
- Confirm no background worker remains after Portal exits.

Do not add a benchmarking framework. OS process inspection and ordinary timing
are sufficient.

## 21. CI Requirements

Update the existing `check.yml` only as needed; because it runs workspace-wide
commands, adding `portal-tui` should automatically include it.

The cross-platform check must continue to run:

1. Format check on one platform.
2. Clippy with warnings denied on Windows, macOS, and Linux.
3. All workspace tests on Windows, macOS, and Linux.
4. Release build on Windows, macOS, and Linux.

No CI test may require live Tailscale, RustDesk, private tailnet data, or a real
interactive terminal.

The release workflow is separate from `check.yml` and follows Phase 9.

## 22. Acceptance Checklist

- [ ] Stage 1 baseline is green before changes.
- [ ] Workspace contains `portal-core`, `portal-cli`, and `portal-tui` only.
- [ ] `portal-tui` is a library, and the only executable remains `portal`.
- [ ] Bare `portal` opens the TUI.
- [ ] Existing CLI commands, aliases, JSON, and provisioning still work.
- [ ] Bare `--dry-run` fails before terminal initialization.
- [ ] TUI input remains responsive during discovery.
- [ ] Discovery runs on one standard worker thread.
- [ ] Automatic refresh occurs every five seconds after completion.
- [ ] Repeated refresh requests do not queue unbounded work.
- [ ] Refresh errors preserve the last good device list.
- [ ] Selection is retained by stable node ID.
- [ ] Recent, name, and status sorts match section 11.3.
- [ ] Arrow and `j`/`k` navigation clamps correctly.
- [ ] `Enter` performs the verified connect action.
- [ ] `s` performs the verified file-transfer action.
- [ ] Offline peers warn but remain actionable.
- [ ] Successful actions refresh recent-use state.
- [ ] TUI errors do not corrupt or exit the terminal unexpectedly.
- [ ] Normal exit and panic restoration leave the terminal usable.
- [ ] Five-token themes parse strictly and default correctly.
- [ ] The default interface matches the minimal design specification.
- [ ] Wide, stacked, list-only, and undersized layouts are tested.
- [ ] No mouse, animation, fuzzy search, port forwarding, vault, or GUI was added.
- [ ] No async runtime, logging framework, or unnecessary dependency was added.
- [ ] No shell launches Tailscale or RustDesk.
- [ ] No credentials or private tailnet fixtures were added.
- [ ] Windows per-user installer is idempotent and PATH-safe.
- [ ] macOS/Linux installer uses `~/.local/bin` by default and avoids sudo.
- [ ] Checksums are verified before installation.
- [ ] Release archives contain only intended files.
- [ ] Private GitHub Release download is documented.
- [ ] Unsigned binary limitations are documented honestly.
- [ ] `cargo fmt`, Clippy, tests, and release build pass.
- [ ] Cross-platform CI passes.
- [ ] README reflects Stage 2 behavior and limitations.
- [ ] Remaining platform-only runtime checks are explicitly listed.

## 23. Agent Handoff Rules

An implementing agent must:

1. Read every authoritative file in section 2 before acting.
2. Follow phases in order and update its working plan after each phase.
3. Keep the repository buildable and warning-free at each phase boundary.
4. Use focused tests after meaningful edits and the full suite after every
   phase.
5. Make minor naming and formatting decisions without asking the user.
6. Use the exact interaction, sorting, palette, and responsive rules in this
   plan instead of opening new design discussions.
7. Preserve unrelated user changes in a dirty worktree.
8. Use sanitized fixtures only.
9. Avoid expanding Stage 2 for completeness.
10. Report limitations plainly.

Ask the user only when:

- A current Ratatui/platform conflict materially requires a different runtime
  or new production dependency.
- A manual test would change the real user PATH, require elevation, alter a
  security setting, or delete an installed binary/configuration.
- Verified RustDesk behavior conflicts with the existing Stage 1 action
  contract.
- Release signing is requested but no signing identity or certificate is
  available.

Do not stop after scaffolding, static rendering, or unit tests. Continue until
all applicable Stage 2 phases are complete or an explicit decision gate is
reached.
