//! UI language catalogs and the plaintext `settings.json` beside the vault.
//!
//! Language is not a secret and must stay readable before unlock, including
//! when no vault exists yet. Writes merge into the JSON object so unknown
//! keys survive. `vault.json` is never touched here.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Map, Value};

pub struct I18nState {
    pub lang: std::sync::Mutex<String>,
    pub data_dir: PathBuf,
}

pub fn ci_mode() -> bool {
    matches!(
        std::env::var("SECRETSTICKY_CI").ok().as_deref(),
        Some("1") | Some("true")
    )
}

pub fn exact_locale_id(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "en" => "en",
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "pt-BR" => "pt-BR",
        "it" => "it",
        "ru" => "ru",
        "pl" => "pl",
        "ja" => "ja",
        "ko" => "ko",
        "zh-CN" => "zh-CN",
        _ => return None,
    })
}

/// Map an OS or stored tag onto a shipped locale. Traditional Chinese stays English.
pub fn match_system_tag(tag: &str) -> &'static str {
    let norm = tag.trim().replace('_', "-");
    let lower = norm.to_ascii_lowercase();
    if lower.is_empty() {
        return "en";
    }
    if let Some(id) = exact_locale_id_ci(&lower) {
        return id;
    }
    if lower == "zh-tw" || lower == "zh-hk" || lower == "zh-mo" || lower.contains("hant") {
        return "en";
    }
    let primary = lower.split('-').next().unwrap_or("");
    match primary {
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "pt" => "pt-BR",
        "it" => "it",
        "ru" => "ru",
        "pl" => "pl",
        "ja" => "ja",
        "ko" => "ko",
        "zh" => "zh-CN",
        "en" => "en",
        _ => "en",
    }
}

fn exact_locale_id_ci(lower: &str) -> Option<&'static str> {
    Some(match lower {
        "en" => "en",
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "pt-br" => "pt-BR",
        "it" => "it",
        "ru" => "ru",
        "pl" => "pl",
        "ja" => "ja",
        "ko" => "ko",
        "zh-cn" => "zh-CN",
        _ => return None,
    })
}

pub fn system_locale_tag() -> String {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserDefaultLocaleName(lp_locale_name: *mut u16, cch_locale_name: i32) -> i32;
        }
        let mut buf = [0u16; 85];
        let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 1 {
            return String::new();
        }
        return String::from_utf16_lossy(&buf[..(n as usize) - 1]);
    }
    #[cfg(not(windows))]
    {
        let raw = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        raw.split(['.', '@']).next().unwrap_or("").to_string()
    }
}

/// Saved language wins. `SECRETSTICKY_LANG` applies only in CI when nothing is saved.
pub fn initial_lang_from(ci: bool, secretsticky_lang: Option<&str>, system_tag: &str) -> String {
    if ci {
        if let Some(raw) = secretsticky_lang {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return match_system_tag(trimmed).to_string();
            }
        }
    }
    match_system_tag(system_tag).to_string()
}

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

fn parse_catalog(raw: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

macro_rules! catalog_cell {
    ($name:ident, $file:literal) => {
        fn $name() -> &'static Map<String, Value> {
            static CELL: OnceLock<Map<String, Value>> = OnceLock::new();
            CELL.get_or_init(|| parse_catalog(include_str!($file)))
        }
    };
}

catalog_cell!(cat_en, "../../src/locales/en.json");
catalog_cell!(cat_es, "../../src/locales/es.json");
catalog_cell!(cat_fr, "../../src/locales/fr.json");
catalog_cell!(cat_de, "../../src/locales/de.json");
catalog_cell!(cat_pt_br, "../../src/locales/pt-BR.json");
catalog_cell!(cat_it, "../../src/locales/it.json");
catalog_cell!(cat_ru, "../../src/locales/ru.json");
catalog_cell!(cat_pl, "../../src/locales/pl.json");
catalog_cell!(cat_ja, "../../src/locales/ja.json");
catalog_cell!(cat_ko, "../../src/locales/ko.json");
catalog_cell!(cat_zh_cn, "../../src/locales/zh-CN.json");

fn catalog_map(lang: &str) -> &'static Map<String, Value> {
    match exact_locale_id(lang).unwrap_or("en") {
        "es" => cat_es(),
        "fr" => cat_fr(),
        "de" => cat_de(),
        "pt-BR" => cat_pt_br(),
        "it" => cat_it(),
        "ru" => cat_ru(),
        "pl" => cat_pl(),
        "ja" => cat_ja(),
        "ko" => cat_ko(),
        "zh-CN" => cat_zh_cn(),
        _ => cat_en(),
    }
}

fn lookup(lang: &str, key: &str) -> Option<String> {
    catalog_map(lang)
        .get(key)
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
        .map(|text| text.to_string())
}

