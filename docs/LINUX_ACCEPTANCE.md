# Ubuntu 24.04 amd64 acceptance

This checklist is the release evidence boundary for the Debian target. It is
not a statement that Ubuntu support has been accepted. Unit tests, package
metadata, and a Windows host cannot prove GNOME/KDE panels, Wayland/X11
windowing, D-Bus notifications, or Secret Service behavior.

The current target is version `1.1.0`, whose expected package name is
`codex-barbar_1.1.0_amd64.deb`. Release publication requires all of the
following for the exact candidate commit:

- Green Windows CI and green Ubuntu 24.04 CI.
- A verified Debian artifact and its recorded SHA-256.
- A completed [Ubuntu acceptance record](verification/linux/ubuntu-24.04-acceptance.md)
  with no release-blocking `PENDING` or `NOT RUN` item.
- Explicit authorization to create/publish a release. This document does not
  authorize a tag or release.

## Package and runtime contract

Install the exact candidate through APT:

```bash
sudo apt install ./codex-barbar_1.1.0_amd64.deb
codex-barbar
```

The Debian package declares the runtime dependencies `libwebkit2gtk-4.1-0`,
`libgtk-3-0`, `libayatana-appindicator3-1`, and `libsecret-1-0`. WebKitGTK,
GTK, and Ayatana AppIndicator are needed for the desktop shell and tray.
GNOME is the primary target. KDE is best effort because panel/tray behavior
depends on the installed AppIndicator integration.

## Platform behavior to accept

- **Tray and panel:** the tray icon, menu, and usage panel must be observed on
  GNOME and, when available, KDE. Missing or hidden AppIndicator support is a
  desktop-environment compatibility failure, not a silent pass.
- **Settings and Current CLI refresh:** Settings must open and a signed-in
  Current CLI profile must refresh without exposing credentials.
- **Float ball:** observe movement and activity rotation. On Wayland it is a
  normal draggable window; the compositor may restrict placement or
  always-on-top behavior. Record the actual fallback instead of claiming
  Windows-style taskbar behavior.
- **Taskbar status:** unsupported on Linux by design. There must be no Linux
  taskbar status or measurement helper window.
- **Notifications:** record the capability reported by the app and the result
  of a real test notification. Desktop/global notification policy can make a
  supported transport unavailable.
- **XDG autostart:** enabling start-at-login creates, and disabling it removes,
  `${XDG_CONFIG_HOME:-$HOME/.config}/autostart/com.naipi11.codexbarbar.desktop`.
- **Secret Service:** managed credentials must round-trip through the logged-in
  user's Secret Service over D-Bus. The vault file may contain only the
  `codex-barbar-secret-service:v1:<profile-uuid>` marker, never credential
  plaintext. A locked or unavailable service must fail closed; it must not
  create a plaintext fallback.

## Required sessions

Run GNOME Wayland and GNOME X11 where available. Also record a KDE session
when available. If a session is unavailable, write `NOT RUN` with the reason
and keep the release gate pending rather than inferring a result from another
desktop/session.

## Evidence commands

Use these commands and attach their output paths, screenshots, and observed
results to the record. Do not substitute a Windows, WSL, or CI build for this
desktop evidence.

```bash
sha256sum codex-barbar_1.1.0_amd64.deb
dpkg-deb --info codex-barbar_1.1.0_amd64.deb
sudo apt install ./codex-barbar_1.1.0_amd64.deb
codex-barbar
test -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/com.naipi11.codexbarbar.desktop"
```

After collecting desktop evidence, run the release artifact verifier against
the staged assets:

```bash
bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release
```

The Windows host that created this document cannot run Ubuntu GNOME, Wayland,
D-Bus, Secret Service, or `dpkg-deb`. Its package hash and all desktop rows
therefore remain `PENDING`/`NOT RUN` in the acceptance record.
