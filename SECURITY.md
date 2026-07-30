# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Non-negotiable: updates must NEVER corrupt saved stickies

**Hard product invariant.** App updates (NSIS/MSI install, in-app auto-update, or replacing the binary) must **never**:

1. Delete, truncate, wipe, or overwrite `%APPDATA%\SecretSticky\vault.json` as part of install/update
2. Rewrite ciphertext so existing notes cannot decrypt with the same master password / recovery key
3. Ship a vault-format change that cannot **read** vaults written by every prior supported 0.1.x build
4. Make sticky note data unreadable by tightening IPC/ACL/UI without a working path to open notes (a stuck “Loading note…” is a **critical bug**, not acceptable “hardening”)

**Allowed / required:**

- Installers and the updater replace **application binaries only** under Program Files / install dir — never the user vault under AppData
- On-disk format evolution is **additive and backward-compatible** only (new optional fields with defaults; read old → write new only after a successful unlock)
- Atomic vault saves (`vault.json.tmp` then replace) so a crash mid-write does not leave a half-written file as the only copy
- UI or ACL bugs may block *display*, but must not destroy *ciphertext on disk*

If a change could violate this, it **does not ship**. Fix data safety first.

## What SecretSticky protects

- Vault file at rest (`%APPDATA%\SecretSticky\vault.json`)
- Note **titles and bodies** (authenticated encryption)
- Casual snooping, disk theft, and backup copies of the vault file
- Continuity of saved stickies across app updates (see invariant above)

## What it does **not** protect

- Malware running as your user while the vault is unlocked
- Keyloggers capturing the master password
- Screen capture of open sticky windows
- A fully compromised OS or debugger attached to the process

There is **no backdoor** and **no cloud recovery**. Losing the master password and the recovery key means permanent data loss. The app will never intentionally destroy your vault to “recover” from that.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for exploitable crypto, IPC bypass, or plaintext leaks.

1. Email or message the maintainer privately (GitHub: [@AhmiDarrow](https://github.com/AhmiDarrow))
2. Include: SecretSticky version, OS build, steps to reproduce, impact
3. Allow reasonable time for a fix before public disclosure

We will credit reporters who want attribution (unless you prefer anonymity).

## Hardening tips for users

- Use a long, unique master password
- Store the recovery key offline (password manager or paper), not in another sticky
- Lock the vault when stepping away; tray → **Lock**
- Prefer the in-app copy button (auto-clear) over leaving secrets in the clipboard
- Keep Windows and WebView2 updated

## Hardening notes

- Note windows cannot read other notes' full bodies; manager uses preview list only.
- Manager + sticky windows may read vault status (no secrets) and show the manager for unlock.
- Manager-only commands: quit, hide main, open external URL, create note, vault setup/unlock/lock, list notes, open windows, idle settings, change password.
- Sticky note capability is split from manager (no process/updater permissions).
- Master password minimum length is 12 characters.
- Unlock attempts are rate-limited (escalating cooldown after repeated failures).

