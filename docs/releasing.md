# Releasing Portal

## Release model

GitHub Releases is the canonical source for Portal binaries and installers. A
tag-triggered workflow builds artifacts on native GitHub runners and creates a
draft release. Publishing remains a deliberate manual step after the draft has
been tested.

## One-time repository setup

1. Enable immutable releases under repository settings.
2. Protect `main` and require the Check workflow.
3. Review GitHub Actions permissions and allow the release workflow to write
   repository contents and artifact attestations.
4. Configure Windows signing and Apple Developer ID/notarization credentials
   before calling the installers production-ready.
5. Add `.github/release.yml` later if generated release-note categories need
   customization.
6. Confirm the release remains non-commercial under Inno Setup's current terms,
   or purchase the appropriate Inno Setup commercial license before production
   compilation. CI use counts as compiler use under those terms.

### Optional signing secrets

Windows Authenticode signing uses:

- `WINDOWS_CERTIFICATE_PFX`: base64-encoded code-signing PFX;
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX password.

macOS Developer ID signing and notarization use:

- `MACOS_CERTIFICATE_P12`: base64-encoded P12 containing the Developer ID
  Application and Installer identities;
- `MACOS_CERTIFICATE_PASSWORD`;
- `MACOS_KEYCHAIN_PASSWORD`: an ephemeral CI keychain password;
- `MACOS_APPLICATION_IDENTITY`;
- `MACOS_INSTALLER_IDENTITY`;
- `APPLE_ID`;
- `APPLE_TEAM_ID`; and
- `APPLE_APP_PASSWORD`: an app-specific password.

When these secrets are absent, a manually dispatched workflow still produces
unsigned artifacts for packaging tests. Do not publish those artifacts as a
stable consumer release.

## Prepare a release

1. Update the workspace version in `Cargo.toml`.
2. Run the complete validation suite:

   ```text
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-targets --locked
   cargo build --workspace --release --locked
   ```

3. Build and smoke-test the Windows installer locally:

   ```powershell
   ./packaging/windows/build-installer.ps1
   ./packaging/windows/test-installer.ps1 -TestPathRegistration
   ```

4. Run the interactive Windows installer and verify:

   - neutral dark styling remains legible at normal and high DPI;
   - the scope page offers current-user and all-users installation;
   - current-user setup defaults to `%LOCALAPPDATA%\Programs\Portal` without
     elevation;
   - all-users setup requests elevation and defaults to
     `C:\Program Files\Portal`;
   - a custom installation path works;
   - selected PATH registration uses the user or machine scope appropriately
     and works in a new terminal;
   - Start menu creation follows the selected option;
   - only missing Tailscale or RustDesk download links are offered;
   - upgrade and uninstall preserve unrelated files and PATH entries.

5. Commit the version and release notes.
6. Create and push an annotated tag:

   ```text
   git tag -a v0.0.1 -m "Portal v0.0.1"
   git push origin v0.0.1
   ```

## Validate the draft

The Release workflow creates a draft containing:

- Windows setup EXE and portable ZIP;
- macOS Apple Silicon and Intel PKGs and archives;
- Linux x86-64 and ARM64 DEBs, RPMs, and archives;
- `SHA256SUMS`; and
- GitHub artifact attestations.

Before publishing:

1. Verify every expected asset is present and named correctly.
2. Verify checksums and attestations.
3. Test Windows install, upgrade, custom path, PATH, launch, and uninstall in a
   clean Windows 10 or Windows 11 VM.
4. Test macOS packages on Apple Silicon and Intel or Rosetta, including
   Gatekeeper, Tailscale login, RustDesk permissions, and actual launch actions.
5. Test the DEB and RPM in clean supported distributions.
6. Confirm the release notes clearly mark any unsigned or runtime-unvalidated
   package as preview-only.
7. Publish the draft. With release immutability enabled, published assets and
   their tag cannot be replaced; corrections require a new version.

## Third-party prerequisites

Portal v0.0.1 does not redistribute or silently install Tailscale or RustDesk.
The Windows installer detects normal installations and offers official download
pages only for missing products.

A later prerequisite bootstrapper may install them only after all of these are
implemented:

- versioned architecture-specific URLs from official origins;
- pinned SHA-256 hashes;
- Authenticode or native signature verification;
- explicit user consent for each product and requested elevation;
- fixed, directly invoked installer arguments without a shell;
- vendor license and redistribution review;
- cancellation, offline, partial-install, and rollback tests; and
- post-install readiness checks that do not retain private output.

Do not scrape “latest release” pages or execute unverified downloads during
setup.
