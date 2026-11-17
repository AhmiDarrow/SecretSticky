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
    pub load_error: String,
    pub throttle: RefCell<UnlockThrottle>,
    pub screen: Signal<Screen>,
    pub notes: Signal<Vec<Preview>>,
    pub idle_secs: Signal<u64>,
    pub has_recovery: Signal<bool>,
    pub busy: Signal<bool>,
    /// The last error worth showing in the manager.
    pub error: Signal<Option<String>>,
    pub recovery_key: RefCell<Option<Zeroizing<String>>>,
    pub manager: RefCell<Option<Window>>,
    pub open: RefCell<HashMap<String, OpenNote>>,
    locking: Cell<bool>,
}

pub type St = Rc<Sticky>;

/// Every note color, in the order the pickers show them.
pub const COLORS: [(NoteColor, &str); 8] = [
    (NoteColor::Yellow, "Yellow"),
    (NoteColor::Green, "Green"),
    (NoteColor::Pink, "Pink"),
    (NoteColor::Blue, "Blue"),
    (NoteColor::Purple, "Purple"),
    (NoteColor::Gray, "Gray"),
    (NoteColor::Black, "Black"),
    (NoteColor::DarkGreen, "Dark green"),
];

/// A note color as an engine color.
pub fn fill(c: &NoteColor) -> remedy_ui::core::Color {
    remedy_ui::core::Color::from_hex(c.as_css()).unwrap_or(remedy_ui::core::Color::WHITE)
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

/// Builds the app on the default vault, or the one `SECRETSTICKY_VAULT` names.
pub fn start(app: &App) {
    start_at(app, vault_override());
}

/// Builds the app: state, tray, manager window, second-launch and idle handling.
pub fn start_at(app: &App, vault_path: Option<std::path::PathBuf>) -> St {
    let opened = match vault_path {
        Some(p) => Vault::open_path(p),
        None => Vault::open_default(),
    };
    let (vault, load_error) = match opened {
        Ok(v) => (Some(Arc::new(Mutex::new(v))), String::new()),
        Err(e) => (None, format!("The vault could not be opened: {e}")),
    };
    let st: St = Rc::new(Sticky {
        app: app.clone(),
        vault,
        load_error,
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
            st.error.set(Some(e.to_string()));
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
    st.locking.set(false);
    refresh(st);
    show_manager(st);
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
    if let Some(n) = st.open.borrow().get(id) {
        n.window.set_visible(true);
        n.window.focus();
        return;
    }
    if let Some(dto) = checked(st, st.with(|v| v.get_note(id))) {
        crate::note::open(st, dto);
    }
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
