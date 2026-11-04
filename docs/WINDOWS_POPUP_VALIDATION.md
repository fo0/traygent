# Windows tray popup

The main window opens beside the clicked notification icon. The click's physical
rectangle is authoritative, including icons in Explorer's overflow flyout. Menu
and second-instance activation query the current icon rectangle. Placement uses
that monitor's work area and DPI; the previous desktop position is not reused.
All four taskbar edges, negative monitor coordinates, auto-hide and windows larger
than the work area are handled. A temporary size cap retains the preferred logical
size for the next monitor. Manual startup positions the hidden window before
showing it; [Windows login startup](WINDOWS_AUTOSTART.md) keeps it hidden.

Outside focus loss hides the existing webview after 120 ms. A generation cancels
stale dismissals after a tray gesture, focus return, explicit hide or reopen.
Holding the mouse on the icon does not race the delayed blur. A double-click
toggles once. Native owned dialogs and WebView children defer dismissal; the check
continues until focus returns or moves outside. HTML dialogs, unsaved fields and
running operations survive hiding because the webview is not destroyed. Opening
a terminal or Explorer window dismisses the panel without cancelling other work;
work that intentionally returns to the panel uses `tray_popup::show`.

## Automated checks

- `cargo test --locked --manifest-path src-tauri/Cargo.toml --features updates`
  covers placement on all edges, negative coordinates, integer/fractional DPI,
  overflow/auto-hide, constrained work areas, stale blur and click sequences.
- On Windows, `cargo test --locked --manifest-path src-tauri/Cargo.toml --features desktop --bin traygent`
  creates real Win32 windows to test owner/child relationships and repeated frame
  placement against the executor's actual monitor work area. Checks CI runs this
  after the full desktop compile.
- Frontend verification remains `pnpm verify`; browser tests are UI regressions,
  not evidence of Windows tray behavior.

The Win32 tests do **not** click Explorer's notification area, run WebView2 login,
or change monitor topology/DPI. A Linux Windows-target compile is compilation
evidence only. No interactive Windows validation is claimed by those checks.

## Interactive Windows acceptance

Use an authorized test Windows desktop and the branch build; no access to a user's
PC is needed. Keep the published installer/release unchanged.

1. Start the app, then open/hide it repeatedly with the tray icon, its context
   menu and a second launch. No first-frame desktop flash or position drift.
2. Test every supported taskbar edge, auto-hide and overflow. Click another
   application: hide. Hold the tray press for more than 120 ms, then release;
   double-click and rapidly reopen: no hide/reopen race.
3. Use two monitors (including one left/above the primary) with 100%, 125%, 150%
   and 200% scaling. Move the taskbar/primary display and reopen. The window stays
   beside the clicked icon, fits the work area and preserves logical sizing.
4. Resize the panel, visit a smaller work area and return to the larger monitor.
   Confirm that the preferred size returns and controls remain reachable.
5. Edit an unsaved form, use selects, keyboard navigation and an owned native
   dialog. Internal interactions must not dismiss the panel. Click outside,
   reopen and confirm the draft remains. Start an update check in Settings; hiding must not cancel it.
6. Disconnect/reconnect a display or restart Explorer, then reopen from the
   recreated tray icon. Confirm placement uses the new icon/monitor geometry.
7. Drag the panel by its header, then double-click the header: the panel moves,
   never maximizes, and the next tray open places it beside the icon again.

The remaining native-only checks (snapshot events under the narrowed
capabilities, in-app update with progress and restart, WebView2 clipboard,
High Contrast and autostart after uninstall) are listed in the
[native Windows smoke checklist](VALIDATION.md#native-windows-smoke-checklist).

The lifecycle and generation approach was compared read-only with
`fo0/OpenQuota` at `0b21b354e1a0a3f65d78900ec244c43f581542e2`
(`window.rs`, `popup.rs`, and the tray event wiring). Traygent's Win32 placement
and tests are its own; OpenQuota was not modified.
