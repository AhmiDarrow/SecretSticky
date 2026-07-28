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

## Security expectations

- Never log passwords, recovery keys, or note bodies
- Prefer encrypting new sensitive fields; document any intentional plaintext (e.g. geometry)
- Keep IPC least-privilege: note windows must not gain manager-only commands
- Update README threat model when behavior changes

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
4. Tag `vX.Y.Z` and push — **Release** workflow builds Windows installers and opens a draft GitHub Release

## License

By contributing, you agree your changes are licensed under the MIT License (see `LICENSE`).
