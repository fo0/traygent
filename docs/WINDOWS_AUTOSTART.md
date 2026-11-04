# Windows login and tray startup

In **Einstellungen → App → Mit Windows starten**, explicitly enable login startup
for the current Windows user and press **Speichern**; **Abbrechen** drops the
change. A fresh installation does not register anything.
After the next **user login**, Traygent creates its tray icon and keeps the main
window hidden. This is not a service and does not run before login. Windows may
delay startup. There is no separate “start minimized” preference: login launches
always start in the tray, while manual launches open the existing popup.

Closing or hiding the popup preserves the webview and ongoing work. The tray
icon/menu can reopen it; **Traygent beenden** exits until the next manual launch
or login. A second manual launch opens the existing instance. A second launch
with `--autostart` does not show, hide or focus it.

## Registration and Windows control

The native commands use the documented Windows per-user `Run` mechanism:

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Run
  io.github.fo0.traygent = "<absolute current executable>" --autostart
```

There is no stored preference copy and no registration during app
setup, rendering, status reads or upgrades. Opening the App tab, returning focus
to the window, or **Status prüfen** (shown while the state is unknown, blocked or
in conflict) reads the OS state. The switch only stages a choice; **Speichern**
writes it and reads it back before the dialog closes, and a failed write keeps
the dialog open with its error. Failed reads/writes leave the UI explicitly
unknown (**Status unbekannt.**) until another read succeeds.

Only the dedicated value whose **complete command matches this executable and
argument** can be removed. Other values, whole keys, machine-wide startup entries,
scheduled tasks and services are never changed. A different command under the
reserved name is shown as a conflict and cannot be overwritten or deleted.
Paths are quoted; invalid or overlong Run command lines are rejected. No admin
privilege or elevated helper is used.

Windows can retain a separate startup veto in
`HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`.
Known enabled/disabled formats are read conservatively; an unexpected type or
format is displayed as unknown. **This Windows-owned format is not a public API
contract**. Traygent never writes/deletes it, and cannot guarantee detection of
every future Windows format or enterprise startup policy. A visible Run entry
is not a guarantee that Windows will execute it.

When Windows blocks the entry, the switch is locked and one line names the
reason. **Windows-Autostart öffnen** opens the fixed Windows Settings page, where
the user can grant permission again. **Autostarteintrag entfernen** can still
remove Traygent's own Run value. Both act at once, without **Speichern**. Windows may retain its veto even after removal, so
re-enabling the app may also require re-enabling it in Windows Settings.

The official [Tauri autostart plugin](https://v2.tauri.app/plugin/autostart/) was
evaluated. The explicit adapter keeps the ownership guard and current-user-only
writes under application control and leaves Windows' veto untouched. The
existing official [Single Instance plugin](https://v2.tauri.app/plugin/single-instance/)
remains first in the plugin list. Microsoft's
[Run key documentation](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)
defines login timing and command-line limits; its
[startup app guidance](https://learn.microsoft.com/en-us/windows/compatibility/startup-apps)
explains user control through Task Manager.

## Installation, updates and removal

Enable startup from the installed Windows executable at a stable location, not
from `desktop:dev` or a temporary build directory. The setting registers the
currently running path. In-place NSIS upgrades keep that path and registration;
neither unsigned public installers nor signed-updater configuration opt users in.
The standard in-app update runs the installer with `/P /R /UPDATE` and no `/ARGS`,
so the installer relaunches Traygent without `--autostart`: after an update the
panel opens, even when the app had started from login. Signed builds using the
Tauri updater pass the launch arguments on (`/ARGS`) and restart hidden after an
`--autostart` launch. The next login launch starts in the tray again.

Moving the executable or changing installation directory requires disabling
startup from the old installation, then enabling it from the new one. The new
installation treats an old-path value as a conflict, even if the filename is
the same. **Disable startup before uninstalling**; the installer has no NSIS
uninstall hook for it. If the old executable is already gone, remove only that
identified Traygent Run value yourself in an authorized Windows session. Do not
delete the Run key or unrelated entries. No claim is made about which version a
particular user's computer currently has installed.

## Verification and remaining acceptance

- Core Rust tests cover default off, exact launch flags, quoting/Unicode/length,
  idempotent registration/removal, OS readback, failures, Windows vetoes and
  conflicting/relocated commands.
- Browser tests use a **native IPC mock**, without network access or Windows
  changes (`tests/startup.spec.ts`). They cover the staged switch (**Speichern**
  writes once and reads back, **Abbrechen** writes nothing), reopen/focus refresh,
  read/write failures, conflicts, unknown approval and the 360px layout. Review screenshots
  are captures of this mock UI, not Windows login evidence.
- Checks CI compiles the Windows desktop shell and runs a real registry roundtrip
  against a unique **test subkey**. It verifies adjacent values and Windows veto
  data survive removal. It never writes the actual login Run key. The same job
  retains native popup/window-ownership tests. No installer build is needed for
  these changes; normal CI continues to skip packaging.

Interactive acceptance remains on an authorized Windows test desktop: enable
and inspect the exact user Run value; log out/in and confirm no first-frame
window flash; open/close/hide via tray and a second manual launch; launch a second
`--autostart` instance with the panel both open and hidden; disable/re-enable in
Task Manager and verify refresh; remove in the app and confirm the Run value is
gone; test an in-place upgrade and stable-path restart. Also confirm the tray
icon after Explorer restart and check any enterprise startup restrictions.
Mocks, Linux builds and isolated Windows registry tests do not establish these
interactive login/installer outcomes. No setting on a user's PC is changed by
this implementation or its automated checks.
