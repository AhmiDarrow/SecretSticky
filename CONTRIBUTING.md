# Contributing to SecretSticky

Thanks for helping harden a local secrets app. Keep changes small, tested, and honest about the threat model.

## Prerequisites

- Windows 10/11 (primary target)
- Node.js 20+
- Rust stable (edition 2021)
- WebView2 (ships with modern Windows)

## Setup

```bash
git clone https://github.com/AhmiDarrow/SecretSticky.git
cd SecretSticky
npm install
npm run tauri:dev
```

## Tests (required before PR)

```bash
# Frontend unit tests + typecheck
npm test

# Rust crypto/vault/ACL unit tests
npm run test:rust

# Production frontend bundle
npm run build
```

CI runs the same gates on `windows-latest`.

## Project layout

| Path | Role |
|------|------|
| `src/` | React UI (unlock, manager, sticky windows) |
| `src-tauri/src/crypto.rs` | Argon2id + XChaCha20-Poly1305 |
| `src-tauri/src/vault.rs` | On-disk format + session |
| `src-tauri/src/commands.rs` | Tauri IPC + window/tray ACL |
| `.github/workflows/` | CI + release |
| `app-icon.png` | Master 1024×1024 icon source |
| `scripts/prepare_icon.py` | Rebuild public favicons from the master PNG |
| `src-tauri/icons/` | Window / tray / installer icons (`npx tauri icon app-icon.png`) |

### Regenerating icons

```bash
# After replacing app-icon.png (square PNG, ideally 1024×1024):
python scripts/prepare_icon.py
npx tauri icon app-icon.png -o src-tauri/icons
# Drop mobile trees if generated (Windows-only product):
# remove src-tauri/icons/android and src-tauri/icons/ios
```

## Security expectations

- Never log passwords, recovery keys, or note bodies
- Prefer encrypting new sensitive fields; document any intentional plaintext (e.g. geometry)
- Keep IPC least-privilege: note windows must not gain manager-only commands
- Update README threat model when behavior changes

## Non-negotiable: updates must NEVER corrupt saved stickies

See [SECURITY.md](SECURITY.md) — this is a **ship blocker**, not a nice-to-have.

Before any PR that touches vault format, crypto, persistence, IPC ACL, sticky boot, installers, or the updater:

1. **AppData is sacred** — updates replace the app binary only. Never delete/rewrite `%APPDATA%\SecretSticky\vault.json` from install/update paths.
2. **Backward-compatible vault reads** — any new code must still unlock and list notes from vaults written by prior 0.1.x builds. Prefer optional fields + defaults; no destructive migrations.
3. **Sticky windows must still load** — if you tighten ACL, re-test open sticky, create sticky, unlock-from-locked-sticky, and manager list. A sticky stuck on “Loading note…” with a cryptic ACL error is a **critical regression** (as in 0.1.4 → fixed in 0.1.5).
4. **No silent data wipe** — failed decrypt, locked vault, or ACL deny must error clearly; never replace the vault with an empty one “to fix” load errors.
5. **Tests** — keep or add vault reload / legacy-format / persist-replace coverage in `vault.rs` when touching disk format.

Checklist for release (in addition to green CI):

- [ ] Fresh vault: create note → quit → reopen → body intact  
- [ ] Upgrade path: vault from previous release still unlocks with same password  
- [ ] Open sticky from manager after upgrade  
- [ ] Auto-update / installer does not touch AppData vault  

## Commit style

Conventional commits preferred:

- `feat:` new user-visible capability
- `fix:` bug fix
- `security:` hardening / crypto / ACL
- `test:`, `docs:`, `ci:`, `chore:`

## Release

1. Bump version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` together
2. Update `CHANGELOG.md`
3. Merge to `main` with green CI
4. Ensure repo secret **`TAURI_SIGNING_PRIVATE_KEY`** is set (minisign private key matching the public key in `tauri.conf.json` → `plugins.updater.pubkey`). Optional password: `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
5. Tag `vX.Y.Z` and push — **Release** workflow builds Windows NSIS/MSI, signs updater artifacts, publishes `latest.json`, and opens a draft GitHub Release

```bash
# One-time keygen (store private key only in GitHub secrets / offline backup)
npx tauri signer generate -w ~/.tauri/secretsticky.key
# Public key → tauri.conf.json plugins.updater.pubkey
# Private key → gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/secretsticky.key
```

## License

By contributing, you agree your changes are licensed under the MIT License (see `LICENSE`).
