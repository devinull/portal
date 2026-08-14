# Portal

Portal is a cross-platform terminal interface for finding desktop peers on a
Tailscale network and opening them directly in RustDesk.

It combines asynchronous discovery, active Tailscale reachability checks,
round-trip latency, favorites and nicknames, and RustDesk desktop or file
transfer actions in one responsive TUI.

## Requirements

- Tailscale installed, running, and authenticated.
- RustDesk installed on the controller and target devices.
- RustDesk direct IP access enabled on target devices.

Portal never accepts or stores Tailscale credentials or RustDesk passwords.

## Install

### Windows

Download `PortalSetup-<version>-x86_64.exe` from the
[latest GitHub Release](https://github.com/devinull/portal/releases/latest).
The installer lets you choose between a current-user installation (defaulting
to `%LOCALAPPDATA%\Programs\Portal`) and an all-users installation (defaulting
to `C:\Program Files\Portal` and requiring administrator approval). You can
choose another destination, optionally add Portal to the matching user or
machine `PATH`, and remove it later through Windows Installed Apps. If Tailscale
or RustDesk is not detected, the final page can open that product's official
download page.

A portable Windows ZIP is also published for users who do not want an installer.

### macOS

Download the PKG matching Apple Silicon (`aarch64`) or Intel (`x86_64`) and run
it. The package installs `portal` in `/usr/local/bin`.

### Linux

Install the matching DEB or RPM from GitHub Releases, or extract the portable
archive and place `portal` somewhere on `PATH`.

See [setup instructions](docs/setup.md) for dependency and first-run details.

## Use

Open a new terminal after installation and run:

```text
portal
```

Useful non-interactive commands include:

```text
portal list
portal list --json
portal connect <name>
portal share <name>
portal --dry-run connect <name>
portal init <name>
```

`portal <name>` is shorthand for `portal connect <name>`. `share` opens
RustDesk file transfer. On Windows, `portal init <name>` previews and optionally
applies local RustDesk receiving configuration.

## Configuration

Portal works without a configuration file. Optional nicknames, tags, favorites,
notes, executable overrides, and theme colors use TOML:

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

Default configuration locations:

- Windows: `%APPDATA%\portal\config.toml`
- macOS: `~/Library/Application Support/portal/config.toml`
- Linux: `$XDG_CONFIG_HOME/portal/config.toml`, with the normal platform fallback

## How availability works

Portal keeps Tailscale's JSON status for discovery and metadata, then checks each
desktop peer sequentially with a lightweight TSMP probe. A peer becomes
unavailable only after two consecutive failures and returns to reachable after
one success. Probe output is never logged or persisted.

The `online` value in `portal list --json` remains the raw Tailscale coordination
hint for compatibility; it is not Portal's active reachability result.

## Build from source

Install stable Rust and run:

```text
cargo build --workspace --release --locked
```

The executable is written to `target/release/portal` (`portal.exe` on Windows).

To build and smoke-test the Windows installer:

```powershell
winget install --id JRSoftware.InnoSetup --exact
./packaging/windows/build-installer.ps1
./packaging/windows/test-installer.ps1 -TestPathRegistration
```

## Releases

Pushing a version tag such as `v0.0.1` runs the release workflow, builds native
Windows, macOS, and Linux artifacts, generates checksums and provenance, and
creates a draft GitHub Release. See [the release guide](docs/releasing.md).

## Platform status

- Windows: runtime-tested, including RustDesk provisioning.
- macOS: compiled in CI; dependency permissions and runtime behavior still need
  real-machine release validation.
- Linux: compiled in CI; distribution packages and runtime behavior still need
  clean-machine release validation.

## Security

Portal launches Tailscale and RustDesk directly with separate process arguments;
it never invokes them through a shell. It does not persist raw Tailscale status,
ping output, credentials, auth URLs, or private tailnet names outside the device
fields shown in the interface.

Tailscale and RustDesk are separate products with their own licenses and update
channels. Portal's installer does not bundle their binaries.

## License

Portal is available under the [MIT License](LICENSE).
