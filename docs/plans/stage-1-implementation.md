# Portal Stage 1 Implementation Plan

Status: Ready for implementation
Scope: Stage 1 — CLI
Primary product reference: `C:\Files\Vaults\Ghost\Dev\Notebook\portal-prd.md`

## 1. Objective

Build a small cross-platform Rust application that discovers devices from the
local Tailscale client, overlays optional address-book metadata, resolves a
human-entered device name, and launches the appropriate RustDesk action.

Stage 1 is complete when the following interface works:

```text
portal list
portal list --json
portal <name>
portal connect <name>
portal share <name>
portal transfer <name>       # alias for share
portal init <name>
portal provision <name>      # alias for init
```

The implementation must remain directly reusable by the Stage 2 TUI. The core
must expose device snapshots and actions rather than CLI-specific commands or
formatted strings.

## 2. Current Environment

These facts were verified when this plan was written:

- Workspace: `C:\Code\apps\portal`
- The workspace started empty and is not yet a Git repository.
- Rust and Cargo are not installed or are not available on `PATH`.
- Tailscale 1.88.3 is available on `PATH`.
- `tailscale status --json` works and currently reports seven peers.
- RustDesk 1.4.6 is installed at:
  `C:\Program Files\RustDesk\rustdesk.exe`
- RustDesk is not currently on `PATH`.

Do not put real tailnet hostnames, addresses, node IDs, user IDs, public keys, or
other private Tailscale output into committed fixtures.

## 3. Scope

### 3.1 Included

- A Cargo workspace containing `portal-core` and `portal-cli`.
- On-demand parsing of `tailscale status --json`.
- Discovered peer hostname, DNS name, stable node ID, IP addresses, OS, and
  online state.
- iOS and Android peers are excluded because Stage 1 actions cannot target
  mobile devices.
- An optional local TOML address book containing nicknames, tags, favorites,
  and notes.
- Deterministic device resolution by nickname, hostname, DNS name, or exact
  Tailscale IP.
- RustDesk remote desktop and file-transfer launch actions.
- The provisioning behavior selected at the checkpoint in section 13.
- Human-readable device listing.
- JSON device listing for scripts.
- Machine-managed last-connected state for the future TUI.
- Windows, macOS, and Linux compilation and automated tests.
- Clear user-facing errors.

### 3.2 Explicitly deferred

- TUI implementation, Ratatui, key handling, polling loops, and terminal color.
- Theme parsing or the five-token theme model.
- Port forwarding.
- Credential storage or the vault.
- Password manager integration.
- Tauri or any GUI code.
- Network APIs or direct use of Tailscale libraries.
- Fuzzy search.
- A general plugin, backend, or provider system.
- Async Rust or an async runtime.
- Logging frameworks and telemetry.
- Empty crates for future stages.

Do not add deferred features “for completeness.” Add only the seams described
in this plan.

## 4. Design Principles

1. Prefer plain functions and data structures over traits.
2. Keep external process execution at the edge of `portal-core`.
3. Make parsing, merging, resolution, sorting, and command construction pure
   functions so they can be tested without spawning applications.
4. Never invoke RustDesk or Tailscale through a shell.
5. Never store credentials in the address book or usage state.
6. Parse only the Tailscale fields Portal needs and allow unknown JSON fields.
7. Keep user-managed configuration separate from machine-managed state.
8. Use stable Tailscale node IDs to preserve TUI selection across refreshes.
9. Keep all UI rendering in frontend crates.
10. Do not introduce an abstraction until there are at least two real callers
    or implementations that need it.

## 5. Target Repository Structure

The following is the complete Stage 1 structure:

```text
portal/
├── .github/
│   └── workflows/
│       └── check.yml
├── docs/
│   └── plans/
│       └── stage-1-implementation.md
├── portal-core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── config.rs
│   │   ├── model.rs
│   │   ├── platform.rs
│   │   ├── rustdesk.rs
│   │   ├── state.rs
│   │   └── tailscale.rs
│   └── tests/
│       └── fixtures/
│           ├── tailscale-status.json
│           ├── tailscale-empty.json
│           └── tailscale-ipv6-only.json
├── portal-cli/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── .gitignore
├── Cargo.lock
├── Cargo.toml
└── README.md
```

Do not split modules further unless a file becomes difficult to navigate.

## 6. Dependency Budget

Use current stable, mutually compatible releases. Do not pin versions in this
plan; let Cargo create and commit `Cargo.lock`.

`portal-core` production dependencies:

- `serde`, with `derive`
- `serde_json`
- `toml`
- `directories`
- `thiserror`

`portal-cli` production dependencies:

- `portal-core`, by workspace path
- `clap`, with `derive`
- `serde_json`

Development dependencies:

- `tempfile`

Do not add `tokio`, `async-std`, `tracing`, `anyhow`, `which`, `chrono`, a table
formatting library, a snapshot testing library, or a mocking framework without
a concrete blocker and user approval.

Use `std::time::SystemTime` and Unix seconds for the one timestamp Stage 1
needs. Implement executable lookup with the standard library because Portal
needs only two named programs and a short platform candidate list.

## 7. Core Data Model

Define the public types in `portal-core/src/model.rs`.

The exact ownership types may be adjusted for readability, but the public
information must be equivalent to:

```rust
pub struct Device {
    pub id: String,
    pub hostname: String,
    pub dns_name: Option<String>,
    pub nickname: Option<String>,
    pub ips: Vec<IpAddr>,
    pub preferred_ip: IpAddr,
    pub os: String,
    pub online: bool,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub notes: Option<String>,
    pub last_connected_at: Option<u64>,
}

pub enum Action {
    Connect,
    Transfer,
}

pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}
```

Requirements:

- `id` is the stable Tailscale node ID, not the hostname.
- `dns_name` is stored without a trailing dot.
- Preserve all valid Tailscale IPs.
- Prefer an IPv4 address for `preferred_ip`; fall back to IPv6.
- A peer with no valid IP cannot be used and should be skipped or surfaced as
  malformed. Choose one behavior and test it. Prefer skipping it while
  recording enough context in the returned error only if all peers are invalid.
- Do not invent a separate `OnlineStatus` enum while the state is only boolean.
- Derive serialization only where needed for `portal list --json` or local
  state.

## 8. Public Core API

Expose a small facade from `portal-core/src/lib.rs`:

```rust
pub struct Portal {
    // Resolved config paths and executable overrides only.
}

impl Portal {
    pub fn load(config_override: Option<&Path>) -> Result<Self, PortalError>;
    pub fn snapshot(&self) -> Result<Vec<Device>, PortalError>;
    pub fn launch_spec(
        &self,
        action: Action,
        device: &Device,
    ) -> Result<Launch, PortalError>;
    pub fn launch(
        &self,
        action: Action,
        device: &Device,
    ) -> Result<(), PortalError>;
}

pub fn resolve<'a>(
    devices: &'a [Device],
    query: &str,
) -> Result<&'a Device, ResolveError>;
```

Exact signatures may change if Rust ownership requires it. Preserve these
boundaries:

- `snapshot` performs one on-demand discovery and returns unformatted data.
- `resolve` operates only on an existing snapshot and does no I/O.
- `launch_spec` constructs a command without spawning it.
- `launch` spawns the exact `Launch`, then records last-connected state.
- The future TUI can call `snapshot` from a worker thread, retain selection by
  `Device.id`, and call `launch` using the selected device.

Do not put polling or caching inside `Portal`. The TUI owns polling cadence and
snapshot replacement.

## 9. Tailscale Discovery

### 9.1 Execution

Run exactly:

```text
tailscale status --json
```

Executable resolution order:

1. Address-book/config override.
2. Search each entry in `PATH`.
3. Known platform installation candidates.

Candidate lists must remain short and obvious. At minimum:

- Windows Tailscale:
  `C:\Program Files\Tailscale\tailscale.exe`
- Windows RustDesk:
  `C:\Program Files\RustDesk\rustdesk.exe`
- macOS RustDesk:
  `/Applications/RustDesk.app/Contents/MacOS/RustDesk`
- Common Unix names/paths:
  `tailscale`, `/usr/bin/tailscale`, `/usr/local/bin/tailscale`,
  `rustdesk`, `/usr/bin/rustdesk`, `/usr/local/bin/rustdesk`

Do not recursively scan disks or inspect a package manager.

### 9.2 Parsing

Create private deserialization structs containing only required fields:

- Top-level `Self`
- Top-level `Peer`
- Peer `ID`
- Peer `HostName`
- Peer `DNSName`
- Peer `OS`
- Peer `TailscaleIPs`
- Peer `Online`

Tailscale currently represents peers as a JSON object keyed by identity. Treat
the object values as the peer list. Do not rely on object iteration order.

Exclude `Self` from the returned device list. Stage 1 is for reaching peers.

Parser behavior:

- Unknown fields are ignored.
- An empty peer object returns an empty vector.
- A trailing dot is removed from DNS names.
- IP strings are parsed using `std::net::IpAddr`.
- Invalid individual IP strings are ignored if another valid IP remains.
- Devices are sorted by case-insensitive display name before CLI rendering.

### 9.3 Process failures

