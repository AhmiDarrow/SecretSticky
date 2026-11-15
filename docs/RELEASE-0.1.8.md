# SecretSticky 0.1.8

Auto-lock closes your notes, and locking saves the last keystrokes before the notes close.

Hi I'm Ahmi, hope this helps!

## Fixes

- **Auto-lock closes open notes.** If the vault locks during a save or another vault action, SecretSticky still closes the stickies and shows the unlock screen. Those notes could otherwise stay on screen with their text visible after the vault had locked.
- **Activity after the deadline cannot postpone a due lock.** A touch once the idle time has already passed leaves the lock due. Idle is checked every 5 seconds, so locking follows the timer within a few seconds. The manager now says the check happens every few seconds.
- **Locking saves the last keystrokes first.** Lock from the manager, a sticky, or the tray flushes open notes before they close. The short wait before lock runs off the UI thread, so it can no longer block those saves.
- **Unlock works in the first minute after Windows starts.** The rate limiter could crash when its 60-second attempt window was still younger than the system timer, and then every later unlock failed until SecretSticky was restarted.
- **Changing the master password keeps the recovery key working.** The vault keeps the key-derivation settings it was created with. Replacing them with this build's defaults would make the recovery key derive under settings it was not created with.
- **Setup, unlock, recovery unlock, and password change leave the windows responsive** while the key is derived.
- **An unreadable vault is kept aside.** If `vault.json` cannot be read and SecretSticky restores `vault.json.bak`, the unreadable file is copied to `vault.json.corrupt` first.

## Build/packaging

- The shipped app includes patched rustls 0.23.46, rustls-webpki 0.103.17, webpki-root-certs 1.0.12, tokio 1.54.1, rustix 1.1.6, and async-executor 1.14.1, including the TLS stack used for update checks.
- CI and the release build use Node 22. Dev-tool updates clear npm audit advisories: Vite 7.3.8, Vitest 5.1.0, nanoid 3.3.20, and source-map-js 1.2.2. The note format is unchanged.

## Install and update

- Windows setup: `SecretSticky_0.1.8_x64-setup.exe` (Start Menu, desktop shortcut, and taskbar icon). `SecretSticky_0.1.8_x64_en-US.msi` is attached as well.
- Already installed: **About → Check for updates**. The update replaces the application only.
- Notes stay in `%APPDATA%\SecretSticky` and unlock with the same master password and recovery key. Existing vaults read and write as before.
