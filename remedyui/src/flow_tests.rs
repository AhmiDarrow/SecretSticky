//! The whole app on the headless host, driven the way a screen reader would: by the
//! accessibility tree's labels and actions. Every test uses a vault in a temporary directory;
//! none opens the person's own vault.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use remedy_ui::host::accesskit::{Action, ActionRequest, Node, NodeId, TreeId};
use remedy_ui::host::{AccessibilityEvent, Host, HostEvent, TrayEvent, WindowId};
use remedy_ui::input::{Key, KeyEvent, Modifiers};
use remedy_ui::{App, AppConfig, GeometryStore, Runner};
use remedy_ui_bridge_headless::HeadlessHost;

use crate::state::{Screen, St};

const PW: &str = "correct horse battery staple";
const COPY: &str = "Copy selection or full body (auto-clears in 30s)";

struct Driver {
    host: HeadlessHost,
    runner: Runner,
    st: St,
    trees: HashMap<WindowId, HashMap<NodeId, Node>>,
    roots: HashMap<WindowId, NodeId>,
}

impl Driver {
    /// Starts the app on `vault` and reads its first frames.
    fn new(vault: std::path::PathBuf) -> Self {
        Self::start(move |app| crate::state::start_at(app, Some(vault)))
    }

    /// Starts the app as `main` does: on `SECRETSTICKY_VAULT` (tests always set it).
    fn from_env() -> Self {
        assert!(crate::state::vault_override().is_some(), "tests never open the person's own vault");
        Self::start(|app| crate::state::start_at(app, crate::state::vault_override()))
    }

    fn start(start: impl FnOnce(&App) -> St + 'static) -> Self {
        let slot: Rc<RefCell<Option<St>>> = Rc::default();
        let s2 = slot.clone();
        let config = crate::fonts::with_inter(AppConfig::new("SecretSticky test").cpu_only().theme(crate::theme::theme(), crate::theme::sheet));
        let mut runner = Runner::with_geometry(config, GeometryStore::in_memory(), move |app: &App| {
            *s2.borrow_mut() = Some(start(app));
        });
        let mut host = HeadlessHost::new();
        host.run_frames(&mut runner, 1);
        let st = slot.borrow().clone().expect("the app started");
        let mut d = Driver { host, runner, st, trees: HashMap::new(), roots: HashMap::new() };
        d.frames(2);
        d
    }

    fn frames(&mut self, n: usize) {
        self.host.run_frames(&mut self.runner, n);
        for id in self.host.window_ids() {
            let tree = self.trees.entry(id).or_default();
            if tree.is_empty() {
                self.host.send(HostEvent::Accessibility(id, AccessibilityEvent::Requested));
                self.host.run_frames(&mut self.runner, 1);
            }
            for u in self.host.take_accessibility(id) {
                if let Some(t) = &u.tree {
                    self.roots.insert(id, t.root);
                }
                for (nid, node) in u.nodes {
                    tree.insert(nid, node);
                }
            }
            // Like a screen reader: only what hangs off the root is still there.
            if let Some(root) = self.roots.get(&id).copied() {
                let mut live = std::collections::HashSet::new();
                let mut todo = vec![root];
                while let Some(n) = todo.pop() {
                    if live.insert(n)
                        && let Some(node) = tree.get(&n)
                    {
                        todo.extend(node.children().iter().copied());
                    }
                }
                tree.retain(|k, _| live.contains(k));
            }
        }
    }

