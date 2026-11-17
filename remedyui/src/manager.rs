//! The manager window: setup, unlock, the recovery key, then the note list and settings.

use std::rc::Rc;
use std::time::Duration;

use remedy_ui::core::{Insets, Point};
use remedy_ui::reactive::{Signal, untrack};
use remedy_ui::style::StylePatch;
use remedy_ui::view::{Align, Justify};
use remedy_ui::{El, TextField, Ui, Window, WindowSpec};
use remedy_ui_widgets_core::{self as w, DialogButton, Icon};
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::state::{self, COLORS, Preview, Screen, St, checked, fill, ink};

const RECOVERY_COPY_KEEP: Duration = Duration::from_secs(120);
const MIN_PASSWORD: usize = 12;

/// Auto-lock choices: label and seconds (0 is off).
const AUTO_LOCK: [(&str, u64); 7] =
    [("Off", 0), ("5 min", 300), ("15 min", 900), ("30 min", 1800), ("1 h", 3600), ("4 h", 14_400), ("12 h", 43_200)];

/// Opens the manager window.
pub fn open(st: &St) {
    let spec = WindowSpec::new("SecretSticky")
        .size(440.0, 600.0)
        .min_size(380.0, 480.0)
        .hide_on_close()
        .persist_geometry("manager")
        .capture_protected(true);
    let s = st.clone();
    let win = st.app.open(spec, move |ui, win| build(&s, ui, win));
    *st.manager.borrow_mut() = Some(win);
}

fn build(st: &St, ui: &Ui, win: &Window) {
    let unit = ui.theme().spacing.unit;
    let use_recovery = Signal::new(false);
    let updater = Rc::new(crate::update::Updater::new());
    let page = w::column(ui).grow(1.0).fill_width().padding(Insets::all(unit * 6.0)).gap(unit * 3.0);
    let p = page.clone();
    let (st2, ui2, win2) = (st.clone(), ui.clone(), win.clone());
    let page = page.bind(move || {
        let screen = st2.screen.get();
        let recovery = use_recovery.get();
        untrack(|| {
            p.clear();
            st2.error.set(None);
            let content = match screen {
                Screen::Setup | Screen::Unlock => gate(&st2, &ui2, &win2, screen == Screen::Setup, recovery, use_recovery),
                Screen::Recovery => recovery_key(&st2, &ui2),
                Screen::Unlocked => unlocked(&st2, &ui2, &win2, &updater),
                Screen::Broken => broken(&st2, &ui2),
            };
            let _ = p.clone().child(content);
        });
    });
    ui.root().child(w::scroll(ui).grow(1.0).fill_width().child(page));
}

/// Error text that follows the shared error signal.
/// Takes no space while there is no error.
fn error_line(st: &St, ui: &Ui) -> El {
    let e = st.error;
    let slot = w::column(ui).fill_width();
    let (s2, ui2) = (slot.clone(), ui.clone());
    slot.bind(move || {
        let msg = e.get();
        untrack(|| {
            s2.clear();
            if let Some(m) = msg {
                let _ = s2.clone().child(w::label(&ui2, m, "error"));
            }
        });
    })
}

/// A button whose label follows `f`.
fn dyn_label(b: El, f: impl Fn() -> String + 'static) -> El {
    if let Some(l) = b.child_elements().into_iter().next() {
        let _ = l.text_dyn(f);
    }
    b
}

fn fineprint(ui: &Ui, s: &str) -> El {
    w::label(ui, s, "muted").style(StylePatch { text_size: Some(ui.theme().type_scale.small), ..StylePatch::default() })
}

fn hide(win: &Window) -> impl Fn() + 'static {
    let win = win.clone();
    move || win.set_visible(false)
}

/// Saves open notes and quits.
pub fn quit(st: &St) {
    let flushes: Vec<_> = st.open.borrow().values().map(|n| n.flush.clone()).collect();
    for f in flushes {
        f();
    }
    st.app.quit();
}

