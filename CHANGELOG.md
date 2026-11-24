# Changelog

All notable changes to SecretSticky are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.5] — 2026-11-24

### Added

- **Find in a note**: Ctrl+F opens a find bar in the note. Every match is marked in the text; Enter or F3 goes to the next, Shift+Enter or Shift+F3 to the one before, Escape closes the bar and leaves the match selected. Selected words are searched for when the bar opens.
- **Search in the manager**: Ctrl+F (or a click in **Search notes**) finds notes by their title or their text. Enter opens the first one found, Escape shows every note again.
- **Shortcuts**. In a note: Ctrl+N new note of the same color, Ctrl+W close the note, Ctrl+S save now, Ctrl+L lock the vault. In the manager: Ctrl+N new note, Ctrl+L lock, Ctrl+W hide to the tray, Ctrl+Q quit.

### Fixed

- **Selecting text in a note shows.** On the light notes a selection was a dark block over dark text, so the selected words could not be read, and the caret was drawn in a light color that did not show. The selection is now a soft tint of the note's ink and the caret is the color of the text, on every note color. The gray "Title" hint is readable on every color too.
- **Up, Down, Page Up and Page Down move the caret** through a note's lines. They did nothing before.
- **Home and End go to the start and end of the line**, and Ctrl+Home and Ctrl+End to the start and end of the note. Home and End used to jump to the ends of the whole note. Shift with any of these selects.
- **Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+A, Ctrl+Z and Ctrl+Y work with a Russian keyboard layout** (and any other layout without Latin letters). They did nothing while such a layout was active.
- **Shift+Delete cuts, Ctrl+Insert copies and Shift+Insert pastes**, as elsewhere on Windows and Linux.

## [1.0.4] — 2026-11-22

### Changed

- **About** says SecretSticky is for Windows and Linux and built with RemedyUI, in every language.
- **Language** says SecretSticky follows the system's language on first launch (it said Windows), in every language.

## [1.0.3] — 2026-11-22

### Added

- Text size in **Language**: 90%, 100%, 115%, 130% or 150%, for every window at once, saved with your language and font. Your notes are not changed.
- Linux: SecretSticky for Linux (x86-64), first release. Runs on distributions with glibc 2.28 or newer (Debian 10, RHEL 8, Ubuntu 20.04 and later). Install with `tar -xzf SecretSticky-1.0.3-linux-x86_64.tar.gz --strip-components=1 -C ~/.local` (the program, a menu entry and the icon). Notes are kept in `~/.local/share/SecretSticky`. Later updates install from **About → Check for updates**.

## [1.0.2] — 2026-11-22

### Fixed

- The X on a note closes it again. In 1.0.1 the note stayed on screen, frozen; your text was saved, but the window would not go away.

## [1.0.1] — 2026-11-22

### Fixed

- Long notes scroll again, with the mouse wheel and the touchpad; the text stays inside the note.
- Middle-click autoscroll is back: click the middle button over a long note (or the note list) and move the mouse to scroll; click again or press Escape to stop.
- Note windows have rounded corners and the Windows shadow again.
- The pin button shows whether the note is pinned: a filled pin when it stays on top, a faded one when it does not, and its tooltip says Pin or Unpin.

### Added

- Font choice in **Language**: Inter (as before), Remedy Sans, Remedy Sans UI, Ahmi Print or Ahmi Pixel, for every window at once. It is saved with your language in `settings.json`; your notes are not changed.

## [1.0.0] — 2026-11-22

SecretSticky 1.0 has a new interface built on RemedyUI. Notes, the vault, the master password and the recovery key are the same.

### Changed

- New interface on RemedyUI, in all eleven languages, with the same windows, tray menu, auto-lock and secret copy as before.
- Polished: the rounded app mark, heading sizes, dark inset password fields, and a header that wraps long translations.

### Install and update

- Already installed (0.1.2 or later): **About → Check for updates**. The update closes the old app, replaces it, and keeps your desktop shortcut. Notes stay in `%APPDATA%\SecretSticky`; the update never touches them.
- Installed from the MSI (for all users): the update removes that copy first, so Windows asks for administrator rights once.
- New install: `SecretSticky_1.0.0_x64-setup.exe`, a per-user install with no administrator rights needed. 0.1.0 and 0.1.1 have no updater: run this setup and it takes over the old install the same way.

## [0.2.0] — 2026-11-15

Now available in Spanish, French, German, Portuguese (Brazil), Italian, Russian, Polish, Japanese, Korean and Chinese (Simplified). Apps use your system language by default; change it any time in Settings → Language.

