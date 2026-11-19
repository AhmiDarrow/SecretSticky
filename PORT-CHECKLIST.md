# SecretSticky → RemedyUI port checklist

The shipped app is the Tauri + React 0.1.8 release (`src/`, `src-tauri/`). The RemedyUI build
is `remedyui/`. It runs the shipped vault, crypto and error code unchanged (`#[path]` modules
from `src-tauri/src`). Each line below was checked against the shipped source. The proof is
either a headless test in `remedyui/src/flow_tests.rs` (driven through the accessibility tree)
or the screenshots in `port-evidence/` (`before/` is the shipped UI, `after/` is RemedyUI).

## Data (never lose saved notes)

- [x] Same vault file, same place: `%APPDATA%\SecretSticky\vault.json` (`Vault::open_default`, unchanged code).
- [x] Same format. The vault code is the shipped `src-tauri/src/vault.rs`, so there is no migration.
- [x] `SECRETSTICKY_VAULT` selects another vault file. It also gets its own single-instance lock (`main.rs`).
- [x] Vaults written by 0.1.0 and 0.1.8 open, can be edited, and read back without loss through the shipped code (`shipped_vaults_open_edit_and_reopen_without_loss`; fixtures from `compat/shipped.sh`, which runs the release tags' own vault code).
- [x] `.bak` and crash recovery come from the shipped `open_path`, unchanged.
- [x] Untouched notes keep their saved geometry. Opening a note does not count as moving it.

## Manager window

- [x] 440×560, minimum 400×500, persisted geometry, hidden from screen capture.
- [x] The close button hides the window to the tray. The app quits only from Quit.
- [x] Header: app mark, "SecretSticky", "Encrypted sticky notes".
- [x] Header buttons: About, Password, Auto-lock (amber while auto-lock is on, with a tooltip), Open all, Lock. A button closes its panel when pressed again (`every_manager_action`).
- [x] About panel: greeting, description, version, GitHub profile / Project repo / Releases links (the allowlisted URLs), Check for updates, install and restart, "Local vault · no cloud", Close.
- [x] Password panel: current, new and confirm fields; at least 12 characters; mismatch error; "Updating…"; success message "Password updated. Recovery key is unchanged."; Cancel.
- [x] Auto-lock panel: the 10 presets Off, 1 min, 5 min, 15 min, 30 min, 1 hr, 2 hr, 4 hr, 8 hr, 12 hr; a "Now: …" badge; help text; Done. The value is clamped to 12 h.
- [x] New-note row: 8 color swatches named "New <color> note".
- [x] Note list: color dot, title or "Untitled note", "<color> · open to edit", delete button. Opening a note hides the manager.
- [x] Empty state: "No notes yet — pick a color to create one."
- [x] Delete confirmation: "Delete note?" with Cancel (the default) and Delete. Deleting also closes the note's window.
- [x] Footer: "N note(s) · local vault", the two hints, Hide to tray, Quit. Quit saves open notes and clears the clipboard.
- [x] Errors show inline.

## Gate (setup / unlock / recovery)

- [x] Setup: master password and confirm; "Use at least 12 characters"; "Passwords do not match"; "Working…" while Argon2 runs (off the UI thread).
- [x] Recovery key screen, shown once: "Copy key (2 min) — continue" (secret clipboard, cleared after 2 min) and "I already saved it — continue".
- [x] Unlock: master password; "Use recovery key" / "Use master password" toggle (only when the vault has a recovery key). The recovery key is shown as typed.
- [x] Unlock throttle: every failed unlock counts (shipped `UnlockThrottle`).
- [x] "Not now — hide to tray" / "Hide to tray", and Quit.
- [x] Fine print: "Local only · Argon2id + XChaCha20-Poly1305 · no cloud".
- [x] A vault that cannot be opened gets its own screen with the reason and Quit.

## Note windows

- [x] Frameless, hidden from capture, the note's color, saved size and place, minimum 345×250, maximum 900.
- [x] New notes cascade from the default origin (same formula as the Tauri app).
- [x] Pinned on top only when pinned. New notes are not pinned.
- [x] Title bar: 8 color swatches with the current one ringed, New note (same color), Open manager (the shipped house, the engine's `Icon::Home`), Lock vault, a saved/saving indicator, Copy, Pin/Unpin, Close. Dragging the bar moves the window.
- [x] Title field and body ("Secrets, API keys, passwords…"), saved 400 ms after typing stops. The window title follows the note title ("Sticky note" when blank).
- [x] Copy copies the selection or the whole body through the secret clipboard (kept out of history, cleared after 30 s), with a "· clears in 30s" status. "Nothing to copy" when empty.
- [x] Ctrl+C and Ctrl+X in a note also go through the secret clipboard (the shipped app intercepted copy), with the "Copied · clears in 30s" status. The title and text fields use the engine's `TextField::copy_as_secret`, not an app-side key handler.
- [x] Moves and resizes are saved. A corner grip resizes the window (keyboard: the system window menu).
- [x] Close saves and closes. Lock saves every note, closes them all, clears the clipboard and shows the manager.

## App-wide

- [x] Tray icon (the shipped 32 px icon) with tooltip "SecretSticky — encrypted sticky notes". Menu: New note, Show manager, Open all notes, Lock vault, Quit. When the vault is locked, New and Open all show the manager instead. Clicking the icon shows the manager.
- [x] Single instance: a second launch brings the manager forward.
- [x] Idle lock after the vault's auto-lock time (real input only), plus the vault's own idle check.
- [x] Signed updates from GitHub releases, using the same minisign key as `tauri.conf.json`, with download progress.
- [x] Brand: the shipped palette (`#1a1b1e` / `#24262b` / amber `#fbbf24` / `#f87171`) as a RemedyUI theme (`remedyui/src/theme.rs`) and the shipped app mark and icons.
- [x] Type: Inter, as shipped: the same Fontsource 5.3.0 WOFF2 files (weights 400 to 800, all seven script subsets) in `remedyui/assets/fonts/inter/`, loaded by the engine and first in the theme's families (`remedyui/src/fonts.rs`).
- [x] Window icon: the manager and every note window show the shipped app icon (`src-tauri/icons/128x128@2x.png`) in the taskbar and Alt+Tab, as the Tauri windows did (`WindowSpec::icon`).
- [x] Screens and panels that are not shown take no room (the engine's `show`; no app-side hiding).
- [x] Accessibility: every control is named and reachable, and the engine's audit is clean on every manager screen and on note windows (focus rings, 24 px targets, no clipping).
- [x] Keyboard: Enter submits each form, Tab order, Escape closes dialogs.

## Not carried over (with reason)

- Boot "Loading SecretSticky…" screen: the native app opens the vault before the first frame, so there is no loading phase.
- The "Vault locked / Unlock" placeholder in note windows: both builds close note windows on lock, so the placeholder could only flash for a moment in the webview build.
- Localization: the shipped app is English only. The port keeps English.

## Targets

- [x] Windows: `cargo build --release`, and the built app was run against a temporary vault.
- [x] Linux (WSL Ubuntu 24.04): `cargo test --release` passes (73 tests, including the shipped-vault round trip), run headless.
