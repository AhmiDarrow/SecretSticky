//! The tray icon (the shipped app's icon) and its menu, as the Tauri app had them.

use remedy_ui::host::{TrayEvent, TrayIcon, TrayItem};

use crate::state::{self, St};

const NEW_NOTE: u32 = 1;
const SHOW_MANAGER: u32 = 2;
const OPEN_ALL: u32 = 3;
const LOCK: u32 = 4;
const QUIT: u32 = 9;

/// The tray's menu, top to bottom, in `lang`.
pub fn menu(lang: crate::lang::Lang) -> Vec<TrayItem> {
    let item = |id, key: &str| TrayItem::item(id, lang.t(key));
    vec![
        item(NEW_NOTE, "tray.new"),
        // Bold where the platform shows a default: what clicking the icon does.
        TrayItem::Item { id: SHOW_MANAGER, label: lang.t("tray.show"), enabled: true, default: true },
        item(OPEN_ALL, "tray.openAll"),
        item(LOCK, "tray.lock"),
        TrayItem::Separator,
        item(QUIT, "tray.quit"),
    ]
}

/// Shows the tray icon with its tooltip and menu in the current language (again after a
/// language switch).
pub fn install_menu(st: &St) {
    let lang = st.lang;
    let (tip, items) = remedy_ui::reactive::untrack(|| (lang.t("tray.tooltip"), menu(lang)));
    if let Some(icon) = crate::brand::tray_icon(32) {
        st.app.set_tray(Some(TrayIcon::new(tip, icon, items)));
    }
}

/// Shows the tray icon and wires its menu.
pub fn install(st: &St) {
    install_menu(st);
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