    /// Runs frames until `done`, letting background work (Argon2) finish.
    fn until(&mut self, what: &str, done: impl Fn(&Driver) -> bool) {
        for _ in 0..3000 {
            self.frames(1);
            if done(self) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {what}: screen {:?}, error {:?}", self.screen(), self.st.error.get_untracked());
    }

    fn windows(&self) -> Vec<WindowId> {
        self.host.window_ids()
    }

    /// The node labelled `label` in `window` that offers `action`.
    fn node(&self, window: WindowId, label: &str, action: Action) -> NodeId {
        let tree = self.trees.get(&window).expect("a tree for the window");
        let found = tree.iter().find(|(_, n)| n.label() == Some(label) && n.supports_action(action));
        match found {
            Some((id, _)) => *id,
            None => {
                let labels: Vec<_> = tree.values().filter_map(|n| n.label()).collect();
                panic!("no {label:?} with {action:?} in {labels:?}")
            }
        }
    }

    /// The engine's accessibility check finds nothing in the window.
    fn assert_accessible(&self, ui: &remedy_ui::Ui) {
        let found: Vec<String> = remedy_ui_devtools::audit(ui)
            .iter()
            .map(|p| format!("{} [{}]", p.message(), ui.element(p.element).info().map(|i| remedy_ui_devtools::describe(&i)).unwrap_or_default()))
            .collect();
        assert!(found.is_empty(), "{found:#?}");
    }

    fn manager_ui(&self) -> remedy_ui::Ui {
        self.st.manager.borrow().as_ref().and_then(|w| w.ui()).unwrap_or_else(|| panic!("the manager is open"))
    }

    fn note_ui(&self) -> remedy_ui::Ui {
        self.st.open.borrow().values().next().and_then(|n| n.window.ui()).unwrap_or_else(|| panic!("a note is open"))
    }

    /// Every control assistive technology can press or focus has a name.
    fn assert_labelled(&self, window: WindowId) {
        let tree = self.trees.get(&window).expect("a tree");
        let controls: Vec<&Node> =
            tree.values().filter(|n| n.supports_action(Action::Click) || n.supports_action(Action::Focus)).collect();
        assert!(controls.len() >= 4, "only {} controls seen", controls.len());
        let nameless: Vec<String> = controls
            .iter()
            .filter(|n| n.label().is_none_or(|l| l.trim().is_empty()))
            .map(|n| format!("{:?}", n.role()))
            .collect();
        assert!(nameless.is_empty(), "unnamed controls: {nameless:?}");
    }

    fn has(&self, window: WindowId, label: &str) -> bool {
        self.trees.get(&window).is_some_and(|t| t.values().any(|n| n.label() == Some(label)))
    }

    fn act(&mut self, window: WindowId, label: &str, action: Action) {
        let target = self.node(window, label, action);
        let req = ActionRequest { action, target_tree: TreeId::ROOT, target_node: target, data: None };
        self.host.send(HostEvent::Accessibility(window, AccessibilityEvent::Action(req)));
        self.frames(2);
    }

    fn click(&mut self, window: WindowId, label: &str) {
        self.act(window, label, Action::Click);
    }

    /// Presses `letter`, with Ctrl when `ctrl`, in `window`.
    fn key(&mut self, window: WindowId, letter: &str, ctrl: bool) {
        let e = KeyEvent::press(Key::Character(letter.into()))
            .with_physical(format!("Key{}", letter.to_uppercase()))
            .with_modifiers(Modifiers { ctrl, ..Modifiers::NONE })
            .with_text(if ctrl { None } else { Some(letter.into()) });
        self.host.send(HostEvent::Key(window, e));
        self.frames(2);
    }

    /// Types into a field by focusing it and pasting (what a password manager does).
    fn type_into(&mut self, window: WindowId, label: &str, text: &str) {
        self.act(window, label, Action::Focus);
        let _ = self.host.set_clipboard_text(text, true);
        self.key(window, "v", true);
    }

    /// Replaces a field's whole text (select all, then paste).
    fn replace_into(&mut self, window: WindowId, label: &str, text: &str) {
        self.act(window, label, Action::Focus);
        self.key(window, "a", true);
        let _ = self.host.set_clipboard_text(text, true);
        self.key(window, "v", true);
    }

    /// What a field holds, as assistive technology reads it.
    fn value(&self, window: WindowId, label: &str) -> Option<String> {
        let tree = self.trees.get(&window)?;
        tree.values()
            .find(|n| n.label() == Some(label) && n.supports_action(Action::Focus))
            .and_then(|n| n.value().map(str::to_owned))
    }

    /// The open note window titled `title`.
    fn note_window(&self, title: &str) -> WindowId {
        self.windows()
            .into_iter()
            .find(|w| self.host.window(*w).is_some_and(|h| h.options().title == title && !h.options().decorated))
            .unwrap_or_else(|| panic!("a note window titled {title}"))
    }

    fn manager_visible(&self) -> bool {
        self.st.manager.borrow().as_ref().is_some_and(remedy_ui::Window::is_visible)
    }

    /// Brings the manager back (notes hide it), as the tray icon does.
    fn show_manager(&mut self) {
        crate::state::show_manager(&self.st);
        self.frames(2);
    }

    fn screen(&self) -> Screen {
        self.st.screen.get_untracked().expect("screen signal")
    }
}

#[test]
fn set_up_write_a_note_lock_and_unlock_again() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("vault.json");
    let mut d = Driver::new(path.clone());
    let manager = d.windows()[0];
    assert_eq!(d.screen(), Screen::Setup);
    d.assert_labelled(manager);
    d.assert_accessible(&d.manager_ui());
    assert!(d.host.window(manager).is_some_and(|w| w.options().capture_protected), "manager hidden from capture");
    assert!(d.host.tray().is_some_and(|t| t.tooltip == crate::tray::TOOLTIP), "tray icon shown");
    assert_eq!(d.host.tray().map(|t| t.menu.clone()), Some(crate::tray::menu()), "the shipped tray menu");

