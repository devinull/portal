# Portal Installer and Distribution Strategy

Status: initial packaging implemented for Portal v0.0.1; later distribution and
prerequisite automation remain roadmap guidance.

## 1. Goals

Portal installation should:

1. Put the `portal` executable in a stable terminal `PATH` location.
2. Provide signed, architecture-correct artifacts for each supported platform.
3. Detect existing Tailscale and RustDesk installations without modifying them.
4. Explain whether each dependency is missing, installed but not ready, or ready.
5. Offer explicit, optional assistance for missing dependencies.
6. Leave authentication, credentials, privacy permissions, and security-affecting
   configuration under the user's control.
7. Verify the final state and give the user a small, actionable summary.

The installer must not silently install third-party applications, accept license
terms on the user's behalf, enroll a device in a tailnet, create or store auth
keys, set a RustDesk password, or weaken operating-system security controls.

## 2. Recommended artifacts and channels

GitHub Releases should be Portal's canonical release origin on every platform.
Each tagged release should contain immutable binaries or installers, release
notes, a SHA-256 checksum file, and build provenance where practical. GitHub
Releases supports attaching binary assets to tagged releases and is suitable as
the upstream source used by package-manager manifests.

### 2.1 Windows

#### Primary installer

Portal v0.0.1 uses an Inno Setup EXE named
`PortalSetup-<version>-x86_64.exe`. It supports current-user installation under
Local AppData and elevated all-users installation under Program Files, optional
scope-appropriate `PATH` registration, Start menu shortcuts, upgrades, and
uninstallation. Missing Tailscale or RustDesk installations are detected and
offered an available WinGet package, an official download link, or a skip
choice. Third-party binaries are never bundled, and package managers run only
after explicit selection.

An MSI or prerequisite bootstrapper may be considered later if managed
deployment or verified dependency automation becomes a real requirement. It is
not part of the initial release.

Do not use MSIX as the primary format. Its application isolation and execution
alias model add friction for a terminal tool that launches external desktop
applications. A portable ZIP can be offered for advanced users, but it should
not be presented as the default because it does not manage `PATH` or upgrades.

#### Distribution channels

Use these channels in order:

1. GitHub Releases: canonical installer, MSI, portable ZIP, checksums, and notes.
2. WinGet: preferred discoverable package-manager channel after signed releases
   are stable. WinGet supports EXE, MSI, WiX, Burn, portable, and other installer
   types, and its manifests include an installer URL and SHA-256 hash.
3. Direct download from a future Portal website, backed by the same immutable
   GitHub release artifacts.

Chocolatey and Scoop may be added later if users request them. They should not
delay the signed installer or WinGet submission.

Relevant platform guidance:

