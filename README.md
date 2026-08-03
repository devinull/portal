# Portal

Portal is a small cross-platform CLI and terminal interface for discovering
devices on a personal Tailscale network and opening them directly in RustDesk.

Portal includes:

- On-demand `tailscale status --json` discovery.
- Optional TOML nicknames, tags, favorites, and notes.
- Exact resolution by nickname, hostname, DNS name, IPv4, or IPv6.
- Human-readable and scriptable JSON device lists.
- Active background TSMP reachability checks and concise round-trip latency in
  the TUI.
- RustDesk remote desktop and file-transfer actions.
- Stable-ID last-connected state.
- Preview-first local Windows provisioning.

## Requirements

- Tailscale installed, running, and authenticated.
- RustDesk installed on the controller and target.
- Direct IP access enabled on targets.
- A supported stable Rust toolchain to build from source.

Portal searches `PATH` first, then common platform installation paths. On
Windows it finds standard installations under `C:\Program Files\Tailscale` and
`C:\Program Files\RustDesk`.

iOS and Android peers are excluded from discovery because Portal's Stage 1
RustDesk desktop and file-transfer actions cannot target those devices.

## Build

```text
cargo build --workspace --release
```

The executable is produced at `target/release/portal` (`portal.exe` on
Windows). No installation step is required.

## Commands

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

`portal <name>` is shorthand for `portal connect <name>`. `transfer` aliases
`share`, and `provision` aliases `init`.

Use `--dry-run` with an action to resolve the device and display each executable
argument separately without starting RustDesk:

```text
portal --dry-run connect rig
portal --dry-run share rig
portal --dry-run init rig
```

Bare `portal` opens the interactive TUI. The human-readable `portal list` output
does not claim a live status because it does not delay the command to probe every
peer. In `portal list --json`, `online` remains the raw Tailscale coordination
hint for compatibility; it is not Portal's active availability result.

## Configuration

Use the platform config directory or override it globally:

```text
portal --config path/to/config.toml list
```

Default locations are:

- Windows: `%APPDATA%\portal\config.toml`
- macOS: `~/Library/Application Support/portal/config.toml`
- Linux: `$XDG_CONFIG_HOME/portal/config.toml`, with the normal platform
  fallback

Example:

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

Device table keys match discovered hostnames case-insensitively. Unknown fields,
empty nicknames or tags, and duplicate nicknames are errors. Exact duplicate
tags are removed while preserving user order. Metadata for undiscovered
hostnames remains dormant.

Credentials are not accepted in this file.

## RustDesk actions

RustDesk 1.4.6 was verified to use these argument vectors:

```text
Connect:  ["--connect", "<preferred-ip>"]
Transfer: ["--file-transfer", "<preferred-ip>"]
```

Portal always passes the preferred Tailscale IP as a separate process argument
and never invokes RustDesk or Tailscale through a shell. IPv4 is preferred when
available, with IPv6 as the fallback.

Devices remain actionable regardless of availability. The TUI gives a
non-blocking warning only when its active checks have explicitly marked a device
unavailable. Portal returns after RustDesk starts rather than waiting for the GUI
session to close.

## Local provisioning

Stage 1 `portal init <name>` prepares the current Windows machine to receive
connections from the selected peer and other tailnet devices. It:

1. Ensures the installed RustDesk service is running.
2. Enables direct IP access.
3. Restricts incoming access to `100.64.0.0/10`.
4. Reads the resulting service and option state before reporting success.

The command always prints the exact plan first. `--dry-run` makes no changes.
Without `--dry-run`, Portal asks for confirmation and requires an elevated
terminal. Re-running the plan is safe: an existing running service is not
reinstalled, and the two options are set to the same desired values.

Portal does not download RustDesk or accept a password. RustDesk's password
operation exposes the value in the process argument vector, so password
provisioning remains deferred.

macOS and Linux provisioning is intentionally deferred until it can be tested
on those platforms. macOS also requires interactive privacy approval that a CLI
cannot truthfully automate.

## State and security

Machine-managed `state.json` is stored separately under the platform local-data
directory. It contains only stable Tailscale node IDs and Unix timestamps.
Updates use a temporary file in the same directory followed by atomic rename.
Invalid state is nonfatal and produces a warning.

Portal never persists credentials, raw Tailscale status or ping output,
executable paths, or configuration paths in JSON list output or usage state.
Active availability and latency remain in TUI memory only. The tailnet is the
primary transport trust boundary.

## Current limitations

- The GUI, vault, port forwarding, and on-demand path diagnostics are later
  stages.
- Password entry and storage are not implemented.
- Successful RustDesk authentication depends on target configuration.
- Windows provisioning assumes RustDesk is already installed.
- Windows is runtime-tested. macOS and Linux are compile-tested by CI but have
  not been runtime-validated.
- Actual bidirectional file transfer requires an authenticated target and
  remains a manual environment check.
- iOS and Android tailnet peers are intentionally hidden.

The authoritative design and validation checklist is in
[`docs/plans/stage-1-implementation.md`](docs/plans/stage-1-implementation.md).
