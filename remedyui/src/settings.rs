//! The font choice, kept in `settings.json` beside the vault under `font`. The file is shared
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

/// Saves `id` as the font, keeping every other key. A file that is not a JSON object is left
/// alone and the save fails, as the shipped language save does.
pub fn save_font(dir: &Path, id: &str) -> Result<(), String> {
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
    map.insert("font".into(), Value::String(id.to_owned()));
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
}