    // Setup refuses short and mismatched passwords.
    d.type_into(manager, "Master password", "short");
    d.type_into(manager, "Confirm password", "short");
    d.click(manager, "Create vault");
    assert_eq!(d.st.error.get_untracked().ok().flatten().as_deref(), Some("Use at least 12 characters"));
    d.type_into(manager, "Master password", PW);
    d.type_into(manager, "Confirm password", "something else entirely");
    d.click(manager, "Create vault");
    assert_eq!(d.st.error.get_untracked().ok().flatten().as_deref(), Some("Passwords do not match"));
    d.type_into(manager, "Master password", PW);
    d.type_into(manager, "Confirm password", PW);
    d.click(manager, "Create vault");
    d.until("the recovery key", |d| d.screen() == Screen::Recovery);
    d.frames(2);
    let key = d.st.recovery_key.borrow().as_ref().map(|k| k.to_string()).expect("a recovery key");
    assert!(d.has(manager, "Save your recovery key"));
    d.click(manager, "Copy key (2 min) — continue");
    assert_eq!(d.host.clipboard(), Some((key.as_str(), true)), "copied as sensitive");
    assert_eq!(d.screen(), Screen::Unlocked);
    assert!(d.st.recovery_key.borrow().is_none(), "the key is forgotten");
    d.host.advance(Duration::from_secs(121));
    d.frames(1);
    assert!(d.host.clipboard().is_none_or(|(t, _)| t.is_empty()), "the recovery key leaves the clipboard after 2 minutes");

    // A new note opens frameless and protected; the manager steps back to the tray.
    d.frames(2);
    assert!(d.has(manager, "No notes yet — pick a color to create one."));
    d.click(manager, "New yellow note");
    let note = *d.windows().iter().find(|w| **w != manager).expect("a note window");
    let opts = d.host.window(note).map(|w| w.options().clone()).expect("note options");
    assert!(!opts.decorated && opts.capture_protected, "frameless and capture protected");
    assert!(!opts.always_on_top, "new notes are not pinned");
    assert_eq!(opts.title, "Sticky note");
    assert!(!d.manager_visible(), "a sticky hides the manager");
    d.frames(2);
    d.assert_labelled(note);
    d.assert_accessible(&d.note_ui());
    d.type_into(note, "Title", "Launch");
    d.type_into(note, "Note", "the launch code is 0000");
    d.host.advance(Duration::from_millis(500));
    d.frames(2);
    let saved = std::fs::read_to_string(&path).expect("vault file");
    assert!(!saved.contains("launch code"), "nothing plain on disk");
    let notes = d.st.with(|v| v.list_notes()).expect("notes");
    assert_eq!(notes.len(), 1);
    assert_eq!((notes[0].title.as_str(), notes[0].body.as_str()), ("Launch", "the launch code is 0000"));
    assert_eq!(d.host.window(note).map(|w| w.options().title.clone()).as_deref(), Some("Launch"), "the title bar follows");