Hi I'm Ahmi, hope this helps!

### Added

- SecretSticky follows the Windows language the first time it opens, and falls back to English when that language is not included. A language picker on the unlock screen, the recovery screen, and in the manager switches immediately. The choice is stored in `settings.json` next to the vault, so notes, the master password, and auto-lock are left alone.
- Interface languages: Spanish, French, German, Portuguese (Brazil), Italian, Russian, Polish, Japanese, Korean, and Chinese (Simplified). English stays the source text.

### Install and update

- Windows setup: `SecretSticky_0.2.0_x64-setup.exe` (Start Menu, desktop shortcut, and taskbar icon). `SecretSticky_0.2.0_x64_en-US.msi` is attached as well.
- Already installed: **About → Check for updates**. The update replaces the application only.
- Notes stay in `%APPDATA%\SecretSticky` and unlock with the same master password and recovery key. Existing vaults read and write as before.

Same text: [docs/RELEASE-0.2.0.md](docs/RELEASE-0.2.0.md).

## [0.1.8] — 2026-11-14

Auto-lock closes your notes, and locking saves the last keystrokes before the notes close.

Hi I'm Ahmi, hope this helps!

### Fixes

- **Auto-lock closes open notes.** If the vault locks during a save or another vault action, SecretSticky still closes the stickies and shows the unlock screen. Those notes could otherwise stay on screen with their text visible after the vault had locked.
- **Activity after the deadline cannot postpone a due lock.** A touch once the idle time has already passed leaves the lock due. Idle is checked every 5 seconds, so locking follows the timer within a few seconds. The manager now says the check happens every few seconds.
- **Locking saves the last keystrokes first.** Lock from the manager, a sticky, or the tray flushes open notes before they close. The short wait before lock runs off the UI thread, so it can no longer block those saves.
- **Unlock works in the first minute after Windows starts.** The rate limiter could crash when its 60-second attempt window was still younger than the system timer, and then every later unlock failed until SecretSticky was restarted.
- **Changing the master password keeps the recovery key working.** The vault keeps the key-derivation settings it was created with. Replacing them with this build's defaults would make the recovery key derive under settings it was not created with.
- **Setup, unlock, recovery unlock, and password change leave the windows responsive** while the key is derived.
- **An unreadable vault is kept aside.** If `vault.json` cannot be read and SecretSticky restores `vault.json.bak`, the unreadable file is copied to `vault.json.corrupt` first.

### Build/packaging

- The shipped app includes patched rustls 0.23.46, rustls-webpki 0.103.17, webpki-root-certs 1.0.12, tokio 1.54.1, rustix 1.1.6, and async-executor 1.14.1, including the TLS stack used for update checks.
- CI and the release build use Node 22. Dev-tool updates clear npm audit advisories: Vite 7.3.8, Vitest 5.1.0, nanoid 3.3.20, and source-map-js 1.2.2. The note format is unchanged.

### Install and update

- Windows setup: `SecretSticky_0.1.8_x64-setup.exe` (Start Menu, desktop shortcut, and taskbar icon). `SecretSticky_0.1.8_x64_en-US.msi` is attached as well.
- Already installed: **About → Check for updates**. The update replaces the application only.
- Notes stay in `%APPDATA%\SecretSticky` and unlock with the same master password and recovery key. Existing vaults read and write as before.

Same text: [docs/RELEASE-0.1.8.md](docs/RELEASE-0.1.8.md).

## [0.1.7] — 2026-09-03

### Added

- **Sticky chrome:** every note window gains **New note** (same-color sibling), **Manager**, and **Lock** buttons — create, manage, and lock straight from a sticky
- **Auto-lock timer settings** in the manager (grid → Auto-lock): Off or 1 / 5 / 15 / 30 min / 1 / 2 / 4 / 8 / 12 h — persisted across lock; backend clamps to the same 12 h ceiling

### Changed

- New stickies no longer force **always on top** — a note floats only while pinned (📌); existing notes keep their saved flag

### Security

- Sticky windows may now create a sibling note and lock the vault (same authenticated, unlocked session — per-note body/delete ACL unchanged, unknown window labels still denied)

## [0.1.6] — 2026-08-13

### Security

- **Vault durability (Windows):** replace `vault.json` with `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` — never delete the live file before the new bytes are at the destination (closes a crash/failure window that could wipe the only copy of secrets)
- Flush temp vault writes (`sync_all`) before swap
- Keep `vault.json.bak` last-known-good; restore on missing or corrupt live file — never treat that as a fresh empty setup
- Refuse vault format versions newer than this build (no unsafe write-back of secrets)

