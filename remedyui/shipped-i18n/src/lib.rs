//! `src-tauri/src/i18n.rs` as the Tauri app ships it: the language catalogs
//! (`src/locales/*.json`), the system-language match and `settings.json` beside the vault.

#[path = "../../../src-tauri/src/i18n.rs"]
#[allow(dead_code)]
mod i18n;

pub use i18n::{load_or_init_lang, match_system_tag, set_lang, t};