    // Copy from the note goes to the secret clipboard and clears itself.
    d.click(note, COPY);
    assert_eq!(d.host.clipboard(), Some(("the launch code is 0000", true)));
    assert!(d.has(note, "Copied body · clears in 30s"));
    d.host.advance(Duration::from_secs(31));
    d.frames(1);
    assert!(d.host.clipboard().is_none_or(|(t, _)| t.is_empty()), "cleared after 30 s");
    // Ctrl+C in the text copies the selection the same way.
    d.act(note, "Note", Action::Focus);
    d.key(note, "a", true);
    d.key(note, "c", true);
    assert_eq!(d.host.clipboard(), Some(("the launch code is 0000", true)), "Ctrl+C is a secret copy");
    assert!(d.has(note, "Copied · clears in 30s"), "and says so");
    assert!(d.host.window(note).is_some_and(|w| w.options().icon.is_some()), "the note shows the app's icon");
    assert!(d.host.window(manager).is_some_and(|w| w.options().icon.is_some()), "so does the manager");

    // Color and pin are saved with the note.
    d.click(note, "Dark green");
    d.click(note, "Always on top");
    d.host.advance(Duration::from_millis(500));
    d.frames(2);
    assert!(d.host.window(note).is_some_and(|w| w.options().always_on_top), "pinned on top");
    let n = d.st.with(|v| v.list_notes()).expect("notes").remove(0);
    assert_eq!((n.color, n.always_on_top), (crate::vault::NoteColor::DarkGreen, true));

    // Lock from the note: notes close, the clipboard clears, the manager asks again.
    d.click(note, "Lock vault");
    d.frames(2);
    assert_eq!(d.screen(), Screen::Unlock);
    assert_eq!(d.windows(), vec![manager], "the note window closed");
    assert!(d.manager_visible(), "the manager comes back to unlock");
    assert!(d.host.clipboard().is_none_or(|(t, _)| t.is_empty()), "lock clears a copied secret");

    // A wrong password is refused; the recovery key opens the vault with the note.
    d.type_into(manager, "Master password", "not the password!!");
    d.click(manager, "Unlock");
    d.until("the refusal", |d| d.st.error.get_untracked().ok().flatten().is_some());
    assert_eq!(d.screen(), Screen::Unlock);
    d.click(manager, "Use recovery key");
    d.type_into(manager, "Recovery key", &key);
    d.click(manager, "Unlock");
    d.until("unlock", |d| d.screen() == Screen::Unlocked);
    d.frames(2);
    assert!(d.has(manager, "Open Launch"), "listed");
    assert!(d.has(manager, "1 note · local vault"));

    // The tray opens every note; idle locks.
    d.host.send(HostEvent::Tray(TrayEvent::Menu(3)));
    d.frames(2);
    assert_eq!(d.windows().len(), 2, "open all");
    d.host.advance(Duration::from_secs(16 * 60));
    d.frames(2);
    assert_eq!(d.screen(), Screen::Unlock, "idle lock after 15 minutes");
    assert_eq!(d.windows(), vec![manager]);
}

