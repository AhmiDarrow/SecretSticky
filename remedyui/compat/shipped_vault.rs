//! Drives a shipped release's own vault code (`vault.rs`, `crypto.rs`, `error.rs` taken from a
//! release tag by `compat/shipped.sh`), so the RemedyUI build's vault handling is checked
//! against the exact code people's vaults were written with.
//!
//!   make <vault.json> <password>      a vault with sample notes; prints the recovery key
//!   dump <vault.json> <password>      unlocks and prints every note and setting as JSON
//!   dump-recovery <vault.json> <key>  the same, unlocked with the recovery key

mod crypto;
mod error;
mod vault;

use std::path::PathBuf;

use vault::{NoteColor, Vault};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (cmd, path, secret) = match args.as_slice() {
        [_, c, p, s] => (c.as_str(), PathBuf::from(p), s.as_str()),
        _ => {
            eprintln!("usage: shipped-vault make|dump|dump-recovery <vault.json> <password|key>");
            std::process::exit(2);
        }
    };
    let result = match cmd {
        "make" => make(path, secret),
        "dump" => dump(path, secret, false),
        "dump-recovery" => dump(path, secret, true),
        _ => Err(format!("unknown command {cmd}")),
    };
    if let Err(e) = result {
        eprintln!("shipped-vault: {e}");
        std::process::exit(1);
    }
}

fn make(path: PathBuf, password: &str) -> Result<(), String> {
    let mut v = Vault::open_path(path).map_err(|e| e.to_string())?;
    let recovery = v.setup(password).map_err(|e| e.to_string())?;
    // (color, title, body, geometry, pinned)
    type Geo = Option<(f64, f64, f64, f64)>;
    let long: String = (0..250).map(|i| format!("line {i:03}: rotate the staging key monthly\n")).collect();
    let notes: Vec<(NoteColor, &str, String, Geo, bool)> = vec![
        (NoteColor::Yellow, "Wi-Fi", "Guest network: hunter2hunter2\nRouter admin: see drawer 🔑".into(), Some((300.0, 200.0, 420.0, 320.0)), true),
        (NoteColor::DarkGreen, "Server keys", long, None, false),
        (NoteColor::Pink, "", String::new(), None, false),
        (NoteColor::Black, "Ünïcödé — 日本語 עברית", "line one\nline two\twith a tab\r\nwindows line".into(), Some((900.0, 40.0, 345.0, 250.0)), false),
        (NoteColor::Gray, "Second screen", "left of the main display".into(), Some((-1400.0, 300.0, 900.0, 900.0)), true),
    ];
    for (color, title, body, geo, pinned) in notes {
        let n = v.create_note(Some(color)).map_err(|e| e.to_string())?;
        let (x, y, w, h) = match geo {
            Some((x, y, w, h)) => (Some(x), Some(y), Some(w), Some(h)),
            None => (None, None, None, None),
        };
        v.update_note(&n.id, Some(title.into()), Some(body), None, x, y, w, h, Some(pinned)).map_err(|e| e.to_string())?;
    }
    // A note made and deleted: deletion must stick.
    let gone = v.create_note(Some(NoteColor::Purple)).map_err(|e| e.to_string())?;
    v.update_note(&gone.id, Some("deleted".into()), Some("must not come back".into()), None, None, None, None, None, None)
        .map_err(|e| e.to_string())?;
    v.delete_note(&gone.id).map_err(|e| e.to_string())?;
    v.set_idle_lock_secs(3600).map_err(|e| e.to_string())?;
    v.lock();
    println!("{recovery}");
    Ok(())
}

fn dump(path: PathBuf, secret: &str, recovery: bool) -> Result<(), String> {
    let mut v = Vault::open_path(path).map_err(|e| e.to_string())?;
    if recovery {
        v.unlock_with_recovery(secret).map_err(|e| e.to_string())?;
    } else {
        v.unlock(secret).map_err(|e| e.to_string())?;
    }
    let status = v.status();
    let mut notes = v.list_notes().map_err(|e| e.to_string())?;
    notes.sort_by(|a, b| a.id.cmp(&b.id));
    let notes: Vec<serde_json::Value> = notes
        .into_iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id, "title": n.title, "body": n.body, "color": n.color,
                "x": n.x, "y": n.y, "width": n.width, "height": n.height,
                "always_on_top": n.always_on_top,
            })
        })
        .collect();
    let out = serde_json::json!({
        "idle_lock_secs": status.idle_lock_secs,
        "has_recovery_key": status.has_recovery_key,
        "notes": notes,
    });
    println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
    v.lock();
    Ok(())
}
