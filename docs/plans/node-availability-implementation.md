# Node Availability and Latency Implementation Plan

Status: Implemented. This is retained as a historical execution and validation
record.

## 1. Objective

Replace Portal's use of the Tailscale `Online` JSON field as the displayed peer
status with a lightweight active reachability check for every eligible desktop
node.

The completed feature must:

- display a filled device dot only after an active Tailscale probe succeeds;
- display an empty dot when a device has not been checked or repeated checks
  fail;
- avoid status flicker from one transient failure;
- record and display the round-trip latency returned by the successful probe;
- keep device discovery and reachability checking asynchronous;
- run no more than one reachability process at a time;
- preserve the current five-second discovery refresh;
- never launch a command through a shell; and
- never persist or display raw ping output, private tailnet names, or addresses
  beyond device fields Portal already displays.

This implementation applies to every non-mobile Tailscale peer. Do not add
machine-specific names, exceptions, configuration, or heuristics.

## 2. Current behavior to replace

The current path is:

1. `portal-core/src/tailscale.rs` runs `tailscale status --json`.
2. `Peer.Online` is copied to `Device.online`.
3. `portal-tui/src/ui.rs` uses `Device.online` for the row dot, header count, and
   details status.
4. `portal-tui/src/app.rs` uses `Device.online` for status sorting.
5. `portal-core/src/rustdesk.rs` creates an offline warning from the same field.

The field is a coordination-state hint, not a successful path test. It can be
false while a node still answers across Tailscale. It must no longer be used as
Portal's availability result.

Keep `tailscale status --json` for discovery and metadata. Do not replace it with
per-node pings.

## 3. Probe contract

For each eligible node, run this exact argument vector:

```text
program: <resolved tailscale executable>
arg[0]: ping
arg[1]: --tsmp
arg[2]: --c
arg[3]: 1
arg[4]: --timeout
arg[5]: 2s
arg[6]: <preferred Tailscale IP>
```

Use `std::process::Command` directly. The target IP must be a separate argument.

TSMP is appropriate for the background monitor because it checks the encrypted
Tailscale path without depending on ordinary ICMP or the remote operating-system
firewall. A successful process exit means the Tailscale node answered. A nonzero
exit, timeout, or spawn failure means that probe attempt failed.

Successful process exit is sufficient for reachability. Invalid or changed
human-readable output may remove the latency measurement, but must not turn a
successful response into a failed reachability check.

The command also returns latency in its success line, for example:

```text
pong from sanitized-node (100.100.0.20, 12345) via TSMP in 20ms
```

The fixture is sanitized. Never use live tailnet output in tests.

Do not run the normal multi-ping DISCO sequence in the background. Direct, DERP,
and peer-relay path discovery belongs to the later on-demand diagnostics feature.

## 4. Required data model

### 4.1 Core probe result

Add a small probe module in `portal-core`, preferably
`portal-core/src/probe.rs`.

Expose:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbeMeasurement {
    pub latency: Option<Duration>,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    // executable resolution, spawn, and nonzero exit variants
}
```

The exact variant names may follow existing `DiscoveryError` naming. Error text
must be concise and sanitized. Do not include stdout or stderr because ping
output can contain peer names and addresses. Do not retain command output in
application state.

Add this method to `Portal`:

```rust
pub fn probe(&self, device: &Device) -> Result<ProbeMeasurement, ProbeError>;
```

It must use the same configured Tailscale executable override used by discovery.

Keep command construction and output parsing as pure functions where practical.
Do not add an async runtime, process pool, logging framework, regular-expression
dependency, or networking dependency.

### 4.2 TUI availability state

Availability is live UI state and must not be added to persisted device state.
Add these internal types to `portal-tui/src/app.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AvailabilityState {
    Unknown,
    Reachable,
    Unavailable,
}

#[derive(Clone, Debug)]
pub(crate) struct Availability {
    pub(crate) state: AvailabilityState,
    pub(crate) latency: Option<Duration>,
    consecutive_failures: u8,
    next_probe_at: Instant,
    ip: IpAddr,
}
```

Store availability in `App` as a `HashMap<String, Availability>` keyed by stable
Tailscale node ID. Do not key it by machine name, IP, nickname, or table index.

Also track the currently running probe:

```rust
pub(crate) struct ProbeTarget {
    pub(crate) id: String,
    pub(crate) ip: IpAddr,
}

