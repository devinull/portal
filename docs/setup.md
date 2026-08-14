# Portal setup

## 1. Install Portal

### Windows

Run the Portal setup EXE and choose whether Portal should be available to the
current user or every user on the computer. The defaults are:

```text
Current user: %LOCALAPPDATA%\Programs\Portal
All users:    C:\Program Files\Portal
```

An all-users installation requires administrator approval. Keep **Add Portal to
PATH** selected unless you plan to launch the EXE by its full path. Portal
updates the user `PATH` for a current-user installation and the machine `PATH`
for an all-users installation. A custom destination can be selected from the
installer's destination page. Open a new terminal after setup so it receives
the updated environment.

### macOS

Run the package for your processor. It installs `/usr/local/bin/portal`.
Tailscale and RustDesk still require their normal interactive macOS setup and
privacy approvals.

### Linux

Install the package matching the distribution:

```text
sudo apt install ./portal_<version>_<architecture>.deb
sudo rpm -i ./portal-<version>-1.<architecture>.rpm
```

The packages install `/usr/bin/portal`.

## 2. Install and sign in to Tailscale

Install Tailscale from its official source, authenticate it, and confirm the
local client can see the expected peers:

```text
tailscale status
```

Portal excludes iOS and Android peers because its actions require a desktop
RustDesk target.

## 3. Install and configure RustDesk

Install RustDesk on the controller and every target. Enable direct IP access on
targets. Windows targets can use Portal's preview-first provisioning flow:

```text
portal --dry-run init <peer-name>
portal init <peer-name>
```

Review the displayed changes before approving them. Portal does not configure or
store a RustDesk password.

## 4. Start Portal

```text
portal
```

New peers begin in the checking state. Successful Tailscale probes fill their
device dot and add current latency. Press `r` to refresh discovery and make all
peer checks due immediately.

## Troubleshooting

- If `portal` is not found after Windows setup, open a new terminal and confirm
  the selected installation directory appears in the user `PATH` for a
  current-user install or the machine `PATH` for an all-users install.
- If discovery fails, confirm `tailscale status` works for the same user.
- If a device is reachable but RustDesk cannot connect, confirm RustDesk is
  installed on both devices and direct IP access is enabled on the target.
- Use `portal --dry-run connect <name>` to inspect the exact safe argument vector
  without opening RustDesk.