- [WinGet supported installer formats](https://learn.microsoft.com/en-us/windows/package-manager/winget/)
- [Submit a package to WinGet](https://learn.microsoft.com/en-us/windows/package-manager/package/)
- [Create a WinGet manifest](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest)

### 2.2 macOS

#### Primary installer

Publish a Developer ID-signed and notarized PKG. Prefer a universal binary when
its size remains reasonable; otherwise publish separate Apple Silicon and Intel
packages. The PKG should install the executable in `/usr/local/bin/portal`, or
install it in a versioned support directory and create a stable executable link
there.

The PKG must not attempt to show interactive prerequisite prompts from a
post-install script. macOS Installer scripts are a poor place for onboarding UI,
network downloads, or privacy-permission guidance. Instead, the package should
finish installing Portal and direct the user to run:

```text
portal setup
```

`portal setup` can perform dependency checks in the user's terminal, open
official vendor download pages or installed applications, and walk through the
remaining manual approvals.

Also publish signed and notarized `.tar.gz` archives for package managers and
advanced users. Apple recommends Developer ID signing and notarization for
software distributed outside the Mac App Store, including installer packages.

#### Distribution channels

Use these channels in order:

1. GitHub Releases: canonical PKG, archives, checksums, and notes.
2. A Portal Homebrew tap during early releases.
3. Homebrew core after Portal has stable tagged releases and meets its acceptance
   requirements.
4. Direct download from a future Portal website using the same release assets.

Portal is a command-line program, so a Homebrew formula is the natural package
definition. Homebrew formulas install into the Homebrew prefix and expose
executables through its normal linking behavior. A Mac App Store release is not
recommended for the TUI because sandboxing and store packaging add little value
to a terminal launcher for external applications.

Relevant platform guidance:

- [Apple packaging for direct distribution](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [Apple notarization guidance](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Homebrew formula cookbook](https://docs.brew.sh/Formula-Cookbook)

### 2.3 Linux

#### Primary packages

Publish these first-party artifacts:

- `.deb` packages for Debian and Ubuntu derivatives;
- `.rpm` packages for Fedora, RHEL, Rocky Linux, and openSUSE-compatible flows;
- architecture-specific `.tar.gz` archives as the universal fallback.

Install the executable to `/usr/bin/portal` through native packages. The archive
should contain the binary, README, license, and shell-completion files if Portal
later provides them. It should not modify `PATH` automatically.

Do not make Tailscale or RustDesk hard package dependencies. Their package names,
repositories, and availability differ across distributions, and Portal should
remain installable before those applications are configured. Native packages
may print a short post-install message directing the user to `portal setup`, but
package scripts must not download or install third-party software.

Avoid Flatpak and Snap initially. Their sandboxes complicate launching host
applications such as RustDesk and discovering a host-installed Tailscale CLI.
AppImage is also a poor primary fit for a command expected to be available on
`PATH`.

#### Distribution channels

Use these channels in order:

1. GitHub Releases: canonical DEB, RPM, archives, checksums, and notes.
2. A maintained APT and RPM repository only when the release cadence and user
   base justify the operational cost.
3. Community or upstream repositories such as AUR, Nixpkgs, Debian, and Fedora
   after Portal is stable and maintainers are available.
4. Homebrew on Linux as a secondary cross-platform convenience.
5. `cargo install` as a developer-oriented fallback, not the main end-user path.

## 3. Shared prerequisite model

Installer checks and the `portal setup` command should
share core detection code. Frontends may render the result differently, but they
must not implement their own parsing or platform rules.

Represent each dependency with one of these states:

| State | Meaning | Default action |
|---|---|---|
| Ready | Installed, supported, and usable | Show success; make no changes |
| Setup required | Installed, but login, permissions, or configuration is incomplete | Guide the user through the missing step |
| Unsupported version | Installed version is outside Portal's tested compatibility range | Explain the supported range; offer an official update path |
| Missing | No supported installation was found | Offer download/install assistance |
| Check failed | A safe probe failed or returned an unknown result | Preserve details in memory and provide a non-destructive retry |

Never classify a dependency as missing solely because it is absent from `PATH`.
Search `PATH` and documented platform installation locations. Configured Portal
executable overrides remain authoritative.

Detection must be read-only. It must not launch a GUI unexpectedly, start or
install a service, trigger UAC or `sudo`, change configuration, or contact a
tailnet beyond the normal local Tailscale status query.

## 4. Tailscale detection and onboarding

### 4.1 Detection

Search the configured override, `PATH`, and supported platform locations. At a
minimum, check:

- Windows: the installed Tailscale CLI under `Program Files` and its normal
  registered installation location.
- macOS: the Standalone, App Store, and supported open-source client layouts.
- Linux: `tailscale` on `PATH` and standard package locations.

Once found, invoke the CLI directly with separate arguments. A readiness check
may use `tailscale status --json`, which Portal already parses. Distinguish these
conditions without displaying or persisting raw output:

- executable found and local node available: ready;
- executable found but logged out: setup required;
- daemon or backend stopped: setup required;
- command cannot run or output cannot be parsed: check failed.

Only retain the fields Portal already permits: local machine name, local
Tailscale IP, peer device fields represented by the core model, and concise
sanitized errors. Never write raw status JSON, tailnet DNS suffixes, user IDs,
public keys, auth URLs, or auth tokens to logs or installer reports.

### 4.2 Missing installation

Present the user with three explicit choices:

1. Install Tailscale.
2. Open the official download or instructions page.
3. Continue without it and finish setup later.

Platform behavior:

- Windows: the bootstrapper may download the official architecture-correct MSI,
  verify it, request elevation, and invoke `msiexec.exe` directly. Tailscale
  documents MSI installation and supported installer properties.
- macOS: open the official Standalone installer page, which Tailscale currently
  recommends, or the installed app when present. The user must approve the VPN
  configuration or system extension and authenticate interactively.
- Linux: prefer distribution-specific official packages. `portal setup` may
  identify the distribution and show the official package steps. A later
  implementation may invoke a supported package manager directly after explicit
  approval, but it must not implement `curl | sh` or launch a shell.

Official references:

- [Install Tailscale](https://tailscale.com/docs/install)
- [Tailscale Windows MSI installation](https://tailscale.com/kb/1189/install-windows-msi/)
- [Tailscale macOS installation](https://tailscale.com/docs/install/mac)
- [Tailscale Linux installation](https://tailscale.com/docs/install/linux)

### 4.3 Required user setup

After installation:

1. Launch the official Tailscale client or display its interactive login path.
2. Let the user authenticate with their identity provider.
3. Re-run the read-only status check.
4. Confirm only that Portal can see the local machine identity and usable IP.

Portal must never ask for a reusable Tailscale auth key during consumer setup.
Unattended mode, exit-node settings, subnet routing, SSH enablement, key-expiry
changes, and tailnet policy remain separate, explicit administrative actions.

## 5. RustDesk detection and onboarding

### 5.1 Detection

Search the configured override, `PATH`, documented install locations, and native
application metadata:

- Windows: `Program Files`, registered uninstall information, and configured
  overrides.
- macOS: `/Applications/RustDesk.app` and configured overrides.
- Linux: standard binary locations plus native package metadata where available.

Do not execute an unverified RustDesk argument merely to discover its version;
some GUI applications treat unknown arguments as a normal launch. Prefer Windows
file-version metadata, macOS bundle metadata, Linux package metadata, or a command
whose behavior has been verified for the supported RustDesk release.

Portal should report these concerns independently:

- RustDesk application present;
- version compatible with Portal's verified command syntax;
- receiving service available where the platform supports it;
- required privacy permissions incomplete;
- Portal's direct-IP and tailnet-only receiving configuration applied.

The application may be usable for outgoing Portal connections even when its
receiving service or local permissions are not ready. Do not collapse these
distinct capabilities into one misleading pass/fail result.

### 5.2 Missing installation

Present the same three choices used for Tailscale: install, open official source,
or continue and finish later.

Platform behavior:

- Windows: download an official release installer, verify it, request elevation,
  and run it directly. RustDesk documents `--silent-install`, but Portal's default
  should remain an attended vendor installation unless the user explicitly
  chooses a clearly labeled silent option.
- macOS: open the official DMG or download page. The user must move RustDesk to
  Applications and approve Accessibility, Screen Recording, and, when required,
  Input Monitoring. Portal can explain and recheck these steps but cannot grant
  them.
- Linux: identify the distribution and offer the matching official DEB, RPM,
  package archive, Flatpak, or other vendor-supported route. Prefer native
  packages when they are available because they integrate the application and
  service with the host system.

Official references:

- [RustDesk client installation](https://rustdesk.com/docs/en/client/)
- [RustDesk Windows guidance](https://rustdesk.com/docs/en/client/windows/)
- [RustDesk macOS installation and permissions](https://rustdesk.com/docs/en/client/mac/)
- [Official RustDesk releases](https://github.com/rustdesk/rustdesk/releases)

### 5.3 Portal-specific receiving setup

Installation and Portal provisioning are separate approvals.

After RustDesk is installed, explain that receiving Portal connections may
require additional configuration. On Windows, reuse the existing previewable and
idempotent `portal init <name>` behavior:

1. Inspect the RustDesk service.
2. Print the exact proposed steps.
3. Request explicit confirmation and elevation.
4. Ensure the service is running.
5. Enable direct IP access.
6. Restrict incoming access to the Tailscale CGNAT range.
7. Read the resulting state and report whether verification succeeded.

Do not fold those changes into a generic “Install RustDesk” button. macOS and
Linux provisioning must remain guided-only until their behavior has been tested
on real machines and a platform-specific plan has been approved.

Portal must not accept, generate, persist, or pass a RustDesk password. The
currently available password operation exposes the value in a process argument,
so password setup should stay in the RustDesk UI unless a future secure vendor
interface is verified.

## 6. Download and execution security

Automatic dependency installation introduces a supply-chain and privilege
boundary. Implement it only with all of these safeguards:

1. Download only over HTTPS from an allowlist of official vendor origins.
2. Never scrape a download page or blindly execute the newest release asset.
3. Use a versioned prerequisite manifest containing platform, architecture,
   tested version, immutable URL, SHA-256 digest, expected installer type, and
   allowed argument vector.
4. Ship that manifest with the Portal release and sign it as part of the Portal
   release process. Updating it requires a Portal release or a separately signed
   manifest update with rollback protection.
5. Verify the SHA-256 digest before execution.
6. Verify Authenticode, Developer ID, or native package signatures when the
   vendor provides them. A valid TLS connection alone is insufficient.
7. Download to a newly created private temporary directory and remove it after
   success or cancellation.
8. Invoke installers directly through the process API. Never use a shell,
   interpolate user-controlled strings into commands, or use `curl | sh`.
9. Use a fixed argument allowlist. Do not accept arbitrary installer flags from
   remote metadata or user configuration.
10. Elevate only the small installation step that needs privilege. Discovery,
    downloads, UI, and user configuration should remain unelevated.
11. Never downgrade or replace an existing dependency automatically.
12. Show vendor, product, version, source, and requested privilege before the
    user confirms installation.

Downloading third-party installers at setup time is preferable to bundling them
inside Portal. It keeps Portal artifacts smaller, avoids shipping stale vendor
software, and reduces redistribution and licensing complications. Before public
release, review each vendor's current redistribution, trademark, and installer
license terms even when Portal only downloads from the official source.

## 7. Recommended setup experience

The installer and `portal setup` should present a compact checklist rather than
a long wizard:

```text
Portal setup

✓ Portal 0.0.1             installed
! Tailscale                installed · login required
× RustDesk                 not found

[Set up Tailscale]  [Install RustDesk]  [Finish later]
```

After the user completes an action, re-run only that dependency's checks and
update the same screen. Do not continuously poll or launch multiple installers.

The final result should distinguish “Portal installed” from “all integrations
ready”:

```text
Portal is installed and available on PATH.
Tailscale is connected as <local-machine> (<local-ip>).
RustDesk is installed. Receiving setup is still optional.

Run `portal` to open the TUI.
Run `portal setup` to revisit this checklist.
```

Do not print real tailnet data into installer logs. Local identity may be shown
only in the interactive UI, consistent with the TUI's existing privacy model.

## 8. Release pipeline

For every tagged Portal release:

1. Build Windows x86-64 artifacts; add ARM64 after it can be validated.
2. Build macOS Apple Silicon and Intel artifacts, or one universal artifact.
3. Build Linux x86-64 and ARM64 artifacts using a conservative libc baseline.
4. Run formatting, Clippy, tests, and release builds before packaging.
5. Test install, upgrade, uninstall, and `PATH` behavior in clean virtual machines.
6. Sign Windows binaries and installers.
7. Sign and notarize macOS binaries and packages.
8. Sign Linux repository metadata when first-party repositories exist.
9. Generate checksums and a software bill of materials.
10. Create a draft GitHub Release, attach every expected asset, then publish it.
11. Update WinGet, Homebrew, and other package manifests only after the canonical
    release assets are immutable and publicly downloadable.

[GitHub's release documentation](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)
describes tagged releases and attached binary assets.

## 9. Implementation phases

### Phase A: shared diagnostics and setup

- Add pure platform-specific discovery data and shared readiness states to core.
- Maintain `portal setup` with read-only checks plus explicit package-manager,
  official-link, and skip choices.
- Keep `portal doctor` focused on richer diagnostics if both commands are later
  retained; `setup` is action-oriented, while `doctor` explains problems.
- Add sanitized fixtures and platform-path tests.

### Phase B: Windows release and guided installer

- Produce a signed Inno Setup EXE and portable ZIP.
- Support current-user and all-users install locations and matching `PATH`
  registration.
- Detect Tailscale and RustDesk.
- Open official installation sources and verify the result.
- Reuse the existing Windows `portal init` flow for optional receiving setup.
- Validate on clean Windows 10 and Windows 11 virtual machines.

### Phase C: Windows automatic prerequisite installation

- Add the signed, pinned prerequisite manifest.
- Download, verify, and directly invoke approved vendor installers.
- Test cancellation, offline behavior, UAC denial, partial installation, upgrade,
  architecture mismatch, checksum failure, and cleanup.
- Submit the stable Portal installer to WinGet.

### Phase D: macOS packages and setup guidance

- Produce signed and notarized PKG and archive artifacts.
- Detect both Tailscale client variants and RustDesk.app.
- Guide interactive Tailscale login and macOS privacy approvals.
- Test Intel and Apple Silicon machines before claiming runtime support.
- Publish a Homebrew tap formula.

### Phase E: Linux packages and setup guidance

- Produce and test DEB, RPM, and tarball artifacts.
- Detect supported distributions and package managers without mutating them.
- Provide vendor-specific guided installation.
- Implement automatic package-manager invocation only after tests on each
  supported distribution family.

## 10. Acceptance criteria

The installer work is complete for a platform only when:

- clean install, upgrade, repair, and uninstall are tested;
- `portal` is available from a new terminal without manual `PATH` editing;
- an existing Tailscale or RustDesk installation is preserved;
- missing dependencies are detected outside `PATH` when installed normally;
- declining dependency installation still leaves Portal installed and usable;
- downloads are pinned, hashed, origin-restricted, and signature-checked where
  supported;
- no external command is launched through a shell;
- privilege is requested only after the user approves a described action;
- Tailscale login and macOS privacy prompts remain interactive;
- no credentials, auth URLs, private tailnet data, or raw status output are
  persisted or written to logs;
- failed and cancelled installs leave no partially trusted executable behind;
- Portal rechecks readiness and clearly identifies remaining manual steps; and
- package artifacts are signed, reproducible where practical, and published from
  the same tagged source revision.
