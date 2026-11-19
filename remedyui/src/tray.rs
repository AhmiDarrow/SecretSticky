//! The tray icon (the shipped app's icon) and its menu, as the Tauri app had them.

use remedy_ui::host::{TrayEvent, TrayIcon, TrayItem};

use crate::state::{self, St};

const NEW_NOTE: u32 = 1;
const SHOW_MANAGER: u32 = 2;
const OPEN_ALL: u32 = 3;
const LOCK: u32 = 4;
const QUIT: u32 = 9;

/// What hovering the tray icon says.
pub const TOOLTIP: &str = "SecretSticky — encrypted sticky notes";

/// The tray's menu, top to bottom.
pub fn menu() -> Vec<TrayItem> {
    let item = |id, label: &str| TrayItem::Item { id, label: label.to_owned(), enabled: true };
    vec![
        item(NEW_NOTE, "New note"),
        item(SHOW_MANAGER, "Show manager"),
        item(OPEN_ALL, "Open all notes"),
        item(LOCK, "Lock vault"),
        TrayItem::Separator,
        item(QUIT, "Quit"),
    ]
}

/// Shows the tray icon and wires its menu.
pub fn install(st: &St) {
    if let Some(icon) = crate::brand::tray_icon(32) {
        st.app.set_tray(Some(TrayIcon::new(TOOLTIP, icon, menu())));
    }
    let st = st.clone();
    st.app.clone().on_tray(move |e| {
        let unlocked = st.screen.get_untracked().ok() == Some(state::Screen::Unlocked);
        match e {
            TrayEvent::Activate | TrayEvent::Menu(SHOW_MANAGER) => state::show_manager(&st),
            // Locked: the manager asks for the password first, as the Tauri app did.
            TrayEvent::Menu(NEW_NOTE) if unlocked => state::new_note(&st, crate::vault::NoteColor::default()),
            TrayEvent::Menu(OPEN_ALL) if unlocked => state::open_all(&st),
            TrayEvent::Menu(NEW_NOTE | OPEN_ALL) => state::show_manager(&st),
            TrayEvent::Menu(LOCK) => state::lock_all(&st),
            TrayEvent::Menu(QUIT) => state::quit(&st),
            TrayEvent::Menu(_) => {}
            _ => {}
        }
    });
}