probe_in_flight: Option<ProbeTarget>
```

`ProbeTarget` must be owned so it can cross the worker channel without borrowing
a `Device`.

### 4.3 Coordination hint

Do not remove Tailscale's `Online` value from parsing in this change. Keeping it
avoids an unrelated JSON and core-model compatibility change. Treat
`Device.online` only as a coordination hint.

After this implementation, no TUI rendering, TUI sorting, or RustDesk launch
warning may use `Device.online` as reachability.

## 5. Availability transitions

Use these constants in `portal-tui`:

```rust
const REACHABLE_PROBE_INTERVAL: Duration = Duration::from_secs(15);
const RETRY_PROBE_INTERVAL: Duration = Duration::from_secs(5);
const FAILURES_BEFORE_UNAVAILABLE: u8 = 2;
```

Apply probe results using one pure state-transition method.

### Successful probe

Immediately:

- set state to `Reachable`;
- store the parsed latency, which may be `None` if only latency parsing failed;
- reset consecutive failures to zero;
- set `next_probe_at` to current instant plus 15 seconds.

### Failed probe

Immediately:

- increment consecutive failures, saturating at `u8::MAX`;
- clear latency so Portal does not show a stale measurement as current;
- set `next_probe_at` to current instant plus 5 seconds.

State handling:

- an `Unknown` node remains `Unknown` after its first failure;
- a `Reachable` node remains `Reachable` after its first consecutive failure;
- any node becomes `Unavailable` after its second consecutive failure;
- one later success always returns it immediately to `Reachable`.

This is the only hysteresis. Do not add moving averages, scoring, exponential
backoff, packet-loss estimates, or configurable thresholds in this phase.

## 6. Latency parsing and formatting

### 6.1 Core parser

Parse only the duration token after the final ` in ` in a successful stdout
line. Support the duration forms Tailscale can emit:

- nanoseconds: `800ns`;
- microseconds: `800us` or `800µs`;
- milliseconds: `20ms` or `20.5ms`;
- seconds: `1s` or `1.2s`.

Implement a small unit-suffix parser with standard-library string and numeric
operations. Do not add a regex or duration-parsing crate.

If the process exits successfully but the duration cannot be parsed, return a
successful `ProbeMeasurement { latency: None }`. Reachability must not depend on
the human-readable latency format.

Add sanitized unit tests for every supported unit, surrounding text, missing
latency, malformed values, and successful output with a trailing newline.

### 6.2 UI formatter

Add a pure formatter in `portal-tui/src/ui.rs` or `app.rs`:

- below 1 ms: `<1 ms`;
- from 1 through 999 ms: rounded whole milliseconds, such as `24 ms`;
- 1 second and above: one decimal place, such as `1.2 s`;
- no current measurement: `—`.

Do not show more precision. The status is operational guidance, not a benchmark.

## 7. Discovery reconciliation

Extend snapshot application in `App` to reconcile the availability map after
each successful discovery refresh.

For each current device:

- preserve its availability when stable node ID and preferred IP are unchanged;
- create `Unknown` availability due immediately when the node ID is new;
- reset to `Unknown` and make it due immediately when its preferred IP changes.

Remove availability entries for devices no longer present.

If a discovery refresh fails, preserve the current devices and their availability
exactly as the TUI does today. The existing header discovery-health dot remains
responsible for reporting whether local discovery is healthy.

Discovery refreshes occur every five seconds, so reconciliation must not reset
the 15-second probe schedule for unchanged devices.

## 8. Probe scheduling

Do not create a queue containing repeated work. When no probe is running, select
the next target by examining current devices and availability:

1. Return no target while `probe_in_flight` is populated.
2. Consider only devices whose `next_probe_at` is at or before the current time.
3. Prefer the selected device if it is due.
4. Otherwise choose the due device with the earliest `next_probe_at`.
5. Break ties by the current deterministic device order.
6. Set `probe_in_flight` before sending the request to the worker.
7. If sending fails, clear `probe_in_flight`; report a disconnected worker only
   once rather than leaving scheduling permanently blocked.

This O(n) scan is preferred over a heap or scheduler abstraction because Portal
has a small peer list and scans only when the one worker is idle.

When the user presses `r`, keep the current discovery refresh behavior and also
mark every device probe due immediately. They must still run sequentially.

When a response arrives:

1. Clear `probe_in_flight` only if its stable node ID and IP match the response.
2. Ignore the result if the device was removed or its IP changed.
3. Apply the success or failure transition.
4. If status sorting is active, preserve the selected stable node ID, re-sort,
   and restore selection.
5. Request a redraw.
6. Allow the scheduler to start the next due probe.

Do not show a footer message for ordinary per-node timeouts. Availability dots
already communicate that state. A disconnected probe worker may produce one
concise warning and must not repeatedly overwrite the footer.

## 9. Probe worker

Keep the existing discovery worker unchanged except for necessary type imports.
Add one separate probe worker in `portal-tui/src/lib.rs`.

Use:

- one request channel with capacity one;
- one response channel;
- one worker thread named `portal-probe`;
- the existing shared `Arc<Portal>`; and
- owned request and response values.

Suggested response shape:

```rust
struct ProbeResponse {
    target: ProbeTarget,
    result: Result<ProbeMeasurement, String>,
}
```

The probe thread loops over requests, calls `Portal::probe`, sends the owned
response, and exits when the channel closes. It must never mutate `App`.

The event loop already wakes at most every 100 ms. On each iteration:

1. drain discovery responses;
2. drain probe responses;
3. request the periodic discovery refresh if due;
4. start one due probe if the probe worker is idle;
5. redraw only when state changed; and
6. poll input as it does now.

On shutdown, drop both request senders and join both workers. Do not detach a
worker and do not wait for a probe while the TUI is running.

## 10. TUI rendering requirements

### 10.1 Header

Change the summary from:

```text
N online · M devices
```

to:

```text
N reachable · M devices
```

Count only `AvailabilityState::Reachable`. Keep the existing top-right discovery
health dot unchanged; it represents successful local discovery, not peer
reachability.

### 10.2 Device dots

Render:

- `●` in the theme signal color for `Reachable`;
- `○` in the theme muted color for `Unknown` and `Unavailable`.

Keep selected-row foreground and background behavior. Do not introduce another
icon, spinner, animation, or text in the row status cell.

### 10.3 Details panel

When the details panel is visible, show latency on the first content line, right
aligned on the same line as the selected device display name:

```text
studio-pc                                      24 ms
reachable
```

Use the existing bold foreground style for the name and the muted style for
latency. Omit the latency text when the panel is too narrow for both values; do
not overlap or truncate the device name merely to force latency into the line.

Change the second line to one of:

- `reachable` in signal color;
- `checking` in muted color for `Unknown`;
- `unavailable` in muted color for `Unavailable`.

Keep the remainder of the details fields and current panel padding unchanged.

Render the panel block first, calculate its padded inner area, and use a
horizontal layout for the first line if that is simpler than injecting spaces
into a `Line`. Do not add a Unicode-width dependency solely for alignment.

### 10.4 List-only layouts

Pass an explicit `show_latency` flag to the table renderer. Set it to `true` only
when `render_body` chooses the list-only branch because no details panel fits.

Use these list-only columns:

- width 72 or greater: dot, name, OS, address, latency;
- width 54 through 71: dot, name, OS, latency;
- width 48 through 53: dot, name, latency.

Use `LATENCY` as the heading and a fixed width of 9 columns. Use the formatter in
section 6.2. The name remains the flexible column. Details-visible layouts must
not duplicate latency in the device table.

Do not lower Portal's existing minimum terminal size.

### 10.5 Status sorting

Replace coordination-hint sorting with availability sorting:

1. `Reachable` devices;
2. `Unknown` devices;
3. `Unavailable` devices;
4. existing deterministic name ordering within each group.

An asynchronous probe result may reorder the table. Always preserve selection by
stable node ID.

## 11. RustDesk and CLI cleanup

### 11.1 RustDesk warning

Remove the offline warning based on `Device.online` from
`portal-core/src/rustdesk.rs`. It is known to produce false warnings.

The TUI may show this non-blocking warning when its active state is explicitly
`Unavailable`:

```text
No recent Tailscale response; opening RustDesk anyway
```

Do not block connect or file-transfer actions. Do not warn for `Unknown`, because
the first probe may simply not have completed.

### 11.2 CLI human list

`portal list` cannot provide an accurate live status without delaying the command
for every peer. Remove the human-readable `STATUS` column instead of continuing
to print `online` or `offline` from the coordination hint.

Keep `Device.online` in `portal list --json` for compatibility in this phase, and
document it as the raw Tailscale coordination hint rather than Portal's active
availability result. Do not add background-probe latency to JSON list output.

Update the CLI rendering tests and README accordingly.

## 12. Sequential implementation phases

Work through these phases in order. Run focused tests after each meaningful
change and the full validation commands at every phase boundary.

### Phase 1: Core command construction and parsing

1. Add `portal-core/src/probe.rs`.
2. Add the exact TSMP argument construction.
3. Add safe process execution and `ProbeError`.
4. Add duration parsing and `ProbeMeasurement`.
5. Add `Portal::probe` using the configured Tailscale executable.
6. Re-export only the public types the TUI needs.
7. Add sanitized unit tests.
8. Run:

```text
cargo fmt --all -- --check
cargo clippy -p portal-core --all-targets -- -D warnings
cargo test -p portal-core
```

Do not continue until the exact argument-vector test, latency parser tests, and
failure sanitization tests pass.

### Phase 2: Pure availability state and reconciliation

1. Add `AvailabilityState`, `Availability`, and `ProbeTarget` to TUI app state.
2. Add success and failure transitions with the exact intervals and hysteresis.
3. Reconcile state by stable node ID and preferred IP after discovery.
4. Remove entries for disappeared nodes.
5. Add due-target selection with selected-device priority.
6. Update status sorting to use availability.
7. Add deterministic app tests using a controlled base `Instant`.
8. Run TUI app tests and the full workspace test suite.

Do not start any worker in this phase. Test state and scheduling as pure app
logic first.

### Phase 3: Probe worker integration

1. Add the bounded probe request channel and response channel.
2. Spawn exactly one `portal-probe` worker.
3. Drain results without blocking the event loop.
4. Start at most one due probe when idle.
5. Ignore stale ID/IP results.
6. Make manual refresh mark probes due.
7. Shut down and join the worker cleanly.
8. Add runtime-level tests where practical without invoking live Tailscale.
9. Run Clippy, workspace tests, and a release build.

Confirm by inspection that discovery and probe processes cannot overlap with
another process of the same type and that only one probe is ever active.

### Phase 4: Accurate rendering and latency placement

1. Update header reachable count and label.
2. Update filled and empty row dots.
3. Update details status text.
4. Add right-aligned details latency on the machine-name line.
5. Add list-only latency columns using the exact width thresholds.
6. Update status-color and responsive-layout tests.
7. Test Portal, Hacker, and Cyber Cyan themes.
8. Run all TUI tests and the full workspace suite.

Use `ratatui::backend::TestBackend` assertions for all three layout families:
side-by-side details, stacked details, and list-only.

### Phase 5: Remove misleading old status uses

1. Remove the RustDesk warning based on `Device.online`.
2. Add the TUI-only unavailable warning without blocking launch.
3. Remove the CLI human `STATUS` column.
4. Verify no TUI code reads `Device.online`.
5. Update tests and README status semantics.
6. Search the repository for remaining misleading `online` and `offline` labels.
7. Run the full validation suite.

The raw field may remain only in Tailscale parsing, the core device model, JSON
serialization, sanitized parser tests, and documentation that calls it a hint.

### Phase 6: Automated and manual validation

Run:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Then perform locally safe validation:

1. Start Portal with multiple desktop peers.
2. Confirm new peers begin with empty dots and fill individually after success.
3. Confirm only one `tailscale ping` process runs at a time.
4. Confirm the TUI remains responsive while a probe times out.
5. Confirm latency appears beside the name in side-by-side details.
6. Resize to stacked details and confirm latency remains in details.
7. Resize to list-only widths above 72, between 54 and 71, and between 48 and 53.
8. Confirm latency appears only in the list when details are hidden.
9. Confirm status sorting preserves the selected device as results arrive.
10. Confirm a coordination-hint false negative becomes reachable after a
    successful probe.
11. Press `r` and confirm probes are reprioritized but remain sequential.
12. Connect and transfer to a reachable device.

Ask the user before tests that stop Tailscale, disconnect a peer, change a
firewall, or otherwise affect connectivity. With approval, also verify:

1. one failed probe does not extinguish a healthy dot;
2. two consecutive failures produce an empty dot and `unavailable` details;
3. one successful recovery probe restores the dot immediately; and
4. relayed connectivity still counts as reachable.

Do not update the installed Portal binary until automated validation passes and
the currently running Portal process is closed.

## 13. Completion checklist

- [ ] `Device.online` no longer controls any displayed availability.
- [ ] TSMP command construction uses exact separate arguments.
- [ ] Only one probe process runs at a time.
- [ ] New and changed-IP nodes probe immediately.
- [ ] Reachable nodes probe every 15 seconds.
- [ ] Failed nodes retry every 5 seconds.
- [ ] Two failures are required before unavailable.
- [ ] One success restores reachable immediately.
- [ ] Latency parsing failure does not turn success into failure.
- [ ] Latency is shown in details when details exist.
- [ ] Latency is shown in the list only when details are hidden.
- [ ] Header count, details text, dots, and status sorting use active availability.
- [ ] RustDesk launch is never blocked by availability.
- [ ] CLI human output no longer claims coordination hints are live status.
- [ ] Probe results remain in memory only.
- [ ] No raw ping output or private tailnet data is logged or persisted.
- [ ] Formatting, Clippy, tests, and release build pass without warnings.
- [ ] README describes the new status and latency behavior.
- [ ] Locally possible manual validation is complete.
