//! The app's state and the actions every window shares: unlock, lock, open and create notes.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use remedy_ui::reactive::Signal;
use remedy_ui::{App, Window};
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::lang::{Lang, Msg};
use crate::vault::{NoteColor, UnlockThrottle, Vault};

/// What the manager shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// No vault yet: choose a master password.
    Setup,
    /// Locked: enter the password or recovery key.
    Unlock,
    /// Just created: the recovery key, shown once.
    Recovery,
    /// Unlocked: notes and settings.
    Unlocked,
    /// The vault file could not be opened.
    Broken,
}

/// A note in the manager's list (no body).
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub id: String,
    pub title: String,
    pub color: NoteColor,
}

/// A title as the manager shows it ("Untitled note" when blank), tracked.
pub fn shown_title(lang: Lang, title: &str) -> String {
    let t = title.trim();
    if t.is_empty() { lang.t("notes.untitled") } else { t.to_owned() }
}

/// A title as a note window's title bar shows it ("Sticky note" when blank), tracked.
pub fn window_title(lang: Lang, title: &str) -> String {
    let t = title.trim();
    if t.is_empty() { lang.t("note.windowTitle") } else { t.to_owned() }
}

/// A color's name, as the pickers say it, tracked.
pub fn color_name(lang: Lang, c: &NoteColor) -> String {
    lang.t(COLORS.iter().find(|(k, _)| k == c).map_or("color.yellow", |(_, key)| key))
}

/// An open note window.
pub struct OpenNote {
    pub window: Window,
    /// Writes any unsaved edits now.
    pub flush: Rc<dyn Fn()>,
}

/// Everything the windows share. Lives as long as the app.
pub struct Sticky {
    pub app: App,
    vault: Option<Arc<Mutex<Vault>>>,
    /// Why the vault could not be opened (the vault's own message).
    pub load_error: String,
    /// The interface language.
    pub lang: Lang,
    /// The chosen font's id ([`crate::fonts::CHOICES`]).
    pub font: Signal<String>,
    /// The chosen text size's id ([`crate::settings::TEXT_SIZES`]).
    pub text_size: Signal<String>,
    /// Where `vault.json` and `settings.json` live.
    pub data_dir: std::path::PathBuf,
    pub throttle: RefCell<UnlockThrottle>,
    pub screen: Signal<Screen>,
    pub notes: Signal<Vec<Preview>>,
    pub idle_secs: Signal<u64>,
    pub has_recovery: Signal<bool>,
    pub busy: Signal<bool>,
    /// The last error worth showing in the manager.
    pub error: Signal<Option<Msg>>,
    pub recovery_key: RefCell<Option<Zeroizing<String>>>,
    pub manager: RefCell<Option<Window>>,
    pub open: RefCell<HashMap<String, OpenNote>>,
    locking: Cell<bool>,
}

pub type St = Rc<Sticky>;

/// Every note color, in the order the pickers show them.
/// Every note color with its catalog key, in the order the pickers show them.
pub const COLORS: [(NoteColor, &str); 8] = [
    (NoteColor::Yellow, "color.yellow"),
    (NoteColor::Green, "color.green"),
    (NoteColor::Pink, "color.pink"),
    (NoteColor::Blue, "color.blue"),
    (NoteColor::Purple, "color.purple"),
    (NoteColor::Gray, "color.gray"),
    (NoteColor::Black, "color.black"),
    (NoteColor::DarkGreen, "color.darkgreen"),
];

/// A note color as an engine color.
pub fn fill(c: &NoteColor) -> remedy_ui::core::Color {
    remedy_ui::core::Color::from_hex(c.as_css()).unwrap_or(remedy_ui::core::Color::WHITE)
}

/// Padding in CSS order: top, end, bottom, start.
pub fn pad(top: f32, end: f32, bottom: f32, start: f32) -> remedy_ui::core::Insets {
    remedy_ui::core::Insets { top, end, bottom, start }
}

