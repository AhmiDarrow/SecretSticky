//! The tray icon: a sticky note with a lock, and the app's menu.

use remedy_ui::host::{TrayEvent, TrayIcon, TrayImage, TrayItem};

use crate::state::{self, St};

const NEW_NOTE: u32 = 1;
const SHOW_MANAGER: u32 = 2;
const OPEN_ALL: u32 = 3;
const LOCK: u32 = 4;
const QUIT: u32 = 9;

/// Shows the tray icon and wires its menu.
pub fn install(st: &St) {
    let item = |id, label: &str| TrayItem::Item { id, label: label.to_owned(), enabled: true };
    st.app.set_tray(Some(TrayIcon {
        tooltip: "SecretSticky".to_owned(),
        icon: icon(32),
        menu: vec![
            item(NEW_NOTE, "New note"),
            item(SHOW_MANAGER, "Show manager"),
            item(OPEN_ALL, "Open all notes"),
            TrayItem::Separator,
            item(LOCK, "Lock vault"),
            TrayItem::Separator,
            item(QUIT, "Quit SecretSticky"),
        ],
    }));
    let st = st.clone();
    st.app.clone().on_tray(move |e| match e {
        TrayEvent::Activate | TrayEvent::Menu(SHOW_MANAGER) => state::show_manager(&st),
        TrayEvent::Menu(NEW_NOTE) => {
            if st.screen.get_untracked().ok() == Some(state::Screen::Unlocked) {
                state::new_note(&st, crate::vault::NoteColor::Yellow);
            } else {
                state::show_manager(&st);
            }
        }
        TrayEvent::Menu(OPEN_ALL) => {
            if st.screen.get_untracked().ok() == Some(state::Screen::Unlocked) {
                state::open_all(&st);
            } else {
                state::show_manager(&st);
            }
        }
        TrayEvent::Menu(LOCK) => state::lock_all(&st),
        TrayEvent::Menu(QUIT) => crate::manager::quit(&st),
        TrayEvent::Menu(_) => {}
    });
}

/// The icon at `size` pixels: a yellow note with a folded corner and a dark padlock,
/// antialiased by 4x4 supersampling. Premultiplied RGBA.
pub fn icon(size: u32) -> TrayImage {
    const NOTE: [f32; 3] = [1.0, 0.898, 0.4];
    const FOLD: [f32; 3] = [0.85, 0.74, 0.25];
    const LOCK: [f32; 3] = [0.09, 0.24, 0.17];
    let n = size as f32;
    // Shapes in a 0..1 square.
    let sample = |x: f32, y: f32| -> Option<[f32; 3]> {
        let fold = 0.28;
        let inside_note = (0.06..0.94).contains(&x) && (0.06..0.94).contains(&y) && x + y < 1.88 - fold;
        let in_fold = x + y >= 1.88 - fold && x < 0.94 && y < 0.94;
        // Padlock: a body and a shackle ring.
        let body = (0.30..0.70).contains(&x) && (0.46..0.78).contains(&y);
        let (dx, dy) = (x - 0.5, y - 0.46);
        let r = (dx * dx + dy * dy).sqrt();
        let shackle = y < 0.46 && (0.11..0.18).contains(&r);
        let keyhole = ((x - 0.5).powi(2) + (y - 0.6).powi(2)).sqrt() < 0.045 || ((0.485..0.515).contains(&x) && (0.6..0.7).contains(&y));
        if (body && !keyhole) || shackle {
            Some(LOCK)
        } else if inside_note || (body && keyhole) {
            Some(NOTE)
        } else if in_fold {
            Some(FOLD)
        } else {
            None
        }
    };
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut c, mut hits) = ([0.0f32; 3], 0u32);
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = (px as f32 + (sx as f32 + 0.5) / 4.0) / n;
                    let y = (py as f32 + (sy as f32 + 0.5) / 4.0) / n;
                    if let Some(s) = sample(x, y) {
                        hits += 1;
                        for (a, b) in c.iter_mut().zip(s) {
                            *a += b;
                        }
                    }
                }
            }
            let a = hits as f32 / 16.0;
            // Sum of colors over 16 samples is already premultiplied by coverage.
            for v in c {
                rgba.push((v / 16.0 * 255.0).round() as u8);
            }
            rgba.push((a * 255.0).round() as u8);
        }
    }
    TrayImage { size, rgba }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_icon_is_premultiplied_and_has_note_and_lock() {
        let img = super::icon(32);
        assert_eq!(img.rgba.len(), 32 * 32 * 4);
        let px: Vec<&[u8]> = img.rgba.chunks(4).collect();
        assert!(px.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]), "premultiplied");
        assert_eq!(px[0][3], 0, "corner is clear");
        let center = px[(20 * 32 + 12) as usize];
        assert!(center[3] == 255 && center[0] < 60, "lock body is dark: {center:?}");
        let note = px[(4 * 32 + 4) as usize];
        assert!(note[3] == 255 && note[0] > 200, "note is yellow: {note:?}");
    }
}