/// English catalog, then the key. Never empty, never panics.
pub fn t(lang: &str, key: &str) -> String {
    let id = exact_locale_id(lang).unwrap_or("en");
    if let Some(text) = lookup(id, key) {
        return text;
    }
    if id != "en" {
        if let Some(text) = lookup("en", key) {
            return text;
        }
    }
    key.to_string()
}

fn write_settings_atomic(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    fs::write(&tmp, text).map_err(|err| err.to_string())?;
    if path.exists() {
        fs::remove_file(path).map_err(|err| err.to_string())?;
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::copy(&tmp, path);
        return Err(err.to_string());
    }
    Ok(())
}

fn quarantine_settings(path: &Path) {
    let bak = path.with_file_name("settings.json.bak");
    let corrupt = path.with_file_name("settings.json.corrupt");
    if !bak.exists() {
        let _ = fs::copy(path, &bak);
    } else if !corrupt.exists() {
        let _ = fs::copy(path, &corrupt);
    }
}

fn read_settings_object(path: &Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(err) => Err(err.to_string()),
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(Value::Object(map)),
            Ok(_) => Err("settings.json is not an object".into()),
            Err(err) => Err(err.to_string()),
        },
    }
}

pub fn load_settings_lang_with(
    data_dir: &Path,
    system_tag: &str,
    ci: bool,
    secretsticky_lang: Option<&str>,
) -> String {
    let path = settings_path(data_dir);
    match fs::read_to_string(&path) {
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let lang = initial_lang_from(ci, secretsticky_lang, system_tag);
            let value = serde_json::json!({ "lang": lang });
            let _ = write_settings_atomic(&path, &value);
            lang
        }
        Err(_) => "en".to_string(),
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(mut map)) => {
                if let Some(Value::String(saved)) = map.get("lang").cloned() {
                    return match_system_tag(&saved).to_string();
                }
                let lang = initial_lang_from(ci, secretsticky_lang, system_tag);
                map.insert("lang".into(), Value::String(lang.clone()));
                let _ = write_settings_atomic(&path, &Value::Object(map));
                lang
            }
            _ => {
                quarantine_settings(&path);
                let lang = initial_lang_from(ci, secretsticky_lang, system_tag);
                let value = serde_json::json!({ "lang": lang });
                let _ = write_settings_atomic(&path, &value);
                lang
            }
        },
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_settings_lang(data_dir: &Path, fallback_tag: &str) -> String {
    load_settings_lang_with(data_dir, fallback_tag, false, None)
}

pub fn load_or_init_lang(data_dir: &Path) -> String {
    let override_lang = std::env::var("SECRETSTICKY_LANG").ok();
    load_settings_lang_with(
        data_dir,
        &system_locale_tag(),
        ci_mode(),
        override_lang.as_deref(),
    )
}