Return distinct, readable errors for:

- Tailscale executable not found.
- Tailscale command could not start.
- Tailscale returned a nonzero exit status.
- Output was not valid UTF-8 or valid JSON.
- Tailscale is stopped, logged out, or otherwise unhealthy when that can be
  inferred from status output.

Include useful stderr but do not dump the entire environment or config.

## 10. Address Book and Usage State

### 10.1 Address-book location

Use `directories::ProjectDirs` with application name `portal`.

Default user-managed path:

- Windows: `%APPDATA%\portal\config.toml`
- macOS: `~/Library/Application Support/portal/config.toml`
- Linux: `$XDG_CONFIG_HOME/portal/config.toml`, falling back according to the
  `directories` crate

Support one global CLI override:

```text
portal --config <path> ...
```

Do not add separate command-line flags for both executable paths. The config
file already provides explicit overrides.

### 10.2 Address-book schema

```toml
[executables]
# tailscale = "/custom/path/to/tailscale"
# rustdesk = "/custom/path/to/rustdesk"

[devices.workstation]
nickname = "rig"
tags = ["desktop", "primary"]
favorite = true
notes = "Main workstation"
```

Behavior:

- A missing config file is valid and acts as an empty address book.
- A present but malformed config file is an error.
- Device table keys match discovered hostnames case-insensitively.
- Metadata for an undiscovered hostname remains dormant and is not emitted as
  an offline synthetic device.
- Missing nickname, tags, favorite, or notes use natural empty defaults.
- Tags retain user order but exact duplicates are removed.
- Empty or whitespace-only nicknames and tags are rejected.
- Duplicate nicknames across configured devices are rejected
  case-insensitively.
- Credentials are not valid fields. Unknown fields should be rejected so
  misspellings do not silently disappear.

### 10.3 Usage state

Store machine-managed state under the platform local-data directory as
`state.json`. Keep it separate from `config.toml`.

Minimal schema:

```json
{
  "last_connected": {
    "n123456": 1785451200
  }
}
```

Behavior:

- Keys are stable Tailscale node IDs.
- Values are Unix seconds.
- Missing state is valid.
- Invalid state should produce a warning and act as empty state; it must not
  prevent discovery or connection.
- Write to a temporary file in the same directory, then rename it over the
  prior state file.
- Record a timestamp only after `Command::spawn` succeeds.
- Failure to persist state should warn but should not turn a successful
  RustDesk launch into a failed action.

This state is required now because the Stage 2 TUI sorts by last-connected.

## 11. Device Resolution

Normalize the query by trimming whitespace. For host and DNS names, compare
case-insensitively and ignore a final DNS dot.

A device matches if the query exactly equals any of:

- Nickname
- Hostname
- DNS name
- Any Tailscale IP

Do not use fuzzy, substring, or prefix matching.

Collect matches by stable device ID:

- Zero devices: return `NotFound`.
- One device: return it.
- More than one device: return `Ambiguous` with enough display information for
  the frontend to list candidates.

Do not prioritize nickname over hostname. A cross-namespace collision is
ambiguous and should be corrected in config rather than silently selecting a
device.

Required tests:

- Exact nickname.
- Nickname with different letter case.
- Hostname.
- DNS name with and without trailing dot.
- IPv4.
- IPv6.
- No match.
- Two devices matching through different identifier types.
- Leading/trailing query whitespace.

## 12. CLI

Keep all argument parsing and presentation in `portal-cli/src/main.rs` unless
the file becomes genuinely difficult to navigate.

### 12.1 Commands

```text
portal list
portal list --json
portal <name>
portal connect <name>
portal share <name>
portal transfer <name>
portal init <name>
portal provision <name>
```

Rules:

- `portal <name>` is exactly equivalent to `portal connect <name>`.
- `share` is the canonical file-transfer spelling from the PRD command table.
- `transfer` is a Clap alias and does not get a second execution path.
- `init` is the canonical provisioning spelling.
- `provision` is a Clap alias and does not get a second execution path.
- Bare `portal` prints concise help plus a note that the TUI arrives in Stage 2.
- Reserve recognized command names. A device named `list`, `connect`, `share`,
  `transfer`, `init`, or `provision` must be addressed through an unambiguous
  identifier such as its IP if needed.
- Invalid arguments use Clap's normal exit code and diagnostics.
- Runtime failures print one concise message to stderr and return failure.

Add a global or action-level `--dry-run` that resolves the target and prints
the program and argument vector without spawning RustDesk. Make clear that the
display is diagnostic, not a shell command intended for copy/paste.

### 12.2 List presentation

Default columns:

```text
STATUS   NAME          HOSTNAME       OS       IP
online   rig           workstation    windows  100.64.0.10
offline  laptop        laptop         macOS    100.64.0.11
```

Requirements:

- `NAME` uses nickname, falling back to hostname.
- Use plain text; do not add color or icon dependencies in Stage 1.
- Implement the small amount of padding directly.
- Empty device lists print a useful one-line message.
- Default ordering is case-insensitive display name.

`portal list --json`:

- Writes only valid JSON to stdout.
- Writes errors only to stderr.
- Includes the merged `Device` data needed by scripts.
- Uses stable, documented field names.
- Does not include executable paths or local config paths.

## 13. RustDesk Actions and Required Provisioning Checkpoint

### 13.1 Validate before implementation

Do not assume every command line in the PRD works unchanged in RustDesk 1.4.6.
Before writing action construction:

1. Inspect official RustDesk documentation and the current upstream source.
2. Validate the installed Windows executable with non-destructive commands.
3. Confirm direct-IP remote desktop syntax.
4. Confirm direct-IP file-transfer syntax.
5. Confirm whether the RustDesk process returns immediately or stays attached.
6. Record the verified argument vectors in this document or a short linked
   implementation note.
7. Do not run installation, service, password, or config mutation flags during
   this validation.

If the PRD spelling is unsupported, report the evidence and use the supported
equivalent after user confirmation.

#### RustDesk 1.4.6 verification (2026-07-30)