/// The manager's header panels, list actions and footer, as the shipped app has them.
#[test]
fn every_manager_action() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut d = Driver::new(dir.path().join("vault.json"));
    let manager = d.windows()[0];
    d.type_into(manager, "Master password", PW);
    d.type_into(manager, "Confirm password", PW);
    d.click(manager, "Create vault");
    d.until("the recovery key", |d| d.screen() == Screen::Recovery);
    d.frames(2);
    d.click(manager, "I already saved it — continue");
    d.frames(2);
    d.assert_accessible(&d.manager_ui());

    // About: version, links, updates.
    d.click(manager, "About");
    assert!(d.has(manager, &format!("Version {}", env!("CARGO_PKG_VERSION"))));
    for (label, url) in crate::manager::LINKS {
        d.click(manager, label);
        assert!(d.host.opened_urls().iter().any(|u| u == url), "{label} opens {url}");
    }
    assert!(d.has(manager, "Check for updates"));
    d.assert_accessible(&d.manager_ui());
    d.click(manager, "Close");
    assert!(!d.has(manager, "Releases"), "About closed");

    // Auto-lock: ten presets; the vault keeps the choice.
    d.click(manager, "Auto-lock");
    assert!(d.has(manager, "Auto-lock now 15 min"));
    for (_, label) in crate::manager::AUTO_LOCK {
        assert!(d.has(manager, label), "preset {label}");
    }
    d.assert_accessible(&d.manager_ui());
    d.click(manager, "2 hr");
    assert_eq!(d.st.with(|v| Ok(v.status().idle_lock_secs)).ok(), Some(7200));
    assert!(d.has(manager, "Auto-lock now 2 hrs"));
    d.click(manager, "Off");
    assert_eq!(d.st.with(|v| Ok(v.status().idle_lock_secs)).ok(), Some(0));
    d.click(manager, "Done");

    // Password: refuses a short one, then changes it.
    d.click(manager, "Password");
    d.assert_accessible(&d.manager_ui());
    d.type_into(manager, "Current password", PW);
    d.type_into(manager, "New password", "short");
    d.type_into(manager, "Confirm new password", "short");
    d.click(manager, "Update password");
    assert!(d.has(manager, "New password must be at least 12 characters."));
    let new_pw = "a brand new master password";
    d.type_into(manager, "Current password", PW);
    d.type_into(manager, "New password", new_pw);
    d.type_into(manager, "Confirm new password", new_pw);
    d.click(manager, "Update password");
    d.until("the change", |d| d.has(manager, crate::manager::PASSWORD_CHANGED));

    // Notes: create two, delete one through the confirmation.
    let before = d.windows();
    d.click(manager, "New pink note");
    let pink_win = *d.windows().iter().find(|w| !before.contains(w)).expect("the pink note's window");
    d.type_into(pink_win, "Title", "Pink one");
    d.host.advance(Duration::from_millis(500));
    d.frames(2);
    d.show_manager();
    d.click(manager, "New blue note");
    d.show_manager();
    assert!(d.has(manager, "2 notes · local vault"));
    let pink = d.st.notes.get_untracked().expect("notes").into_iter().find(|p| p.color == crate::vault::NoteColor::Pink);
    let pink = pink.expect("the pink note");
    d.click(manager, &format!("Delete {}", pink.shown_title()));
    assert!(d.has(manager, "Delete note?"));
    d.click(manager, "Cancel");
    assert_eq!(Some(d.st.notes.with(Vec::len)), Some(2), "cancel keeps it");
    d.click(manager, &format!("Delete {}", pink.shown_title()));
    d.click(manager, "Delete");
    assert_eq!(Some(d.st.notes.with(Vec::len)), Some(1), "deleted");
    assert!(d.st.with(|v| v.get_note(&pink.id)).is_err());
    assert_eq!(d.windows().len(), 2, "its window closed with it");

    // Hide to tray; the tray brings the manager back; lock, unlock, quit.
    d.click(manager, "Hide to tray");
    assert!(!d.manager_visible());
    d.host.send(HostEvent::Tray(TrayEvent::Activate));
    d.frames(2);
    assert!(d.manager_visible());
    d.click(manager, "Lock");
    assert_eq!(d.screen(), Screen::Unlock);
    d.type_into(manager, "Master password", new_pw);
    d.click(manager, "Unlock");
    d.until("unlock with the new password", |d| d.screen() == Screen::Unlocked);
    d.frames(2);
    d.click(manager, "Quit");
    assert!(d.host.has_exited(), "quit");
}

