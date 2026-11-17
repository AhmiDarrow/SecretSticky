//! The whole app on the headless host, driven the way a screen reader would: by the
//! accessibility tree's labels and actions. Uses a vault in a temporary directory.

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

struct Driver {
    host: HeadlessHost,
    runner: Runner,
    st: St,
    trees: HashMap<WindowId, HashMap<NodeId, Node>>,
    roots: HashMap<WindowId, NodeId>,
}

impl Driver {
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

    /// Types into a field by focusing it and pasting (what a password manager does).
    fn type_into(&mut self, window: WindowId, label: &str, text: &str) {
        self.act(window, label, Action::Focus);
        let _ = self.host.set_clipboard_text(text, true);
        let ctrl_v = KeyEvent {
            key: Key::Character("v".into()),
            physical: "KeyV".into(),
            modifiers: Modifiers { ctrl: true, ..Modifiers::NONE },
            pressed: true,
            repeat: false,
            composing: false,
            text: None,
        };
        self.host.send(HostEvent::Key(window, ctrl_v));
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
    assert!(d.host.tray().is_some_and(|t| t.tooltip == "SecretSticky"), "tray icon shown");

    // Setup refuses short and mismatched passwords.
    d.type_into(manager, "Master password", "short");
    d.type_into(manager, "Confirm password", "short");
    d.click(manager, "Create vault");
    assert_eq!(d.st.error.get_untracked().ok().flatten().as_deref(), Some("Use at least 12 characters"));
    let pw = "correct horse battery staple";
    d.type_into(manager, "Master password", pw);
    d.type_into(manager, "Confirm password", pw);
    d.click(manager, "Create vault");
    d.until("the recovery key", |d| d.screen() == Screen::Recovery);
    d.frames(2);
    let key = d.st.recovery_key.borrow().as_ref().map(|k| k.to_string()).expect("a recovery key");
    d.click(manager, "Copy key (2 min) — continue");
    assert_eq!(d.host.clipboard(), Some((key.as_str(), true)), "copied as sensitive");
    assert_eq!(d.screen(), Screen::Unlocked);
    assert!(d.st.recovery_key.borrow().is_none(), "the key is forgotten");

    // A new note opens frameless and protected; typing saves after a pause.
    d.frames(2);
    d.click(manager, "New yellow note");
    let note = *d.windows().iter().find(|w| **w != manager).expect("a note window");
    let opts = d.host.window(note).map(|w| w.options().clone()).expect("note options");
    assert!(!opts.decorated && opts.capture_protected, "frameless and capture protected");
    d.frames(2);
    d.assert_labelled(note);
    d.assert_labelled(manager);
    d.assert_accessible(&d.note_ui());
    d.assert_accessible(&d.manager_ui());
    d.type_into(note, "Note", "the launch code is 0000");
    d.host.advance(Duration::from_millis(500));
    d.frames(2);
    let saved = std::fs::read_to_string(&path).expect("vault file");
    assert!(!saved.contains("launch code"), "nothing plain on disk");
    let notes = d.st.with(|v| v.list_notes()).expect("notes");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].body, "the launch code is 0000");

    // Copy from the note clears itself.
    d.click(note, "Copy note (clears in 30 s)");
    assert_eq!(d.host.clipboard(), Some(("the launch code is 0000", true)));
    d.host.advance(Duration::from_secs(31));
    d.frames(1);
    assert_eq!(d.host.clipboard(), None, "cleared after 30 s");

    // Lock from the note: notes close, the manager asks for the password.
    d.click(note, "Lock vault");
    d.frames(2);
    assert_eq!(d.screen(), Screen::Unlock);
    assert_eq!(d.windows(), vec![manager], "the note window closed");

    // A wrong password is refused, the right one opens the vault with the note.
    d.type_into(manager, "Master password", "not the password!!");
    d.click(manager, "Unlock");
    d.until("the refusal", |d| d.st.error.get_untracked().ok().flatten().is_some());
    assert_eq!(d.screen(), Screen::Unlock);
    d.type_into(manager, "Master password", pw);
    d.click(manager, "Unlock");
    d.until("unlock", |d| d.screen() == Screen::Unlocked);
    d.frames(2);
    assert!(d.has(manager, "Open the launch code is 0000") || d.has(manager, "Open Untitled note"), "listed");

    // The tray opens every note; idle locks.
    d.host.send(HostEvent::Tray(TrayEvent::Menu(3)));
    d.frames(2);
    assert_eq!(d.windows().len(), 2, "open all");
    d.host.advance(Duration::from_secs(16 * 60));
    d.frames(2);
    assert_eq!(d.screen(), Screen::Unlock, "idle lock after 15 minutes");
    assert_eq!(d.windows(), vec![manager]);
}

impl Driver {
    /// Starts the app on `vault` and reads its first frames.
    fn new(vault: std::path::PathBuf) -> Self {
        let slot: Rc<RefCell<Option<St>>> = Rc::default();
        let s2 = slot.clone();
        let config = AppConfig::new("SecretSticky test").cpu_only().theme(
            remedy_ui_themes::theme(&remedy_ui_themes::PALETTES[0]),
            remedy_ui_themes::sheet,
        );
        let mut runner = Runner::with_geometry(config, GeometryStore::in_memory(), move |app: &App| {
            *s2.borrow_mut() = Some(crate::state::start_at(app, Some(vault)));
        });
        let mut host = HeadlessHost::new();
        host.run_frames(&mut runner, 1);
        let st = slot.borrow().clone().expect("the app started");
        let mut d = Driver { host, runner, st, trees: HashMap::new(), roots: HashMap::new() };
        d.frames(2);
        d
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

/// Writes the screens as PNGs to `SECRETSTICKY_SHOTS` (run with `--ignored`).
#[test]
#[ignore = "writes images for a person to look at"]
fn screenshots() {
    let Some(out) = std::env::var_os("SECRETSTICKY_SHOTS").map(std::path::PathBuf::from) else { return };
    let _ = std::fs::create_dir_all(&out);
    let dir = tempfile::tempdir().expect("temp dir");
    let mut d = Driver::new(dir.path().join("vault.json"));
    let shot = |d: &Driver, w: WindowId, name: &str| {
        let win = d.host.window(w).expect("window");
        let (pw, ph) = (win.size().0, win.size().1);
        png(&out.join(name), pw, ph, win.surface());
    };
    let manager = d.windows()[0];
    shot(&d, manager, "1-setup.png");
    let pw = "correct horse battery staple";
    d.type_into(manager, "Master password", pw);
    d.type_into(manager, "Confirm password", pw);
    d.click(manager, "Create vault");
    d.until("recovery", |d| d.screen() == Screen::Recovery);
    d.frames(3);
    shot(&d, manager, "2-recovery.png");
    d.click(manager, "I already saved it — continue");
    d.click(manager, "New pink note");
    d.frames(2);
    d.click(manager, "New dark green note");
    d.frames(3);
    let notes: Vec<_> = d.windows().into_iter().filter(|w| *w != manager).collect();
    d.type_into(notes[0], "Title", "Wi-Fi");
    d.type_into(notes[0], "Note", "guest network password: hunter2hunter2");
    d.host.advance(Duration::from_millis(500));
    d.frames(3);
    shot(&d, manager, "3-manager.png");
    shot(&d, notes[0], "4-note-pink.png");
    shot(&d, notes[1], "5-note-dark.png");
    d.click(notes[0], "Lock vault");
    d.frames(3);
    shot(&d, manager, "6-unlock.png");
}