Verified against the installed Windows build at
`C:\Program Files\RustDesk\rustdesk.exe` and the matching
[RustDesk 1.4.6 `core_main.rs`](https://github.com/rustdesk/rustdesk/blob/1.4.6/src/core_main.rs):

- `rustdesk.exe --version` exits successfully and reports `1.4.6`.
- The 1.4.6 argument parser recognizes `--connect <target>` and
  `--file-transfer <target>`. It does not recognize a bare target as a
  connection action.
- `rustdesk.exe --connect 192.0.2.10` opened remote-control mode for the
  documentation-reserved address.
- `rustdesk.exe --file-transfer 192.0.2.11` opened file-transfer mode for the
  documentation-reserved address.
- Both probe processes remained attached to their GUI sessions for more than
  five seconds and were terminated. Portal must spawn and release the child
  handle rather than wait for completion.
- No installation, service, password, config import, or setting mutation flag
  was run.

Subject to the required user confirmation, the Stage 1 launch vectors are:

```text
Connect:  ["--connect", "<preferred-ip>"]
Transfer: ["--file-transfer", "<preferred-ip>"]
```

### 13.2 Connect and transfer

Construct RustDesk commands as an argument vector, never a command string.

Requirements:

- Target the device's `preferred_ip`.
- Never invoke through PowerShell, `cmd.exe`, `sh`, or another shell.
- Spawn the process and release the child handle rather than waiting for the
  entire GUI session to finish.
- A device marked offline produces a warning but is still allowed.
- A missing RustDesk executable is a clear error with the searched locations.
- `launch_spec` is fully unit-tested without starting RustDesk.
- `--dry-run` uses the same `launch_spec` as the real action.

### 13.3 Provisioning checkpoint

Stop and ask the user for one product decision only after producing a concise
platform capability matrix.

The question to resolve:

> Does `portal init <name>` provision a remote peer, prepare the current local
> machine, or generate a target-specific procedure to run on the peer?

This cannot be guessed because discovery does not provide a cross-platform
remote execution channel, and a not-yet-provisioned RustDesk host cannot be
used to bootstrap itself.

Recommended Stage 1 behavior:

- Provide a local provisioning operation for the current machine.
- Optionally generate or display the procedure appropriate to a selected peer's
  OS.
- Do not add Tailscale SSH or another remote-management subsystem unless the
  user explicitly selects that expanded scope.

Before asking, investigate:

- Windows EXE and MSI installation behavior.
- Windows service installation and required elevation.
- RustDesk config import.
- Direct-access enablement and whitelist configuration.
- macOS application installation and permissions that cannot be automated.
- Linux packaging differences and service installation.
- Whether any password operation exposes the value through process arguments.

Provisioning safety:

- Preview privileged or persistent changes.
- Require explicit confirmation unless `--dry-run` is active.
- Never put credentials in Portal config, state, logs, test fixtures, or error
  messages.
- Do not pass a password on a process command line without explicit user
  approval after documenting the exposure.
- Do not claim success without verifying the installed service/config state.

#### Provisioning capability matrix (2026-07-30)

Research sources:

- [RustDesk client installation and command-line parameters](https://rustdesk.com/docs/en/client/)
- [RustDesk client deployment examples](https://rustdesk.com/docs/en/self-host/client-deployment/)
- [Windows MSI deployment](https://rustdesk.com/docs/en/client/windows/msi/)
- [macOS installation and privacy permissions](https://rustdesk.com/docs/en/client/mac/)
- [Linux packages and platform limitations](https://rustdesk.com/docs/en/manual/linux/)
- [Direct access and whitelist settings](https://rustdesk.com/docs/en/self-host/client-configuration/advanced-settings/)
- [RustDesk 1.4.6 command handling](https://github.com/rustdesk/rustdesk/blob/1.4.6/src/core_main.rs)

| Capability | Windows | macOS | Linux |
|---|---|---|---|
| Install | EXE `--silent-install` or silent MSI; elevation required | Mount DMG and copy app to `/Applications`; root required for scripted copy | Root package-manager operation; package differs across DEB, RPM, Arch, openSUSE, Nix, AppImage, and Flatpak |
| Service | Installed EXE supports `--install-service`; elevation required; verify the `RustDesk` service is running | RustDesk can run its server component, but app installation alone does not grant privacy permissions | System packages install a service managed with `systemctl`; AppImage/Flatpak are different user-scoped cases |
| Config import | `--config` and `--import-config` are supported; installed client and administrator context are required for protected settings | Supported by the app executable; deployment examples run with root | Supported by the installed executable; deployment examples run as root |
| Direct access | `direct-server=Y`; whitelist supports CIDR, including the tailnet range | Same RustDesk options | Same RustDesk options |
| Manual requirements | UAC/elevation and firewall/service verification | Gatekeeper plus Accessibility, Screen Recording, and sometimes Input Monitoring must be granted interactively; reboot may be required | Display-server/login-screen and SELinux behavior vary by distribution |
| Password safety | `--password <value>` places the password in the process argument vector | Same | Same |
| Remote execution from Portal | None | None | None |

Consequences:

- `portal init <name>` cannot directly provision the discovered peer without
  adding a separate remote-management transport.
- A local operation can safely preview and then perform current-machine
  installation/service/config changes, but the selected peer name does not
  identify the machine being changed.
- A generated target-specific procedure can honor the peer OS and avoid
  pretending remote execution occurred, but the user must run it on the peer.
- Stage 1 must not accept or pass a RustDesk password because it would be
  exposed through process arguments. Password handling remains deferred.
- Any mutating local implementation must require explicit confirmation and
  must verify the resulting installation, service, direct-access, and whitelist
  state. macOS privacy approval cannot be automated or claimed complete.

Selected Stage 1 behavior:

- `portal init <name>` prepares the current local machine. The selected peer is
  resolved to make the intended connection context explicit, but Portal changes
  only the machine on which the command is running.
- The implemented provisioning path is Windows-only until macOS and Linux
  hardware is available for runtime testing.
- RustDesk must already be installed. Portal previews an idempotent plan,
  requires confirmation and administrator elevation, ensures the service is
  running, enables direct access, applies `100.64.0.0/10` as the whitelist, and
  reads the resulting state before reporting success.
- `--dry-run` and a negative confirmation make no persistent changes.
- The privileged apply-and-verify checklist remains a manual test requiring
  explicit approval on the development machine.

## 14. Errors

Use `thiserror` in `portal-core` for a compact structured error enum.

Errors should preserve:

- Operation that failed.
- Relevant executable or config path.
- Exit code when an external program returns one.
- Concise stderr when safe.
- Ambiguous-device candidates.

Errors must not include:

- Full environment variables.
- Tailscale keys.
- Credentials.
- Complete raw Tailscale JSON.

The CLI may format a core error into one or two readable lines. Do not build an
error-reporting framework.

## 15. Implementation Sequence

Follow this order. Each phase must finish with green tests before proceeding.

### Phase 0 — Prerequisites

User validation required:

- Ask permission to install the standard Rust toolchain because it is currently
  absent.

Then:

1. Install stable Rust with Cargo and Rustfmt.
2. Confirm `rustc --version`, `cargo --version`, and `rustfmt --version`.
3. Initialize Git if the directory is still not a repository.
4. Read the PRD and this plan before editing.
5. Inspect existing files and preserve user changes.

Exit criterion: Rust tooling works and the repository contains no unexpected
files.

### Phase 1 — Workspace bootstrap

1. Create the root workspace manifest with resolver version 2.
2. Create `portal-core` as a library crate.
3. Create `portal-cli` as a binary crate whose binary name is `portal`.
4. Add only the dependencies in section 6.
5. Add `.gitignore` for `/target`, editor noise, and OS noise.
6. Add a minimal README describing the project and current Stage 1 status.
7. Run format, build, test, and Clippy.

Exit criterion: both crates compile with placeholder entry points and no
warnings.

### Phase 2 — Model and Tailscale parser

1. Implement the core model.
2. Create sanitized JSON fixtures representing:
   - Online and offline peers.
   - Extra unknown fields.
   - IPv4 and IPv6 addresses.
   - Empty peer set.
   - IPv6-only peer.
3. Implement pure JSON parsing.
4. Normalize DNS names and IPs.
5. Exclude the local node.
6. Add parser unit tests.

Exit criterion: fixtures parse deterministically and contain no private data.

### Phase 3 — Executable lookup and live discovery

1. Implement platform candidate generation.
2. Implement `PATH` search with `std`.
3. Add config overrides to executable resolution.
4. Execute `tailscale status --json` directly.
5. Map process failures into core errors.
6. Test candidate generation and lookup using temporary directories.
7. Smoke-test against the installed Tailscale client.

Exit criterion: a core call returns the actual peer snapshot.

### Phase 4 — Config and state

1. Implement default config and state paths.
2. Deserialize config with unknown-field rejection.
3. Validate nicknames and tags.
4. Merge metadata into discovered peers.
5. Load and merge last-connected state.
6. Implement atomic state writes.
7. Add config, merge, and state tests using temporary directories.

Exit criterion: missing config/state works, valid metadata merges, and malformed
user config fails clearly.

### Phase 5 — Resolution

1. Implement exact normalized matching.
2. Deduplicate matches by stable node ID.
3. Implement not-found and ambiguity errors.
4. Add every test listed in section 11.

Exit criterion: all identifiers resolve deterministically without fuzzy logic.

### Phase 6 — First usable CLI

1. Implement Clap parsing.
2. Implement `portal list`.
3. Implement `portal list --json`.
4. Implement shorthand parsing and explicit command stubs.
5. Implement concise errors and help.
6. Add parser tests for all documented command forms.
7. Verify stdout/stderr separation.

Exit criterion: discovery and address-book listing are useful before RustDesk
actions are added.

### Phase 7 — RustDesk contract investigation

1. Perform section 13.1.
2. Record verified syntax.
3. Ask the user only if observed behavior conflicts with the PRD.
4. Do not mutate RustDesk settings in this phase.

Exit criterion: connect and transfer argument vectors are known rather than
assumed.

### Phase 8 — Connect and transfer

1. Implement `launch_spec`.
2. Unit-test its program and individual arguments.
3. Implement `--dry-run`.
4. Implement direct spawning and offline warning.
5. Record last-connected state after successful spawn.
6. Wire shorthand, `connect`, `share`, and `transfer` through the same path.
7. Perform live manual connection and file-transfer tests.

Exit criterion: all connect spellings share one implementation and launch the
correct RustDesk mode.

### Phase 9 — Provisioning

1. Produce the platform capability matrix from section 13.3.
2. Request the single provisioning decision.
3. Implement only the selected behavior.
4. Add pure plan/argument construction tests.
5. Test `--dry-run`.
6. Perform privileged testing only after explicit confirmation.

Exit criterion: provisioning behavior is explicit, safe, documented, and
validated on supported platforms.

### Phase 10 — Final hardening and release

1. Add cross-platform CI.
2. Complete README usage and configuration documentation.
3. Run all automated validation in section 16.
4. Run applicable manual validation in section 17.
5. Record limitations and untested platforms truthfully.
6. Produce a release build.
7. Inspect the final dependency tree and remove unused dependencies.

Exit criterion: every applicable acceptance item passes and remaining platform
checks are explicitly identified.

## 16. Automated Testing and Validation

### 16.1 Required commands

Run from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --release
```

Also inspect:

```text
cargo tree
```

Every dependency must correspond to section 6 or have a clear transitive
origin. Remove unused direct dependencies.

### 16.2 Core test matrix

Tailscale parsing:

- Valid mixed online/offline peers.
- Unknown JSON fields are accepted.
- Empty peer object.
- DNS trailing-dot normalization.
- IPv4 preference over IPv6.
- IPv6 fallback.
- Invalid IP ignored when another valid IP exists.
- Peer with no usable IP follows the documented behavior.
- JSON object order does not change sorted output.
- Self is excluded.
- Invalid JSON returns a parsing error.

Config:

- Missing config is empty config.
- Minimal valid config.
- Full valid config.
- Executable path overrides.
- Unknown config field rejected.
- Empty nickname rejected.
- Whitespace-only tag rejected.
- Duplicate tags normalized.
- Duplicate nickname rejected case-insensitively.
- Metadata merges case-insensitively by hostname.
- Undiscovered config entries do not create devices.

State:

- Missing state is empty state.
- Valid state merges by node ID.
- State does not merge by hostname.
- Invalid state is nonfatal.
- Atomic replacement produces valid JSON.
- Successful launch records time.
- Failed launch does not record time.
- State-write failure does not convert a successful launch into failure.

Resolution:

- Every case in section 11.
- A nickname colliding with another hostname is ambiguous.
- Two aliases for the same node count as one match.

Executable lookup:

- Config override wins.
- `PATH` wins over fallback candidates.
- Platform candidate is used when absent from `PATH`.
- Missing executable reports searched locations.
- Paths containing spaces remain one executable path.
- Windows executable-extension behavior.

RustDesk launch specification:

- Connect action exact argument vector.
- Transfer action exact argument vector.
- IPv4 target.
- IPv6 target.
- Configured RustDesk path.
- Fallback RustDesk path.
- No argument is shell-concatenated.

CLI parsing and rendering:

- Bare invocation.
- Shorthand connect.
- Explicit connect.
- Share and transfer alias.
- Init and provision alias.
- Missing target.
- Unknown flag.
- Human list with and without nicknames.
- Empty list.
- JSON list parses successfully with `serde_json`.
- Runtime errors go to stderr rather than corrupting JSON stdout.
- `--dry-run` does not spawn.

### 16.3 CI matrix

Create `.github/workflows/check.yml` with:

- `windows-latest`
- `macos-latest`
- `ubuntu-latest`

For each platform:

1. Check out the repository.
2. Install stable Rust with Rustfmt and Clippy.
3. Run formatting once or on one job.
4. Run Clippy with warnings denied.
5. Run all workspace tests.
6. Build the release binary.

Tests must not require Tailscale or RustDesk to be installed in CI. Pure parsing,
resolution, merge, lookup, and launch-spec tests provide boundary coverage.

## 17. Manual Final Validation

Manual checks validate real external applications and OS behavior that unit
tests cannot prove.

### 17.1 Test prerequisites

For each tested controller:

- Tailscale installed, running, and authenticated.
- At least one online and, if possible, one offline peer.
- RustDesk installed.
- The online target has RustDesk running as a service.
- Direct IP access is enabled on the target.
- Target security settings allow the intended tailnet traffic.
- A temporary Portal config is available for nickname testing.

Never weaken the target firewall or security configuration merely to make a
test pass without recording and approving the change.

### 17.2 Windows discovery and list

Run:

```text
portal list
portal list --json
```

Verify:

- The local machine is absent.
- Expected peers appear once.
- Online state matches `tailscale status`.
- Hostname, OS, and preferred IP are correct.
- IPv4 is preferred when both families exist.
- JSON output parses and contains no diagnostic text.
- RustDesk is found at its standard path despite not being on `PATH`.

### 17.3 Address-book behavior

Using a temporary config:

1. Add a nickname, tags, favorite, and notes to one discovered hostname.
2. Run `portal --config <path> list`.
3. Verify merged metadata in human and JSON output.
4. Connect using nickname, hostname, DNS name, and exact IP.
5. Add a duplicate nickname and verify Portal refuses the malformed config.
6. Add an unknown field and verify it is reported.
7. Add metadata for an undiscovered host and verify no synthetic device appears.

Do not alter the user's primary config during negative tests.

### 17.4 Connect

Run:

```text
portal --dry-run connect <online-name>
portal connect <online-name>
portal <online-name>
```

Verify:

- Dry run shows separate program and argument values and starts nothing.
- RustDesk opens directly for the expected tailnet IP.
- Both explicit and shorthand forms behave identically.
- Portal returns promptly rather than waiting for the GUI session to close.
- Successful spawning updates the selected node's state timestamp.
- No credentials appear in output or state.

### 17.5 File transfer

Run:

```text
portal --dry-run share <online-name>
portal share <online-name>
portal transfer <online-name>
```

Verify:

- Dry run uses the verified file-transfer argument vector.
- RustDesk opens in file-transfer mode for the expected target.
- `share` and `transfer` are behaviorally identical.
- A harmless small file can be transferred in each direction if target policy
  permits it.

### 17.6 Offline and failure behavior

Without changing the actual Tailscale installation:

- Select a peer reported offline and confirm Portal warns but attempts launch.
- Use a temporary config with a nonexistent RustDesk override and confirm the
  searched-path error.
- Use a temporary config with a nonexistent Tailscale override and confirm the
  discovery error.
- Use malformed temporary TOML and confirm a location-aware parse error.
- Use an unknown device name and confirm a not-found error.
- Create an intentional identifier collision in temporary config and confirm an
  ambiguity error lists candidates.
- Make a temporary state path unwritable where practical and verify a successful
  RustDesk spawn is not reported as failed.

Do not stop Tailscale or modify the user's real config merely to test errors.

### 17.7 Argument-safety checks

Using temporary config metadata:

- Use a nickname containing spaces.
- Use harmless punctuation in notes and tags.
- If validation permits it, attempt a nickname containing shell metacharacters
  such as `&` or `;`.

Verify that these values never become executable shell syntax and that only the
resolved IP is passed to RustDesk.

On Windows, inspect the RustDesk process command line after launch if available:

```powershell
Get-CimInstance Win32_Process |
  Where-Object Name -Like 'rustdesk*' |
  Select-Object Name, CommandLine
```

Confirm there are no Portal credentials or unrelated config values.

### 17.8 State and TUI-readiness

1. Connect to device A.
2. Record its state timestamp.
3. Connect to device B.
4. Confirm both states exist under stable node IDs and B is newer.
5. Rename a nickname and confirm state remains attached to the same device.
6. Run repeated snapshots while changing peer online state.
7. Confirm stable node IDs do not change and are sufficient for a TUI to retain
   selection.

### 17.9 Startup performance

Measure `tailscale status --json` and `portal list` repeatedly after one warm-up.

On Windows:

```powershell
Measure-Command { tailscale status --json | Out-Null }
Measure-Command { portal list | Out-Null }
```

Run at least ten samples. Compare medians rather than one run. Portal should add
only modest parsing and formatting overhead. Investigate if median Portal
overhead exceeds approximately 50 ms on the development machine; do not add a
cache solely to satisfy a synthetic benchmark.

### 17.10 Cross-platform smoke tests

On macOS and Linux:

- Run `portal list` and `portal list --json`.
- Verify executable discovery.
- Verify config paths.
- Verify connect and transfer command syntax against the installed RustDesk
  version.
- Verify Portal returns promptly after spawn.
- Verify state path and atomic update.
- Verify target permissions required by RustDesk.

If physical systems are unavailable, CI compile/tests are necessary but are not
a substitute for these smoke tests. Mark the platform as compile-tested rather
than runtime-validated.

### 17.11 Provisioning

After the section 13 checkpoint is resolved, create an OS-specific manual
checklist that verifies:

- Installation result.
- Service installation and running state.
- Direct-IP access.
- Tailnet whitelist behavior.
- Imported configuration.
- Required elevation.
- Required macOS privacy permissions.
- Re-running provisioning is safe or clearly rejected.
- No credential leaks in process arguments, logs, config, or state.

Do not mark provisioning complete on an OS that has not passed its checklist.

## 18. Final Acceptance Checklist

- [x] Workspace contains only the two Stage 1 crates.
- [x] All automated validation commands pass.
- [ ] CI passes on Windows, macOS, and Linux.
- [x] `portal list` reflects live Tailscale discovery.
- [x] `portal list --json` is clean and scriptable.
- [x] Missing config works.
- [x] Address-book metadata merges correctly.
- [x] Exact resolution works for nickname, hostname, DNS, IPv4, and IPv6.
- [x] Ambiguous identifiers fail safely.
- [x] Connect launches the verified RustDesk direct-IP action.
- [ ] Share launches the verified RustDesk file-transfer action.
- [x] Shorthand and aliases use the same execution paths.
- [x] Offline peers warn but remain actionable.
- [x] Last-connected state updates atomically by stable node ID.
- [x] No shell is used to launch external commands.
- [x] No secrets are stored or printed.
- [x] Provisioning semantics were explicitly selected and validated.
- [x] README documents installation, config, commands, and limitations.
- [x] Release binary builds.
- [ ] Windows runtime smoke tests pass.
- [x] macOS and Linux are accurately labeled runtime-tested or compile-tested.
- [x] The core API can support Stage 2 polling and selection without changes.

## 19. Agent Handoff Rules

An implementing agent should:

1. Read the PRD and this entire plan before acting.
2. Work through phases in order.
3. Keep one phase in progress at a time.
4. Run the phase's focused tests after each meaningful change.
5. Run the full automated suite at phase boundaries.
6. Make reasonable local implementation decisions that preserve the contracts
   in this plan.
7. Avoid asking the user about naming, formatting, minor library details, or
   internal code organization.
8. Ask only for:
   - Rust installation permission.
   - A conflict between verified RustDesk behavior and the PRD.
   - The provisioning semantic decision.
   - Approval for privileged or security-affecting manual tests.
9. Report discovered limitations plainly; do not hide them behind abstractions.
10. Leave the repository buildable and tested at the end of every phase.