/// Vaults written by shipped releases (0.1.0, the first, and 0.1.8, the last Tauri release)
/// open in this build through `SECRETSTICKY_VAULT`, show every note as it was, take edits,
/// and read back afterwards through the shipped vault code with nothing lost. The fixtures
/// and their expected contents were made by `compat/shipped.sh` with the release tags' own
/// vault code.
#[test]
fn shipped_vaults_open_edit_and_reopen_without_loss() {
    for (tag, pw) in [("v0.1.0", "old-pass9"), ("v0.1.8", PW)] {
        shipped_vault_round_trip(tag, pw);
    }
}

fn shipped_vault_round_trip(tag: &str, pw: &str) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("vault.json");
    std::fs::copy(fixtures.join(format!("vault-{tag}.json")), &path).expect("copy the fixture");
    let recovery = std::fs::read_to_string(fixtures.join(format!("{tag}.recovery"))).expect("recovery key");
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixtures.join(format!("vault-{tag}.expected.json"))).expect("expected"))
            .expect("expected json");
    let notes_want = expected["notes"].as_array().cloned().expect("notes");
    let want = |id: &str| notes_want.iter().find(|n| n["id"] == id).cloned();

    // The app finds the vault through the environment, as a second vault would.
    // SAFETY: this is the only test that reads or writes SECRETSTICKY_VAULT, and the
    // variable is set and read on this thread before the app starts.
    unsafe { std::env::set_var("SECRETSTICKY_VAULT", &path) };
    let mut d = Driver::from_env();
    unsafe { std::env::remove_var("SECRETSTICKY_VAULT") };
    let manager = d.windows()[0];
    assert_eq!(d.screen(), Screen::Unlock, "{tag}: an existing vault asks for its password");
    d.type_into(manager, "Master password", pw);
    d.click(manager, "Unlock");
    d.until("unlock", |d| d.screen() == Screen::Unlocked);
    d.frames(2);
    assert_eq!(d.st.idle_secs.get_untracked().ok(), Some(3600), "{tag}: auto-lock setting kept");
    for n in &notes_want {
        let title = crate::state::shown_title(n["title"].as_str().unwrap_or_default());
        assert!(d.has(manager, &format!("Open {title}")), "{tag}: {title} listed");
    }

    // Every note opens with its text and pin as saved.
    d.click(manager, "Open all");
    d.frames(3);
    let windows: Vec<_> = d.windows().into_iter().filter(|w| *w != manager).collect();
    assert_eq!(windows.len(), notes_want.len(), "{tag}: every note opens");
    for w in &windows {
        let (title, pinned) = d.host.window(*w).map(|h| (h.options().title.clone(), h.options().always_on_top)).expect("w");
        let n = notes_want
            .iter()
            .find(|n| crate::state::window_title(n["title"].as_str().unwrap_or_default()) == title)
            .unwrap_or_else(|| panic!("{tag}: a note titled {title}"));
        assert_eq!(pinned, n["always_on_top"].as_bool().unwrap_or(false), "{tag}: {title} pin");
        let body = d.value(*w, "Note").unwrap_or_default();
        assert_eq!(body, n["body"].as_str().unwrap_or_default(), "{tag}: {title} body");
    }

    // Edits: Wi-Fi gets new text, a color and loses its pin; "Second screen" is deleted; a
    // new note is made; auto-lock changes; the password changes.
    let wifi_id = notes_want.iter().find(|n| n["title"] == "Wi-Fi").and_then(|n| n["id"].as_str()).expect("wifi").to_owned();
    let wifi = d.note_window("Wi-Fi");
    d.replace_into(wifi, "Note", "Guest network changed: correct-horse-2026");
    d.click(wifi, "Blue");
    d.click(wifi, "Unpin");
    d.host.advance(Duration::from_millis(500));
    d.frames(2);
    d.click(wifi, "Close");
    d.show_manager();
    d.click(manager, "Delete Second screen");
    d.click(manager, "Delete");
    let before = d.windows();
    d.click(manager, "New purple note");
    let new_note = *d.windows().iter().find(|w| !before.contains(w)).expect("the new note's window");
    d.type_into(new_note, "Title", "Made in RemedyUI");
    d.type_into(new_note, "Note", "new body");
    d.host.advance(Duration::from_millis(500));
    d.frames(2);
    d.show_manager();
    d.click(manager, "Auto-lock");
    d.click(manager, "2 hr");
    d.click(manager, "Done");
    let new_pw = "the new master password 2026";
    d.click(manager, "Password");
    d.type_into(manager, "Current password", pw);
    d.type_into(manager, "New password", new_pw);
    d.type_into(manager, "Confirm new password", new_pw);
    d.click(manager, "Update password");
    d.until("the change", |d| d.has(manager, crate::manager::PASSWORD_CHANGED));
    d.click(manager, "Lock");
    assert_eq!(d.screen(), Screen::Unlock);
    drop(d);

    // Read back with the shipped vault code (src-tauri/src/vault.rs, the module the Tauri
    // app runs), by the new password and by the original recovery key.
    for by_recovery in [false, true] {
        let mut v = crate::vault::Vault::open_path(path.clone()).expect("the shipped code opens it");
        if by_recovery {
            v.unlock_with_recovery(recovery.trim()).expect("the recovery key still opens it");
        } else {
            v.unlock(new_pw).expect("the new password opens it");
        }
        assert_eq!(v.status().idle_lock_secs, 7200, "{tag}: auto-lock saved");
        let after = v.list_notes().expect("notes");
        assert_eq!(after.len(), notes_want.len(), "{tag}: one deleted, one added");
        for n in &after {
            if n.id == wifi_id {
                assert_eq!(n.body, "Guest network changed: correct-horse-2026");
                assert_eq!((n.color.clone(), n.always_on_top), (crate::vault::NoteColor::Blue, false));
                assert_eq!(n.title, "Wi-Fi");
            } else if n.title == "Made in RemedyUI" {
                assert_eq!((n.body.as_str(), n.color.clone()), ("new body", crate::vault::NoteColor::Purple));
            } else {
                // Untouched notes are exactly as the shipped release wrote them.
                let w = want(&n.id).unwrap_or_else(|| panic!("{tag}: unexpected note {}", n.title));
                assert_ne!(w["title"], "Second screen", "{tag}: the deleted note stays deleted");
                assert_eq!(n.title, w["title"].as_str().unwrap_or_default(), "{tag}: title");
                assert_eq!(n.body, w["body"].as_str().unwrap_or_default(), "{tag}: body of {}", n.title);
                assert_eq!(serde_json::to_value(&n.color).ok(), Some(w["color"].clone()), "{tag}: color");
                assert_eq!(n.always_on_top, w["always_on_top"].as_bool().unwrap_or(false), "{tag}: pin");
                for (k, got) in [("x", n.x), ("y", n.y), ("width", n.width), ("height", n.height)] {
                    assert_eq!(Some(got), w[k].as_f64(), "{tag}: {k} of {}", n.title);
                }
            }
        }
        v.lock();
    }
    // For a cross-check with a release's own vault code (`compat/shipped.sh <tag> dump`).
    if let Some(out) = std::env::var_os("SECRETSTICKY_COMPAT_OUT") {
        let out = std::path::PathBuf::from(out);
        let _ = std::fs::create_dir_all(&out);
        let _ = std::fs::copy(&path, out.join(format!("after-{tag}.json")));
    }
}