/// Setup (new vault) or unlock (password or recovery key).
fn gate(st: &St, ui: &Ui, win: &Window, setup: bool, recovery: bool, use_recovery: Signal<bool>) -> El {
    let unit = ui.theme().spacing.unit;
    let (title, hint) = if setup {
        ("Create your vault", "Choose a strong master password. Notes are encrypted on disk.")
    } else {
        ("Unlock SecretSticky", "Enter your master password to open encrypted sticky notes.")
    };
    let label = if recovery { "Recovery key" } else { "Master password" };
    let pass = w::secret_input(ui, label).placeholder(label);
    if recovery {
        pass.set_reveal(true);
    }
    let confirm = setup.then(|| w::secret_input(ui, "Confirm password").placeholder("Confirm password"));

    let submit: Rc<dyn Fn()> = {
        let (st, pass, confirm) = (st.clone(), pass.clone(), confirm.clone());
        Rc::new(move || submit(&st, &pass, confirm.as_ref(), setup, recovery))
    };
    let s2 = submit.clone();
    let pass = pass.on_submit(move || s2());
    let busy = st.busy;
    let go = w::button(ui, if setup { "Create vault" } else { "Unlock" }, {
        let s = submit.clone();
        move || {
            if !busy.get_untracked().unwrap_or(false) {
                s()
            }
        }
    })
    .variant("primary")
    .fill_width();
    let go = dyn_label(go, move || {
        if busy.get() {
            "Working…".to_owned()
        } else if setup {
            "Create vault".to_owned()
        } else {
            "Unlock".to_owned()
        }
    });

    let mut form = w::column(ui)
        .gap(unit * 3.0)
        .fill_width()
        .child(w::heading(ui, 1, title))
        .child(w::label(ui, hint, "muted"))
        .child(w::field(ui, label, pass.el().fill_width()));
    if let Some(c) = &confirm {
        let s3 = submit.clone();
        let c = c.clone().on_submit(move || s3());
        form = form.child(w::field(ui, "Confirm password", c.el().fill_width()));
    }
    form = form.child(error_line(st, ui)).child(go);

    let mut escapes = w::wrap(ui).gap(unit * 2.0).justify(Justify::Center);
    if !setup && st.has_recovery.get_untracked().unwrap_or(false) {
        let label = if recovery { "Use master password" } else { "Use recovery key" };
        escapes = escapes.child(w::button(ui, label, move || use_recovery.set(!recovery)).variant("quiet"));
    }
    let s = st.clone();
    escapes = escapes
        .child(w::button(ui, if setup { "Not now — hide to tray" } else { "Hide to tray" }, hide(win)).variant("quiet"))
        .child(w::button(ui, "Quit", move || quit(&s)).variant("danger"));
    let card = w::card(ui)
        .fill_width()
        .child(form.child(escapes).child(fineprint(ui, "Local only · Argon2id + XChaCha20-Poly1305 · no cloud")));
    pass.el().focus();
    card
}

fn submit(st: &St, pass: &TextField, confirm: Option<&TextField>, setup: bool, recovery: bool) {
    if st.busy.get_untracked().unwrap_or(false) {
        return;
    }
    st.error.set(None);
    let pw = pass.take_secret();
    if setup {
        let again = confirm.map(TextField::take_secret);
        if pw.expose().chars().count() < MIN_PASSWORD {
            st.error.set(Some(format!("Use at least {MIN_PASSWORD} characters")));
            return;
        }
        if again.as_ref().map(|a| a.expose()) != Some(pw.expose()) {
            st.error.set(Some("Passwords do not match".to_owned()));
            return;
        }
    } else if pw.is_empty() {
        st.error.set(Some(if recovery { "Enter your recovery key" } else { "Enter your master password" }.to_owned()));
        return;
    } else if let Err(e) = st.throttle.borrow_mut().check() {
        st.error.set(Some(e.to_string()));
        return;
    }
    let Some(vault) = st.shared() else {
        st.error.set(Some(st.load_error.clone()));
        return;
    };
    st.busy.set(true);
    let s = st.clone();
    // Argon2 takes a moment: off the UI thread.
    st.app.background(
        move || -> AppResult<Option<Zeroizing<String>>> {
            let mut v = vault.lock().unwrap_or_else(|e| e.into_inner());
            if setup {
                v.setup(pw.expose()).map(|k| Some(Zeroizing::new(k)))
            } else if recovery {
                v.unlock_with_recovery(pw.expose()).map(|()| None)
            } else {
                v.unlock(pw.expose()).map(|()| None)
            }
        },
        move |r| {
            s.busy.set(false);
            match r {
                Ok(key) => {
                    if !setup {
                        s.throttle.borrow_mut().record_success();
                    }
                    *s.recovery_key.borrow_mut() = key;
                    state::refresh(&s);
                }
                Err(e) => {
                    if !setup && matches!(e, AppError::BadPassword | AppError::Crypto(_)) {
                        s.throttle.borrow_mut().record_failure();
                    }
                    s.error.set(Some(e.to_string()));
                }
            }
        },
    );
}