pub fn set_lang(data_dir: &Path, requested: &str) -> Result<String, String> {
    let id = exact_locale_id(requested).ok_or_else(|| "unsupported language".to_string())?;
    let path = settings_path(data_dir);
    let mut value = read_settings_object(&path)?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("lang".into(), Value::String(id.to_string()));
    }
    write_settings_atomic(&path, &value)?;
    Ok(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::fs;

    #[test]
    fn match_system_tag_cases() {
        assert_eq!(match_system_tag("de-DE"), "de");
        assert_eq!(match_system_tag("pt-BR"), "pt-BR");
        assert_eq!(match_system_tag("pt-PT"), "pt-BR");
        assert_eq!(match_system_tag("zh-CN"), "zh-CN");
        assert_eq!(match_system_tag("zh-TW"), "en");
        assert_eq!(match_system_tag("ja-JP"), "ja");
        assert_eq!(match_system_tag("en-US"), "en");
        assert_eq!(match_system_tag("fr-CA"), "fr");
        assert_eq!(match_system_tag("unknown"), "en");
        assert_eq!(match_system_tag("zh-Hans"), "zh-CN");
        assert_eq!(match_system_tag("zh-hant-tw"), "en");
        assert_eq!(match_system_tag("PT_br"), "pt-BR");
        assert_eq!(match_system_tag("zh-HK"), "en");
        assert_eq!(match_system_tag("zh-SG"), "zh-CN");
        assert!(!"zh-hans".contains("hant"));
    }

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'{' {
                if let Some(end) = text[index + 1..].find('}') {
                    let name = &text[index + 1..index + 1 + end];
                    if !name.is_empty()
                        && name
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    {
                        names.insert(name.to_string());
                    }
                    index += end + 2;
                    continue;
                }
            }
            index += 1;
        }
        names
    }

    fn strong_counts(text: &str) -> (usize, usize) {
        (
            text.matches("<strong>").count(),
            text.matches("</strong>").count(),
        )
    }

    #[test]
    fn catalogs_match_english() {
        let brands = [
            "SecretSticky",
            "Ahmi",
            "Argon2id",
            "XChaCha20-Poly1305",
            "GitHub",
            "MIT",
            "Windows",
        ];
        let english = cat_en();
        let hello = english
            .get("about.hello")
            .and_then(|value| value.as_str())
            .unwrap();
        assert_eq!(hello, "Hi I'm Ahmi, hope this helps!");
        let langs = [
            "en", "es", "fr", "de", "pt-BR", "it", "ru", "pl", "ja", "ko", "zh-CN",
        ];
        for lang in langs {
            let table = catalog_map(lang);
            assert_eq!(table.len(), english.len(), "{lang}");
            for (key, en_value) in english {
                let en_text = en_value.as_str().unwrap_or("");
                let value = table.get(key).and_then(|item| item.as_str()).unwrap_or("");
                assert!(!value.is_empty(), "{lang}:{key}");
                assert_eq!(placeholders(value), placeholders(en_text), "{lang}:{key}");
                assert_eq!(strong_counts(value), strong_counts(en_text), "{lang}:{key}");
                for brand in brands {
                    if en_text.contains(brand) {
                        assert!(value.contains(brand), "{lang}:{key}:{brand}");
                    }
                }
            }
            assert_eq!(
                table.get("about.hello").and_then(|v| v.as_str()),
                Some(hello)
            );
        }
        assert_eq!(t("es", "missing.key"), "missing.key");
        assert_eq!(t("zz", "about.hello"), hello);
    }

    #[test]
    fn settings_roundtrip_preserves_extra_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark","lang":"en"}"#).unwrap();
        let saved = set_lang(dir.path(), "ja").unwrap();
        assert_eq!(saved, "ja");
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["lang"], "ja");
    }

    #[test]
    fn set_lang_refuses_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{").unwrap();
        assert!(set_lang(dir.path(), "ja").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{");
    }

    #[test]
    fn corrupt_file_is_preserved_aside_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{not json").unwrap();
        let lang = load_settings_lang(dir.path(), "fr");
        assert_eq!(lang, "fr");
        let bak = fs::read(dir.path().join("settings.json.bak")).unwrap();
        assert_eq!(bak, b"{not json");
        fs::write(&path, b"[1,2]").unwrap();
        let again = load_settings_lang(dir.path(), "de");
        assert_eq!(again, "de");
        assert_eq!(fs::read(dir.path().join("settings.json.bak")).unwrap(), bak);
        assert_eq!(
            fs::read(dir.path().join("settings.json.corrupt")).unwrap(),
            b"[1,2]"
        );
    }

    #[test]
    fn first_run_writes_detected_tag_and_keeps_saved_choice() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_settings_lang(dir.path(), "de-DE"), "de");
        let value: Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(value["lang"], "de");
        let dir_ci = tempfile::tempdir().unwrap();
        assert_eq!(
            load_settings_lang_with(dir_ci.path(), "en-US", true, Some("ja-JP")),
            "ja"
        );
        assert_eq!(
            load_settings_lang_with(dir_ci.path(), "en-US", true, Some("de")),
            "ja"
        );
    }

    #[test]
    fn object_without_lang_merges_and_io_error_stays_english() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(load_settings_lang(dir.path(), "fr"), "fr");
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["lang"], "fr");

        let blocked = tempfile::tempdir().unwrap();
        fs::create_dir_all(blocked.path().join("settings.json")).unwrap();
        assert_eq!(load_settings_lang(blocked.path(), "de"), "en");
        assert!(blocked.path().join("settings.json").is_dir());
    }

    #[test]
    fn initial_lang_ignores_override_outside_ci() {
        assert_eq!(initial_lang_from(false, Some("ja"), "de-DE"), "de");
        assert_eq!(initial_lang_from(true, Some(" "), "fr-CA"), "fr");
        assert_eq!(initial_lang_from(true, None, "pl"), "pl");
    }

    #[test]
    fn language_file_does_not_change_vault_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = Vault::open_path(dir.path().join("vault.json")).unwrap();
        let _recovery = vault.setup("password1234").unwrap();
        let before = fs::read(vault.path()).unwrap();
        assert_eq!(load_settings_lang(dir.path(), "de"), "de");
        assert_eq!(fs::read(vault.path()).unwrap(), before);
        let settings = dir.path().join("settings.json");
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        value["extra"] = Value::String("keep".into());
        fs::write(&settings, serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(set_lang(dir.path(), "ja").unwrap(), "ja");
        let after: Value = serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(after["extra"], "keep");
        assert_eq!(after["lang"], "ja");
        assert_eq!(fs::read(vault.path()).unwrap(), before);
        assert!(vault.status().unlocked);
        vault.lock();
        assert_eq!(fs::read(vault.path()).unwrap(), before);
        vault.unlock("password1234").unwrap();
        assert!(vault.status().unlocked);
    }
}