/// Minimal PNG writer (stored deflate) for looking at frames.
fn png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend(crc(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for row in rgba.chunks((w * 4) as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    let _ = std::fs::write(path, out);
}

/// Writes every screen as PNGs to `SECRETSTICKY_SHOTS` (run with `--ignored`): the "after"
/// evidence beside the shipped app's in `port-evidence/before`.
#[test]
#[ignore = "writes images for a person to look at"]
fn screenshots() {
    let Some(out) = std::env::var_os("SECRETSTICKY_SHOTS").map(std::path::PathBuf::from) else { return };
    let _ = std::fs::create_dir_all(&out);
    let dir = tempfile::tempdir().expect("temp dir");
    let mut d = Driver::new(dir.path().join("vault.json"));
    let shot = |d: &mut Driver, w: WindowId, name: &str, size: Option<(u32, u32)>| {
        if let Some((pw, ph)) = size {
            d.host.user_resize(w, remedy_ui::core::Size::new(pw as f32, ph as f32));
            d.frames(3);
        }
        let win = d.host.window(w).expect("window");
        let (pw, ph) = (win.size().0, win.size().1);
        png(&out.join(format!("{name}.png")), pw, ph, win.surface());
    };
    let manager = d.windows()[0];
    shot(&mut d, manager, "01-setup", None);
    d.type_into(manager, "Master password", "short");
    d.type_into(manager, "Confirm password", "short");
    d.click(manager, "Create vault");
    shot(&mut d, manager, "01-setup-short-password", None);
    d.type_into(manager, "Master password", PW);
    d.type_into(manager, "Confirm password", PW);
    d.click(manager, "Create vault");
    d.until("recovery", |d| d.screen() == Screen::Recovery);
    d.frames(3);
    shot(&mut d, manager, "02-recovery-key", None);
    d.click(manager, "I already saved it — continue");
    d.frames(3);
    shot(&mut d, manager, "04-manager-empty", None);
    let samples = [
        ("New yellow note", "Wi-Fi", "Guest network: hunter2hunter2\nRouter admin: see drawer"),
        ("New dark green note", "Server keys", "staging api key: sk-sample-0000\nrotate monthly"),
        ("New pink note", "", ""),
        ("New blue note", "Bank PIN reminder (sample)", "Not a real PIN."),
    ];
    for (button, title, body) in samples {
        d.show_manager();
        let before = d.windows();
        d.click(manager, button);
        d.frames(2);
        let w = *d.windows().iter().find(|w| !before.contains(w)).expect("the new note's window");
        if !title.is_empty() {
            d.type_into(w, "Title", title);
            d.type_into(w, "Note", body);
        }
        d.host.advance(Duration::from_millis(500));
        d.frames(3);
    }
    d.show_manager();
    d.frames(3);
    shot(&mut d, manager, "04-manager", None);
    shot(&mut d, manager, "04-manager-narrow", Some((400, 500)));
    shot(&mut d, manager, "04-manager-wide", Some((900, 700)));
    d.host.user_resize(manager, remedy_ui::core::Size::new(440.0, 560.0));
    d.frames(3);
    d.click(manager, "About");
    shot(&mut d, manager, "05-about", None);
    d.click(manager, "About");
    d.click(manager, "Password");
    shot(&mut d, manager, "06-password", None);
    d.click(manager, "Password");
    d.click(manager, "Auto-lock");
    shot(&mut d, manager, "07-auto-lock", None);
    shot(&mut d, manager, "07-auto-lock-narrow", Some((400, 500)));
    d.host.user_resize(manager, remedy_ui::core::Size::new(440.0, 560.0));
    d.click(manager, "Auto-lock");
    d.click(manager, "Delete Wi-Fi");
    shot(&mut d, manager, "08-delete-dialog", None);
    d.click(manager, "Cancel");
    let wifi = d.note_window("Wi-Fi");
    shot(&mut d, wifi, "10-note-yellow", None);
    let keys = d.note_window("Server keys");
    shot(&mut d, keys, "10-note-dark-green", None);
    let empty = d.note_window("Sticky note");
    shot(&mut d, empty, "10-note-empty-pink", None);
    let bank = d.note_window("Bank PIN reminder (sample)");
    shot(&mut d, bank, "10-note-wide", Some((600, 420)));
    d.click(wifi, COPY);
    shot(&mut d, wifi, "10-note-copied", None);
    d.click(wifi, "Lock vault");
    d.frames(3);
    shot(&mut d, manager, "03-unlock", None);
    d.type_into(manager, "Master password", "not the password!!");
    d.click(manager, "Unlock");
    d.until("refusal", |d| d.st.error.get_untracked().ok().flatten().is_some());
    d.frames(2);
    shot(&mut d, manager, "03-unlock-wrong-password", None);
    d.click(manager, "Use recovery key");
    shot(&mut d, manager, "03-unlock-recovery-key", None);
}