/// The recovery key, shown once after setup.
fn recovery_key(st: &St, ui: &Ui) -> El {
    let unit = ui.theme().spacing.unit;
    let key = st.recovery_key.borrow().as_ref().map(|k| k.to_string()).unwrap_or_default();
    let done = {
        let st = st.clone();
        move || {
            st.recovery_key.borrow_mut().take();
            state::refresh(&st);
        }
    };
    let copy = {
        let (st, done) = (st.clone(), done.clone());
        move || {
            if let Some(k) = st.recovery_key.borrow().as_ref() {
                st.app.copy_secret(k.to_string(), RECOVERY_COPY_KEEP);
            }
            done();
        }
    };
    let shown = w::label(ui, Zeroizing::new(key).as_str(), "strong")
        .data_text()
        .style(StylePatch { mono: Some(true), ..StylePatch::default() })
        .semantics(remedy_ui_a11y::Semantics::new(remedy_ui_a11y::Role::Label).label("Recovery key"));
    w::card(ui).fill_width().child(
        w::column(ui)
            .gap(unit * 3.0)
            .child(w::heading(ui, 1, "Save your recovery key"))
            .child(w::label(
                ui,
                "This is shown once. Store it offline. Losing both your master password and this key \
                 means your notes cannot be recovered.",
                "muted",
            ))
            .child(w::card(ui).fill_width().child(shown))
            .child(w::button(ui, "Copy key (2 min) — continue", copy).variant("primary").fill_width())
            .child(w::button(ui, "I already saved it — continue", done).variant("quiet").fill_width())
            .child(fineprint(ui, "Protects data at rest on disk. Does not protect a fully compromised PC while unlocked.")),
    )
}

fn broken(st: &St, ui: &Ui) -> El {
    let s = st.clone();
    w::card(ui).fill_width().child(
        w::column(ui)
            .gap(ui.theme().spacing.unit * 3.0)
            .child(w::heading(ui, 1, "SecretSticky can't start"))
            .child(w::label(ui, st.load_error.clone(), "error"))
            .child(w::button(ui, "Quit", move || quit(&s)).variant("danger")),
    )
}

