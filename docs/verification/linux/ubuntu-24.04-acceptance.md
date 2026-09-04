# Ubuntu 24.04 amd64 acceptance record

## Candidate

| Field | Value |
|---|---|
| Product version | `1.1.0` (target only; no tag or release asserted) |
| Expected asset | `codex-barbar_1.1.0_amd64.deb` |
| Candidate commit | PENDING — record the exact commit SHA |
| Ubuntu version / architecture | NOT RUN — Ubuntu 24.04 amd64 required |
| Package SHA-256 | PENDING — run `sha256sum codex-barbar_1.1.0_amd64.deb` on the candidate artifact |
| Debian control / staged verifier | NOT RUN — run `bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release` |
| Windows CI for exact commit | PENDING — link run and result |
| Ubuntu CI for exact commit | PENDING — link run and result |

## Desktop-session matrix

| Session | Tray/menu/panel | Settings + Current CLI refresh | Float-ball movement/rotation | Notification capability/test | XDG autostart create/remove | Secret Service round-trip/no plaintext | No taskbar helper window | Evidence | Status |
|---|---|---|---|---|---|---|---|---|---|
| GNOME Wayland | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | PENDING | PENDING |
| GNOME X11 (when available) | NOT RUN — availability must be recorded | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | PENDING | PENDING |
| KDE Wayland or X11 (best effort, when available) | NOT RUN — availability must be recorded | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | NOT RUN | PENDING | PENDING |

## Test procedure and evidence fields

Run for every available row above. Capture the session type, desktop version,
package path, terminal output, screenshots, and any observed panel/compositor
caveat. A missing GNOME/KDE/X11 session is `NOT RUN`, never a pass.

```bash
sha256sum codex-barbar_1.1.0_amd64.deb
sudo apt install ./codex-barbar_1.1.0_amd64.deb
codex-barbar
```

1. Confirm the tray icon/menu and usage panel; open Settings and refresh a
   signed-in Current CLI profile.
2. Enable the float ball, drag it, and observe the activity rotation. On
   Wayland record the normal-window fallback and any compositor restriction.
3. Read the notification capability in Settings and send a test notification;
   record whether the desktop/app/global policy made it unavailable.
4. Enable then disable start-at-login. Record both results for the exact file:

   ```bash
   test -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/com.naipi11.codexbarbar.desktop"
   ```

5. Complete a managed-credential Secret Service round-trip over the active
   D-Bus session. Inspect the local vault only for the marker
   `codex-barbar-secret-service:v1:<profile-uuid>`; never copy credential
   content into this record. Lock/unavailable Secret Service must fail closed,
   with no plaintext fallback.
6. Confirm there is no Linux taskbar status or measurement helper window;
   that absence is the expected Linux behavior.
7. Quit cleanly and attach the evidence paths. Record `PASS` only for an
   observed result; retain `PENDING` or `NOT RUN` otherwise.

## Current status

**PENDING / NOT RUN.** This record was created on a Windows host that cannot
run Ubuntu GNOME/Wayland/D-Bus/Secret Service/`dpkg-deb`. No package hash,
desktop result, CI result, tag, or release is claimed here.