### Fixed

- Idle lock path can detect due lock without mutating session first (`is_idle_lock_due`) so UI can flush before `lock()`
- Note window / manager small hardening around save and lock coordination

### Documentation

- Record non-negotiable invariant: **updates must never corrupt saved stickies** (SECURITY.md, CONTRIBUTING.md, README, vault module docs)

## [0.1.5] — 2026-07-30

### Fixed

- Sticky note windows stuck on **Loading note…** with *only the manager can create or delete notes* after 0.1.4 ACL hardening — `vault_status` and `show_main` again allow `note-*` windows (needed for boot unlock check and Unlock button)

## [0.1.4] — 2026-07-30

### Added

- **Single-instance** lock via `tauri-plugin-single-instance` — a second launch focuses the existing manager instead of starting another process
- Split Tauri capabilities: manager (`main`) vs sticky (`note-*`) windows
- Expanded frontend unit coverage (`api`, `errors`, `updater`)

### Security

- Master password minimum length raised to **12** characters (setup + change password)
- Manager-only ACL on vault status, quit, show/hide main, open external URL
- Sticky note windows cannot read other notes’ full bodies; manager list is preview-only
- `notes_get` body access limited to the owning note window (not manager)
- CSP allows `img-src blob:` for in-app assets; sticky capability omits process/updater
- Documented hardening notes in [SECURITY.md](SECURITY.md)

### Changed

- Dev server port **1422** (avoids clash with sibling local apps)

## [0.1.3] — 2026-07-28

### Added

- In-app **delete confirmation** modal (no browser `localhost says` dialog)
- Manager **About** layout polish: equal link chips, update row, Close footer

### Changed

- Manager default window size **440×560** (min 400×500)
- Sticky note minimum size **345×250** (resize floor + create/open/geometry clamps)
- New stickies default **345×280** (min width, slightly taller for typing)
- Manager chrome: two-row header, stacked footer, delete control inside note rows
- Note list scrolls independently so header/footer stay put

### Fixed

- Header/footer wrap and cramped toolbar at manager width
- About panel button salad / uneven blocks
- Detached note-row delete control

## [0.1.2] — 2026-07-28

### Added

- In-app **About** panel: “Hi I'm Ahmi, hope this helps!” with links to the [GitHub profile](https://github.com/AhmiDarrow), project repo, and Releases (allowlisted `open_external_url`)
- Brand mark on unlock/setup and manager header
- **Auto-update** from GitHub Releases (signed minisign artifacts + `latest.json`); About → **Check for updates** / install & restart
- Version label in About (from the running binary)

### Fixed

- Brand icon applied on **every** window (manager + stickies), tray, installer, and shortcuts
- Explicit window/taskbar icon set so WebView2 paths no longer fall back to the old default mark
- NSIS installer uses the sticky+lock ICO for setup UI and installed shortcuts
- Re-apply window title + icon when showing the manager from tray
- Version surfaces aligned (`package.json`, `tauri.conf.json`, `Cargo.toml`)

### Changed

- App icon: yellow sticky + padlock (window, tray, installer ICO/PNG set)
- Webview favicon uses bundled PNG instead of default Vite mark
- Release workflow signs updater payloads and publishes updater JSON alongside NSIS/MSI

## [0.1.0] — 2026-07-28

### Added

- Master-password vault with **Argon2id** KDF and **XChaCha20-Poly1305** AEAD
- Titles and bodies encrypted at rest under `%APPDATA%\SecretSticky\vault.json`
- Stable content key: change password without re-encrypting notes or invalidating recovery
- One-time **recovery key** at setup (store offline)
- Multiple colored sticky windows (yellow, green, pink, blue, purple, gray, black, dark green)
- High-contrast ink colors per sticky background
- System tray: New note, Show manager, Open all, Lock, Quit
- Manager **close → tray** (process stays resident until tray Quit)
- Opening a sticky tucks the manager away; unlock does **not** auto-open all notes
- Auto-lock after idle (default 15 minutes)
- Clipboard auto-clear for copied secrets (30s; recovery key 2 min)
- Change master password from the manager
- Self-hosted **Inter** font (no Google Fonts network call)
- IPC ACL: note windows cannot list/open admin vault actions or other notes’ bodies
- CI (frontend + Rust) and tag-driven Windows release workflow

### Security notes

- Protects secrets **at rest** on disk; does not protect a compromised OS while unlocked
- Geometry meta (position/size/color) is plaintext so windows can restore chrome
- No cloud sync, no account, no backdoor — lost password **and** recovery key means data loss