/// Whether a note color is dark (light text on it).
pub fn is_dark(c: &NoteColor) -> bool {
    matches!(c, NoteColor::Black | NoteColor::DarkGreen)
}

/// The text color for a note color.
pub fn ink(c: &NoteColor) -> remedy_ui::core::Color {
    remedy_ui::core::Color::from_hex(c.text_css()).unwrap_or(remedy_ui::core::Color::BLACK)
}

impl Sticky {
    /// Runs `f` on the vault. Blocks while a background unlock holds it, so callers only
    /// use it when no unlock is running (the manager shows "Working…" meanwhile).
    pub fn with<T>(&self, f: impl FnOnce(&mut Vault) -> AppResult<T>) -> AppResult<T> {
        let Some(v) = &self.vault else { return Err(AppError::Message(self.load_error.clone())) };
        let mut g = v.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut g)
    }

    /// The vault, for work on a background thread.
    pub fn shared(&self) -> Option<Arc<Mutex<Vault>>> {
        self.vault.clone()
    }
}

/// `SECRETSTICKY_VAULT` points the app at another vault file (tests, a second vault).
pub fn vault_override() -> Option<std::path::PathBuf> {
    std::env::var_os("SECRETSTICKY_VAULT").map(Into::into)
}

/// `--home <dir>` or `SECRETSTICKY_HOME`: the folder that holds the vault and settings
/// (SecretSticky 0.2.0's portable mode).
pub fn home_override() -> Option<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    crate::vault::home_override_from(&args, std::env::var("SECRETSTICKY_HOME").ok().as_deref())
}

