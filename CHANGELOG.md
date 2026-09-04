# Changelog

All notable changes to SecretSticky are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
