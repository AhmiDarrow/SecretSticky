//! The font and text size choices, kept in `settings.json` beside the vault under `font`
//! and `textSize`. The file is shared
//! with the language (written by the shipped code, which keeps keys it does not know), so
//! this reads the whole object and writes it back with only `font` changed, the same way.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde_json::{Map, Value};

fn path(dir: &Path) -> std::path::PathBuf {
    dir.join("settings.json")
}

/// The saved font id, or `inter` when none is saved, the file cannot be read, or the id is
/// one this version does not offer.
pub fn load_font(dir: &Path) -> String {
    let saved = fs::read_to_string(path(dir))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("font").and_then(Value::as_str).map(str::to_owned));
    match saved {
        Some(id) if crate::fonts::CHOICES.iter().any(|(i, _, _)| *i == id) => id,
        _ => crate::fonts::CHOICES[0].0.to_owned(),
    }
}

/// The text sizes offered: (setting id, percent of the designed size).
pub const TEXT_SIZES: [(&str, u32); 5] = [("90", 90), ("100", 100), ("115", 115), ("130", 130), ("150", 150)];

/// The saved text size's scale (1 as designed); 1 when none or an unknown one is saved.
pub fn load_text_size(dir: &Path) -> String {
    let saved = fs::read_to_string(path(dir))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("textSize").and_then(Value::as_str).map(str::to_owned));
    match saved {
        Some(id) if TEXT_SIZES.iter().any(|(i, _)| *i == id) => id,
        _ => "100".to_owned(),
    }
}

/// The scale for a text size id.
pub fn text_scale(id: &str) -> f32 {
    TEXT_SIZES.iter().find(|(i, _)| *i == id).map_or(1.0, |(_, p)| *p as f32 / 100.0)
}

/// Saves `id` as the font, keeping every other key.
pub fn save_font(dir: &Path, id: &str) -> Result<(), String> {
    save(dir, "font", id)
}

/// Saves `id` as the text size, keeping every other key.
pub fn save_text_size(dir: &Path, id: &str) -> Result<(), String> {
    save(dir, "textSize", id)
}

/// Sets one key, keeping every other. A file that is not a JSON object is left alone and the
/// save fails, as the shipped language save does.
fn save(dir: &Path, key: &str, value: &str) -> Result<(), String> {
    let p = path(dir);
    let mut map = match fs::read_to_string(&p) {
        Err(e) if e.kind() == ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(e.to_string()),
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err("settings.json is not an object".into()),
            Err(e) => return Err(e.to_string()),
        },
    };
    map.insert(key.into(), Value::String(value.to_owned()));
    let text = serde_json::to_string_pretty(&Value::Object(map)).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Written whole beside it, then swapped in: a crash leaves the old file or the new one.
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_saves_beside_the_language_and_keeps_every_other_key() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert_eq!(load_font(dir.path()), "inter", "nothing saved yet");
        fs::write(path(dir.path()), r#"{"lang":"de","fromNewer":7}"#).expect("write");
        save_font(dir.path(), "ahmi-pixel").expect("saved");
        assert_eq!(load_font(dir.path()), "ahmi-pixel");
        let v: Value = serde_json::from_str(&fs::read_to_string(path(dir.path())).expect("read")).expect("json");
        assert_eq!(v["lang"], "de");
        assert_eq!(v["fromNewer"], 7);
        fs::write(path(dir.path()), r#"{"font":"a-font-from-later"}"#).expect("write");
        assert_eq!(load_font(dir.path()), "inter", "an unknown font falls back");
        fs::write(path(dir.path()), "[1]").expect("write");
        assert!(save_font(dir.path(), "inter").is_err(), "a broken file is not overwritten");
        assert_eq!(fs::read_to_string(path(dir.path())).expect("read"), "[1]");
    }

    #[test]
    fn the_text_size_saves_beside_the_font() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert_eq!((load_text_size(dir.path()), text_scale("100")), ("100".to_owned(), 1.0));
        save_font(dir.path(), "ahmi-print").expect("font");
        save_text_size(dir.path(), "130").expect("size");
        assert_eq!((load_font(dir.path()), load_text_size(dir.path())), ("ahmi-print".to_owned(), "130".to_owned()));
        assert!((text_scale("130") - 1.3).abs() < 1e-6);
        assert_eq!(text_scale("7"), 1.0, "an unknown size is the designed one");
    }
}