/// The folder for `settings.json` when no vault could be opened: the vault's folder as
/// `Vault::open_default` resolves it.
fn fallback_data_dir(vault_path: Option<&std::path::Path>) -> std::path::PathBuf {
    if let Some(dir) = vault_path.and_then(std::path::Path::parent) {
        return dir.to_owned();
    }
    home_override()
        .or_else(|| dirs::data_dir().map(|d| d.join("SecretSticky")))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Switches the interface language: saved to `settings.json` (the vault is not touched),
/// then every window, the tray and the engine's own words follow.
pub fn set_language(st: &St, id: &str) {
    match secretsticky_shipped_i18n::set_lang(&st.data_dir, id) {
        Ok(saved) => {
            st.app.set_locale(Some(saved.clone()));
            st.lang.0.set_if_changed(saved);
            crate::tray::install_menu(st);
        }
        Err(e) => st.error.set(Some(Msg::Error(e))),
    }
}

/// Saves `id` as the font and restyles every window with it at once.
pub fn set_font(st: &St, id: &str) {
    match crate::settings::save_font(&st.data_dir, id) {
        Ok(()) => {
            st.font.set_if_changed(id.to_owned());
            restyle(st);
        }
        Err(e) => st.error.set(Some(Msg::Error(e))),
    }
}

/// Saves `id` as the text size and resizes every window's text at once.
pub fn set_text_size(st: &St, id: &str) {
    match crate::settings::save_text_size(&st.data_dir, id) {
        Ok(()) => {
            st.text_size.set_if_changed(id.to_owned());
            restyle(st);
        }
        Err(e) => st.error.set(Some(Msg::Error(e))),
    }
}

/// Every window in the chosen font at the chosen size.
fn restyle(st: &St) {
    let (font, size) = remedy_ui::reactive::untrack(|| (st.font.get(), st.text_size.get()));
    st.app.set_theme(crate::theme::theme_with(crate::fonts::family(&font), crate::settings::text_scale(&size)));
}

/// Builds the app on the default vault, or the one `SECRETSTICKY_VAULT` names.
pub fn start(app: &App) {
    start_at(app, vault_override());
}

/// Builds the app: state, tray, manager window, second-launch and idle handling.
pub fn start_at(app: &App, vault_path: Option<std::path::PathBuf>) -> St {
    let opened = match &vault_path {
        Some(p) => Vault::open_path(p.clone()),
        None => Vault::open_default(),
    };
    let (vault, load_error, data_dir) = match opened {
        Ok(v) => {
            let dir = v.data_dir();
            (Some(Arc::new(Mutex::new(v))), String::new(), dir)
        }
        Err(e) => (None, e.to_string(), fallback_data_dir(vault_path.as_deref())),
    };
    // The saved language, or the system's on first run (written to settings.json).
    let lang = secretsticky_shipped_i18n::load_or_init_lang(&data_dir);
    app.set_locale(Some(lang.clone()));
    // The saved font and text size, where not the shipped ones, restyle every window.
    let font = crate::settings::load_font(&data_dir);
    let text_size = crate::settings::load_text_size(&data_dir);
    if font != crate::fonts::CHOICES[0].0 || text_size != "100" {
        app.set_theme(crate::theme::theme_with(crate::fonts::family(&font), crate::settings::text_scale(&text_size)));
    }
    let st: St = Rc::new(Sticky {
        app: app.clone(),
        vault,
        load_error,
        lang: Lang(Signal::new(lang)),
        font: Signal::new(font),
        text_size: Signal::new(text_size),
        data_dir,
        throttle: RefCell::new(UnlockThrottle::new()),
        screen: Signal::new(Screen::Unlock),
        notes: Signal::new(Vec::new()),
        idle_secs: Signal::new(crate::vault::DEFAULT_IDLE_LOCK_SECS),
        has_recovery: Signal::new(false),
        busy: Signal::new(false),
        error: Signal::new(None),
        recovery_key: RefCell::new(None),
        manager: RefCell::new(None),
        open: RefCell::new(HashMap::new()),
        locking: Cell::new(false),
    });
    // The tray keeps the app alive with every window hidden.
    app.keep_running(true);
    refresh(&st);
    crate::tray::install(&st);
    crate::manager::open(&st);
    let s = st.clone();
    app.on_second_instance(move |_| show_manager(&s));
    // Real input keeps the vault's own idle clock in step with the app's.
    let s = st.clone();
    app.on_activity(move || {
        if let Some(v) = &s.vault
            && let Ok(mut g) = v.try_lock()
        {
            g.touch();
        }
    });
    let s = st.clone();
    app.idle_lock(None, move || lock_all(&s));
    st
}

/// Reads the vault's status into the signals and picks the screen.
pub fn refresh(st: &St) {
    let status = match st.with(|v| Ok(v.status())) {
        Ok(s) => s,
        Err(_) => {
            st.screen.set_if_changed(Screen::Broken);
            return;
        }
    };
    st.idle_secs.set_if_changed(status.idle_lock_secs);
    st.has_recovery.set_if_changed(status.has_recovery_key);
    let screen = if !status.initialized {
        Screen::Setup
    } else if status.unlocked {
        if st.recovery_key.borrow().is_some() { Screen::Recovery } else { Screen::Unlocked }
    } else {
        Screen::Unlock
    };
    st.screen.set_if_changed(screen);
    if status.unlocked {
        reload_notes(st);
        st.app.set_idle_timeout(idle_timeout(status.idle_lock_secs));
    } else {
        st.notes.set_if_changed(Vec::new());
        st.app.set_idle_timeout(None);
    }
}

/// The idle timeout for a vault setting (0 is off).
pub fn idle_timeout(secs: u64) -> Option<Duration> {
    (secs > 0).then(|| Duration::from_secs(secs))
}

/// Re-reads the manager's note list.
pub fn reload_notes(st: &St) {
    if let Some(list) = checked(st, st.with(|v| v.list_note_previews())) {
        st.notes.set_if_changed(
            list.into_iter().map(|p| Preview { id: p.id, title: p.title, color: p.color }).collect(),
        );
    }
}

/// The value of `r`, or `None` after handling the error: a locked vault locks the UI, other
/// errors are shown in the manager.
pub fn checked<T>(st: &St, r: AppResult<T>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(AppError::Locked) => {
            lock_all(st);
            None
        }
        Err(e) => {
            st.error.set(Some(e.into()));
            None
        }
    }
}

