//! SecretSticky on RemedyUI: encrypted sticky notes that live on the desktop.
//!
//! The vault (Argon2id + XChaCha20-Poly1305) is the same code the Tauri app uses; this
//! crate is only the interface: a manager window that hides to the tray, frameless
//! always-on-top note windows, idle lock, clipboard clearing and signed updates.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../../src-tauri/src/crypto.rs"]
#[allow(dead_code)]
mod crypto;
#[path = "../../src-tauri/src/error.rs"]
#[allow(dead_code)]
mod error;
#[path = "../../src-tauri/src/vault.rs"]
#[allow(dead_code)]
mod vault;

mod brand;
mod fonts;
mod manager;
mod note;
mod state;
mod theme;
mod tray;
mod update;

#[cfg(test)]
mod flow_tests;

fn main() {
    // Another vault is another app: its own single-instance lock and window geometry.
    let name = if state::vault_override().is_some() { "SecretSticky (other vault)" } else { "SecretSticky" };
    let config = fonts::with_inter(remedy_ui::AppConfig::new(name).single_instance().theme(theme::theme(), theme::sheet));
    if let Err(e) = remedy_ui::run(config, |app| {
        state::start(app);
    }) {
        eprintln!("SecretSticky: {e:?}");
        std::process::exit(1);
    }
}
