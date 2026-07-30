# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## What SecretSticky protects

- Vault file at rest (`%APPDATA%\SecretSticky\vault.json`)
- Note **titles and bodies** (authenticated encryption)
- Casual snooping, disk theft, and backup copies of the vault file

## What it does **not** protect

- Malware running as your user while the vault is unlocked
- Keyloggers capturing the master password
- Screen capture of open sticky windows
- A fully compromised OS or debugger attached to the process

There is **no backdoor** and **no cloud recovery**. Losing the master password and the recovery key means permanent data loss.

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