/// Locks the vault: saves open notes, closes them, forgets the key and shows the unlock
/// screen.
pub fn lock_all(st: &St) {
    if st.locking.replace(true) {
        return;
    }
    let open: Vec<OpenNote> = st.open.borrow_mut().drain().map(|(_, n)| n).collect();
    for n in &open {
        (n.flush)();
    }
    let _ = st.with(|v| {
        v.lock();
        Ok(())
    });
    for n in open {
        n.window.close();
    }
    st.recovery_key.borrow_mut().take();
    // A secret copied from a note or the recovery key does not outlive the lock.
    st.app.clear_secret();
    st.locking.set(false);
    refresh(st);
    show_manager(st);
}

/// Saves open notes, clears a copied secret and quits (the tray stays until then).
pub fn quit(st: &St) {
    let flushes: Vec<_> = st.open.borrow().values().map(|n| n.flush.clone()).collect();
    for f in flushes {
        f();
    }
    st.app.clear_secret();
    st.app.quit();
}

/// Hides the manager to the tray.
pub fn hide_manager(st: &St) {
    if let Some(w) = st.manager.borrow().as_ref() {
        w.set_visible(false);
    }
}

/// Shows and focuses the manager, opening it if it was closed.
pub fn show_manager(st: &St) {
    let w = st.manager.borrow().clone();
    match w {
        Some(w) if w.is_open() => {
            w.set_visible(true);
            w.set_minimized(false);
            w.focus();
        }
        _ => crate::manager::open(st),
    }
}

/// Opens a note's window, or brings it forward when it is open.
pub fn open_note(st: &St, id: &str) {
    let open = st.open.borrow().get(id).map(|n| n.window.clone());
    if let Some(w) = open {
        w.set_visible(true);
        w.set_minimized(false);
        w.focus();
    } else if let Some(dto) = checked(st, st.with(|v| v.get_note(id))) {
        crate::note::open(st, dto);
    } else {
        return;
    }
    // A sticky takes over from the manager, which waits in the tray (as the Tauri app did).
    hide_manager(st);
}

/// Opens every note.
pub fn open_all(st: &St) {
    let ids: Vec<String> = st.notes.get_untracked().unwrap_or_default().into_iter().map(|p| p.id).collect();
    for id in ids {
        open_note(st, &id);
    }
}

/// Creates a note and opens it.
pub fn new_note(st: &St, color: NoteColor) {
    if let Some(dto) = checked(st, st.with(|v| v.create_note(Some(color)))) {
        reload_notes(st);
        crate::note::open(st, dto);
        hide_manager(st);
    }
}

/// Deletes a note, closing its window without saving.
pub fn delete_note(st: &St, id: &str) {
    let open = st.open.borrow_mut().remove(id);
    if let Some(n) = open {
        n.window.close();
    }
    if checked(st, st.with(|v| v.delete_note(id))).is_some() {
        reload_notes(st);
    }
}

/// Runs `f` after `delay` unless called again first (only the last call runs).
#[derive(Clone, Default)]
pub struct Debounce(Rc<Cell<u64>>);

impl Debounce {
    pub fn call(&self, app: &App, delay: Duration, f: impl FnOnce() + 'static) {
        let r#gen = self.0.get().wrapping_add(1);
        self.0.set(r#gen);
        let me = self.0.clone();
        app.after(delay, move || {
            if me.get() == r#gen {
                f();
            }
        });
    }

    /// Drops a pending call.
    pub fn cancel(&self) {
        self.0.set(self.0.get().wrapping_add(1));
    }
}
