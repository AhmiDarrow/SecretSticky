# Changelog

All notable changes to SecretSticky are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