/// The unlocked manager: new notes, the list, actions and settings.
fn unlocked(st: &St, ui: &Ui, win: &Window, updater: &Rc<crate::update::Updater>) -> El {
    let unit = ui.theme().spacing.unit;
    let notes = st.notes;

    let count = w::badge(ui, "").text_dyn(move || {
        let n = notes.with(Vec::len);
        if n == 1 { "1 note".to_owned() } else { format!("{n} notes") }
    });
    let header = w::row(ui)
        .fill_width()
        .align(Align::Center)
        .child(w::heading(ui, 1, "SecretSticky"))
        .child(w::spacer(ui))
        .child(count);

    let swatches = COLORS.iter().fold(w::wrap(ui).gap(unit * 2.0), |row, (c, name)| {
        let (st, c2) = (st.clone(), c.clone());
        row.child(
            w::icon_button(ui, Icon::Plus, format!("New {} note", name.to_lowercase()), move || {
                state::new_note(&st, c2.clone())
            })
            .style(StylePatch {
                background: Some(fill(c)),
                foreground: Some(ink(c)),
                border_width: Some(ui.theme().stroke),
                ..StylePatch::default()
            }),
        )
    });

    let list = w::column(ui).gap(unit).fill_width();
    let (st2, ui2) = (st.clone(), ui.clone());
    // Keyed by what a row shows, so renames and recolors rebuild it.
    let list = w::for_each(
        &list,
        notes,
        |p: &Preview| (p.id.clone(), p.title.clone(), p.color.as_css()),
        move |p| note_row(&st2, &ui2, p),
    );
    let empty = w::column(ui).fill_width();
    let e2 = empty.clone();
    let ui3 = ui.clone();
    let empty = empty.bind(move || {
        let none = notes.with(Vec::is_empty);
        untrack(|| {
            e2.clear();
            if none {
                let _ = e2.clone().child(w::empty_state(&ui3, "No notes yet", "Pick a color above to start one."));
            }
        });
    });

    let (s1, s2, s3) = (st.clone(), st.clone(), st.clone());
    let actions = w::wrap(ui)
        .gap(unit * 2.0)
        .child(w::button(ui, "Open all", move || state::open_all(&s1)))
        .child(w::button(ui, "Lock", move || state::lock_all(&s2)).variant("primary"))
        .child(w::button(ui, "Hide to tray", hide(win)).variant("quiet"))
        .child(w::button(ui, "Quit", move || quit(&s3)).variant("danger"));

    w::column(ui)
        .gap(unit * 4.0)
        .fill_width()
        .child(header)
        .child(w::section(ui, "New note").child(swatches))
        .child(w::section(ui, "Notes").child(list).child(empty))
        .child(error_line(st, ui))
        .child(actions)
        .child(settings(st, ui))
        .child(crate::update::section(st, ui, updater))
}

fn note_row(st: &St, ui: &Ui, p: &Preview) -> El {
    let unit = ui.theme().spacing.unit;
    let color = fill(&p.color);
    let dot = w::canvas(ui, move |cx| {
        let r = 6.0;
        let c = Point::new(cx.size.width / 2.0, cx.size.height / 2.0);
        cx.list.fill_rounded(
            remedy_ui::core::Rect::new(c.x - r, c.y - r, c.x + r, c.y + r),
            remedy_ui::core::CornerRadii::all(r),
            remedy_ui_paint::Brush::Solid(color),
        );
    })
    .width(16.0)
    .height(16.0);
    let title = if p.title.trim().is_empty() { "Untitled note".to_owned() } else { p.title.clone() };
    let (st1, st2, id1, id2, t2, ui2) = (st.clone(), st.clone(), p.id.clone(), p.id.clone(), title.clone(), ui.clone());
    let delete = w::icon_button(ui, Icon::Trash, format!("Delete {title}"), move || {
        let (st, id) = (st2.clone(), id2.clone());
        let _ = w::dialog(
            &ui2,
            "Delete note?",
            format!("“{t2}” will be deleted for good. This cannot be undone."),
            vec![
                DialogButton::new("Cancel", "quiet", || {}),
                DialogButton::new("Delete", "danger", move || state::delete_note(&st, &id)),
            ],
        );
    })
    .variant("quiet");
    ui.el("list_item")
        .row()
        .gap(unit * 2.0)
        .align(Align::Center)
        .fill_width()
        .padding(Insets::symmetric(unit * 2.0, unit * 3.0))
        .child(dot)
        .child(w::text(ui, title.clone()).data_text().grow(1.0).nowrap())
        .child(delete)
        .on_click(move || state::open_note(&st1, &id1))
        .semantics(remedy_ui_a11y::Semantics::new(remedy_ui_a11y::Role::Button).label(format!("Open {title}")))
}

