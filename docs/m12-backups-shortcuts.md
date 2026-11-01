# M12: timed pauses after restart, quick tagging, shortcuts and backups

## Timed pauses after a restart
The end of a timed pause is stored in `pause.json` in the data folder. After a
crash, update or reboot the pause continues until its original end; an expired
pause is dropped and recording starts. Resume now, expiry and untimed API pauses
remove the file.

## Tag the last N minutes
The tag editor for a new range has 5/15/30/60 minute buttons and a minutes field
(1–1440) that moves the start relative to the end. The tray has
**Tag last N minutes…**, which opens the editor with the field focused.

## Keyboard shortcuts
Press **F1** or **?** for the list. Added: **Ctrl/⌘+1…5** (Day, Week, Tags,
Rules, Settings), **P** (pause for the last chosen preset, or resume), **N** (tag
last N minutes). Shortcuts act only when no text field or editor has focus. They
work inside the window only.

## Backups
Settings → Data & import → Backups: **Back up now**, automatic backups (daily or
weekly, keep 3/7/14/30 automatic copies) and a list of the latest backups.
**Restore…** checks the file and replaces history on the next launch; the
current history is kept as a "Before restore" backup. A staged restore can be
cancelled until then.

## Verification
Automated: daemon restart tests (`daemon/tests/tracking.rs`), backup and restore
tests (`daemon/tests/backup.rs`, `core/src/backup.rs`), editor, shortcut and
settings tests in the app. Screenshots of the Data section:
`docs/screenshots/m12-settings-data-*`.

Manual checks on Windows, macOS and Linux X11/Wayland:
- Pause for 30 minutes, quit from the tray, start again: the status pill and tray
  show the remaining time; recording resumes at the original end.
- Tray → Tag last N minutes…, type 40, Enter: the range covers the last 40 minutes.
- Shortcuts with a non-US keyboard layout (`?` and digits), and Cmd on macOS.
- Back up now on a large history while recording; open the backups folder.
  Restore a backup, restart, check the Day view and the "Before restore" copy.
  On Windows, check that antivirus or another process holding the database
  postpones the restore (Settings → Data & import still shows it waiting)
  rather than losing history.
