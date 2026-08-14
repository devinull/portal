# Security policy

## Supported versions

Until Portal reaches a stable release, only the latest published version is
supported with security fixes.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting feature for this repository.
Do not open a public issue containing credentials, private tailnet data, auth
URLs, device identifiers, or raw Tailscale/RustDesk command output.

Include a concise description, affected version, reproduction steps using
sanitized data, and the expected impact. Reports will be acknowledged as soon as
practical.

## Scope and trust boundaries

Portal invokes locally installed Tailscale and RustDesk executables directly.
It does not authenticate users, store credentials, or provide its own network
transport. Installers published by Portal must not silently redistribute or
execute third-party software.