/// Auto-lock and password.
fn settings(st: &St, ui: &Ui) -> El {
    let unit = ui.theme().spacing.unit;
    let secs = st.idle_secs.get_untracked().unwrap_or(0);
    let at = AUTO_LOCK.iter().position(|(_, s)| *s == secs).unwrap_or(2);
    let chosen = Signal::new(at);
    let labels: Vec<&str> = AUTO_LOCK.iter().map(|(l, _)| *l).collect();
    let seg = w::segmented(ui, "Auto-lock after", &labels, chosen);
    let st2 = st.clone();
    let seg = seg.clone().bind(move || {
        let i = chosen.get();
        untrack(|| {
            let Some((_, secs)) = AUTO_LOCK.get(i).copied() else { return };
            if st2.idle_secs.get_untracked().unwrap_or(0) == secs {
                return;
            }
            if checked(&st2, st2.with(|v| v.set_idle_lock_secs(secs))).is_some() {
                st2.idle_secs.set(secs);
                st2.app.set_idle_timeout(state::idle_timeout(secs));
            }
        });
    });
    let (st3, ui3) = (st.clone(), ui.clone());
    w::section(ui, "Security").child(
        w::column(ui)
            .gap(unit * 3.0)
            .fill_width()
            .child(w::field(ui, "Auto-lock after", seg))
            .child(w::button(ui, "Change password…", move || change_password(&st3, &ui3))),
    )
}

fn change_password(st: &St, ui: &Ui) {
    let unit = ui.theme().spacing.unit;
    let current = w::secret_input(ui, "Current password");
    let new = w::secret_input(ui, "New password");
    let again = w::secret_input(ui, "Confirm new password");
    let msg = Signal::new(None::<String>);
    let busy = Signal::new(false);
    let modal_slot: Rc<std::cell::RefCell<Option<w::Modal>>> = Rc::default();
    let save = {
        let (st, current, new, again, slot) = (st.clone(), current.clone(), new.clone(), again.clone(), modal_slot.clone());
        move || {
            if busy.get_untracked().unwrap_or(false) {
                return;
            }
            let (c, n, a) = (current.take_secret(), new.take_secret(), again.take_secret());
            if n.expose().chars().count() < MIN_PASSWORD {
                msg.set(Some(format!("Use at least {MIN_PASSWORD} characters")));
                return;
            }
            if n.expose() != a.expose() {
                msg.set(Some("Passwords do not match".to_owned()));
                return;
            }
            let Some(vault) = st.shared() else { return };
            busy.set(true);
            let slot = slot.clone();
            st.app.background(
                move || vault.lock().unwrap_or_else(|e| e.into_inner()).change_password(c.expose(), n.expose()),
                move |r| {
                    busy.set(false);
                    match r {
                        Ok(()) => {
                            if let Some(m) = slot.borrow().as_ref() {
                                m.close();
                            }
                        }
                        Err(e) => msg.set(Some(e.to_string())),
                    }
                },
            );
        }
    };
    let save = Rc::new(save);
    let s2 = save.clone();
    let slot2 = modal_slot.clone();
    let content = w::column(ui)
        .gap(unit * 3.0)
        .child(w::field(ui, "Current password", current.el().fill_width()))
        .child(w::field(ui, "New password", new.el().fill_width()))
        .child(w::field(ui, "Confirm new password", again.clone().on_submit(move || s2()).el().fill_width()))
        .child(w::label(ui, "", "error").text_dyn(move || msg.get().unwrap_or_default()))
        .child(
            w::row(ui)
                .gap(unit * 2.0)
                .justify(Justify::End)
                .child(
                    w::button(ui, "Cancel", move || {
                        if let Some(m) = slot2.borrow().as_ref() {
                            m.close();
                        }
                    })
                    .variant("quiet"),
                )
                .child(dyn_label(w::button(ui, "Change password", move || save()).variant("primary"), move || {
                    if busy.get() { "Working…".to_owned() } else { "Change password".to_owned() }
                })),
        );
    let m = w::modal(ui, "Change password", content, true);
    *modal_slot.borrow_mut() = Some(m);
    current.el().focus();
}
