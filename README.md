# Portal

Portal is a small cross-platform CLI for discovering devices on a personal
Tailscale network and opening them directly in RustDesk. Its core library is
frontend-neutral so the Stage 2 TUI can reuse discovery, resolution, and action
behavior without duplicating it.

Stage 1 includes:

- On-demand `tailscale status --json` discovery.
- Optional TOML nicknames, tags, favorites, and notes.
- Exact resolution by nickname, hostname, DNS name, IPv4, or IPv6.
- Human-readable and scriptable JSON device lists.
- RustDesk remote desktop and file-transfer actions.
- Stable-ID last-connected state for the future TUI.
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

Bare `portal` displays help. The interactive TUI arrives in Stage 2.

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

Offline devices produce a warning but remain actionable. Portal returns after
RustDesk starts rather than waiting for the GUI session to close.

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

Portal never persists credentials, raw Tailscale status output, executable
paths, or configuration paths in JSON list output or usage state. The tailnet
is the primary transport trust boundary.

## Current limitations

- The TUI, GUI, theming, vault, port forwarding, and polling are later stages.
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
