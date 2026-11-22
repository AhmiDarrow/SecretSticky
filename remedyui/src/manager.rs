//! The manager window, laid out as the shipped app's: setup, unlock and the recovery key on
//! a centered card; unlocked, a header (brand, About, Password, Auto-lock, Open all, Lock),
//! the color swatches that start notes, the note list, and a footer with the tray and quit.

use std::rc::Rc;
use std::time::Duration;

use remedy_ui::core::{Insets, Point};
use remedy_ui::reactive::Signal;
use remedy_ui::style::StylePatch;
use remedy_ui::view::{Align, Justify};
use remedy_ui::{El, TextField, Ui, Window, WindowSpec};
use remedy_ui_a11y::{Action, Role, Semantics};
use remedy_ui_widgets_core::{self as w, DialogButton};
use zeroize::Zeroizing;

use crate::error::AppResult;
use crate::state::{self, COLORS, Preview, Screen, St, checked, color_name, fill};

/// How long a copied recovery key stays on the clipboard.
const RECOVERY_COPY_KEEP: Duration = Duration::from_secs(120);
/// The shortest master password the app accepts.
pub const MIN_PASSWORD: usize = 12;

/// Auto-lock presets, Off … 12 h, as the shipped app offers them.
pub const AUTO_LOCK: [(u64, &str); 10] = [
    (0, "Off"),
    (60, "1 min"),
    (300, "5 min"),
    (900, "15 min"),
    (1800, "30 min"),
    (3600, "1 hr"),
    (7200, "2 hr"),
    (14_400, "4 hr"),
    (28_800, "8 hr"),
    (43_200, "12 hr"),
];

/// The project links About offers (the shipped app's allowlist).
pub const LINKS: [(&str, &str); 3] = [
    ("GitHub profile", "https://github.com/AhmiDarrow"),
    ("Project repo", "https://github.com/AhmiDarrow/SecretSticky"),
    ("Releases", "https://github.com/AhmiDarrow/SecretSticky/releases"),
];

/// An auto-lock time in words ("Off", "15 min", "1 hr", "1.5 hrs").
pub fn format_auto_lock(secs: u64) -> String {
    if secs == 0 {
        return "Off".to_owned();
    }
    let mins = (secs as f64 / 60.0).round();
    if mins < 60.0 {
        return format!("{mins} min");
    }
    let hrs = mins / 60.0;
    let n = if hrs.fract() == 0.0 { format!("{hrs}") } else { format!("{hrs:.1}") };
    format!("{n} hr{}", if hrs == 1.0 { "" } else { "s" })
}

/// What the manager says after the master password changed.
pub const PASSWORD_CHANGED: &str = "Password updated. Recovery key is unchanged.";

/// Which panel the header has open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    None,
    About,
    Password,
    AutoLock,
}

/// Opens the manager window.
pub fn open(st: &St) {
    let spec = WindowSpec::new("SecretSticky")
        .size(440.0, 560.0)
        .min_size(400.0, 500.0)
        .hide_on_close()
        .persist_geometry("manager")
        .capture_protected(true);
    let spec = match crate::brand::window_icon() {
        Some(icon) => spec.icon(icon),
        None => spec,
    };
    let s = st.clone();
    let win = st.app.open(spec, move |ui, win| build(&s, ui, win));
    *st.manager.borrow_mut() = Some(win);
}

fn build(st: &St, ui: &Ui, win: &Window) {
    let updater = Rc::new(crate::update::Updater::new());
    let screen = st.screen;
    let use_recovery = Signal::new(false);
    let page = w::column(ui).grow(1.0).fill_width().layout(|l| l.min_height = remedy_ui::layout::Length::Px(0.0));
    let s = |want: Screen| move || screen.get() == want;
    let (st1, ui1) = (st.clone(), ui.clone());
    let setup = remedy_ui::show(ui, s(Screen::Setup), move || gate(&st1, &ui1, true, false, use_recovery)).el();
    let (st2, ui2) = (st.clone(), ui.clone());
    let unlock = remedy_ui::show(ui, move || screen.get() == Screen::Unlock && !use_recovery.get(), move || {
        gate(&st2, &ui2, false, false, use_recovery)
    }).el();
    let (st3, ui3) = (st.clone(), ui.clone());
    let recovery = remedy_ui::show(ui, move || screen.get() == Screen::Unlock && use_recovery.get(), move || {
        gate(&st3, &ui3, false, true, use_recovery)
    }).el();
    let (st4, ui4) = (st.clone(), ui.clone());
    let key = remedy_ui::show(ui, s(Screen::Recovery), move || centered(&ui4, recovery_key(&st4, &ui4))).el();
    let (st5, ui5, upd) = (st.clone(), ui.clone(), updater.clone());
    let unlocked = remedy_ui::show(ui, s(Screen::Unlocked), move || unlocked(&st5, &ui5, &upd)).el();
    let (st6, ui6) = (st.clone(), ui.clone());
    let broken = remedy_ui::show(ui, s(Screen::Broken), move || centered(&ui6, broken(&st6, &ui6))).el();
    let _ = win;
    // The screens take turns in the page: the one shown fills it (the others take no room),
    // and scrolls inside rather than growing past a small window.
    let screen = |e: El| e.grow(1.0).layout(|l| l.min_height = remedy_ui::layout::Length::Px(0.0));
    ui.root().child(
        page.child(screen(setup))
            .child(screen(unlock))
            .child(screen(recovery))
            .child(screen(key))
            .child(screen(unlocked))
            .child(screen(broken)),
    );
}

/// A card centered in the window, scrolling when the window is small.
fn centered(ui: &Ui, card: El) -> El {
    let unit = ui.theme().spacing.unit;
    let inner = w::column(ui)
        .fill_width()
        .grow(1.0)
        .align(Align::Center)
        .justify(Justify::Center)
        .padding(Insets::all(unit * 6.0))
        .child(card.width(380.0));
    w::scroll(ui).grow(1.0).fill_width().child(inner)
}

/// The app mark at `size`, or nothing when the artwork could not be decoded.
fn mark(ui: &Ui, size: f32) -> El {
    match crate::brand::mark() {
        // The shipped `.brand-mark`: a rounded tile (11 px at 40) on a soft drop shadow.
        Some(img) => w::image(ui, img, "SecretSticky")
            .width(size)
            .height(size)
            .style(StylePatch::new().radius(size * 0.275).shadow(remedy_ui::style::Shadow::new(
                0.0,
                size * 0.1,
                size * 0.35,
                remedy_ui::core::Color::BLACK.with_alpha(0.28),
            )))
            .semantics(Semantics::new(Role::Image).hidden(true)),
        None => w::spacer(ui).width(0.0),
    }
}

/// Error text that follows the shared error signal; takes no space while there is none.
fn error_line(st: &St, ui: &Ui) -> El {
    let e = st.error;
    let u = ui.clone();
    remedy_ui::show(ui, move || e.with(Option::is_some), move || {
        w::text(&u, move || e.get().unwrap_or_default()).variant("error").semantics(move || {
            Semantics::new(Role::Status).label(e.get().unwrap_or_default())
        })
    }).el()
    .fill_width()
}

fn fineprint(ui: &Ui, s: &str) -> El {
    w::text(ui, s).variant("muted").style(StylePatch::new().text_size(ui.theme().type_scale.small))
}

/// Setup (a new vault) or unlock (the master password, or the recovery key).
fn gate(st: &St, ui: &Ui, setup: bool, recovery: bool, use_recovery: Signal<bool>) -> El {
    let unit = ui.theme().spacing.unit;
    let busy = st.busy;
    let (title, hint) = if setup {
        ("Create your vault", "Choose a strong master password. Notes are encrypted on disk.")
    } else {
        ("Unlock SecretSticky", "Enter your master password to open encrypted sticky notes.")
    };
    let label = if recovery { "Recovery key" } else { "Master password" };
    let pass = w::secret_input(ui, label);
    if recovery {
        // The key is typed from paper: shown as typed, as the shipped app did.
        pass.set_reveal(true);
    }
    let confirm = setup.then(|| w::secret_input(ui, "Confirm password"));

    let submit: Rc<dyn Fn()> = {
        let (st, pass, confirm) = (st.clone(), pass.clone(), confirm.clone());
        Rc::new(move || submit(&st, &pass, confirm.as_ref(), setup, recovery))
    };
    let s1 = submit.clone();
    let pass = pass.on_submit(move || s1());
    let go = w::button(
        ui,
        move || {
            if busy.get() {
                "Working…".to_owned()
            } else if setup {
                "Create vault".to_owned()
            } else {
                "Unlock".to_owned()
            }
        },
        {
            let s = submit.clone();
            move || s()
        },
    )
    .variant("primary")
    .fill_width()
    .disabled(move || busy.get());

    let mut form = w::column(ui)
        .gap(unit * 3.0)
        .fill_width()
        .child(w::field(ui, label, pass.el().fill_width().disabled(move || busy.get())));
    if let Some(c) = &confirm {
        let s2 = submit.clone();
        let c = c.clone().on_submit(move || s2());
        form = form.child(w::field(ui, "Confirm password", c.el().fill_width().disabled(move || busy.get())));
    }
    form = form.child(error_line(st, ui)).child(go);

    let mut escapes = w::column(ui).gap(unit).fill_width().align(Align::Start);
    if !setup && st.has_recovery.get_untracked().unwrap_or(false) {
        let (st, label) = (st.clone(), if recovery { "Use master password" } else { "Use recovery key" });
        escapes = escapes.child(
            w::button(ui, label, move || {
                st.error.set(None);
                use_recovery.set(!recovery);
            })
            .variant("quiet")
            .disabled(move || busy.get()),
        );
    }
    let (s1, s2) = (st.clone(), st.clone());
    escapes = escapes
        .child(
            w::button(ui, if setup { "Not now — hide to tray" } else { "Hide to tray" }, move || state::hide_manager(&s1))
                .variant("quiet")
                .disabled(move || busy.get()),
        )
        .child(w::button(ui, "Quit", move || state::quit(&s2)).variant("danger").disabled(move || busy.get()));

    let card = w::card(ui).fill_width().gap(unit * 3.0).child(mark(ui, 44.0)).child(w::heading(ui, 1, title)).child(
        w::text(ui, hint).variant("muted"),
    );
    let card = card
        .child(form)
        .child(escapes)
        .child(fineprint(ui, "Local only · Argon2id + XChaCha20-Poly1305 · no cloud"));
    pass.el().focus();
    centered(ui, card)
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
    // Argon2id takes about a second: off the UI thread.
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
                    // Every failed unlock counts toward the throttle, as in the Tauri app.
                    if !setup {
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
    let key = Zeroizing::new(st.recovery_key.borrow().as_ref().map(|k| k.to_string()).unwrap_or_default());
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
    let warn = ui.theme().colors.warn;
    let shown = w::text(ui, key.as_str())
        .data_text()
        .style(StylePatch::new().mono(true).foreground(warn).text_size(ui.theme().type_scale.small))
        .semantics(Semantics::new(Role::Label).label("Recovery key"));
    w::card(ui)
        .fill_width()
        .gap(unit * 3.0)
        .child(mark(ui, 44.0))
        .child(w::heading(ui, 1, "Save your recovery key"))
        .child(
            w::text(
                ui,
                "This is shown once. Store it offline. Losing both your master password and this key \
                 means your notes cannot be recovered.",
            )
            .variant("muted"),
        )
        .child(w::card(ui).fill_width().variant("inset").child(shown))
        .child(w::button(ui, "Copy key (2 min) — continue", copy).variant("primary").fill_width())
        .child(w::button(ui, "I already saved it — continue", done).variant("quiet").fill_width())
        .child(fineprint(ui, "Protects data at rest on disk. Does not protect a fully compromised PC while unlocked."))
}

fn broken(st: &St, ui: &Ui) -> El {
    let s = st.clone();
    w::card(ui)
        .fill_width()
        .gap(ui.theme().spacing.unit * 3.0)
        .child(mark(ui, 44.0))
        .child(w::heading(ui, 1, "SecretSticky can't start"))
        .child(w::text(ui, st.load_error.clone()).variant("error"))
        .child(w::button(ui, "Quit", move || state::quit(&s)).variant("danger"))
}

/// The unlocked manager.
fn unlocked(st: &St, ui: &Ui, updater: &Rc<crate::update::Updater>) -> El {
    let unit = ui.theme().spacing.unit;
    let panel = Signal::new(Panel::None);
    let busy = st.busy;
    let notes = st.notes;
    let idle = st.idle_secs;

    // Header: brand and the five actions.
    let brand = w::row(ui)
        .gap(unit * 3.0)
        .align(Align::Center)
        .child(mark(ui, 40.0))
        .child(
            w::column(ui)
                .gap(unit * 0.5)
                .child(w::heading(ui, 1, "SecretSticky"))
                .child(w::text(ui, "Encrypted sticky notes").variant("muted")),
        );
    // Five equal buttons in small type, as the shipped header's grid.
    let compact = StylePatch::new()
        .text_size(ui.theme().type_scale.small)
        .padding(remedy_ui::core::Insets::symmetric(unit * 1.5, unit));
    let head_button = |b: El| b.grow(1.0).style(compact.clone()).layout(|l| l.basis = remedy_ui::layout::Length::Px(0.0));
    let toggle = |label: &'static str, which: Panel| {
        let b = head_button(w::button(ui, label, move || panel.update(|p| *p = if *p == which { Panel::None } else { which })))
            .disabled(move || busy.get());
        b.semantics(move || {
            Semantics::new(Role::Button).label(label).expanded(panel.get() == which).action(Action::Click)
        })
    };
    let auto = toggle("Auto-lock", Panel::AutoLock).variant(move || if idle.get() == 0 { "" } else { "primary" });
    w::tooltip(&auto, move || {
        let secs = idle.get();
        if secs == 0 { "Auto-lock is off".to_owned() } else { format!("Auto-lock after {} of inactivity", format_auto_lock(secs)) }
    });
    let (s1, s2) = (st.clone(), st.clone());
    let actions = w::row(ui)
        .gap(unit * 1.5)
        .fill_width()
        .child(toggle("About", Panel::About))
        .child(toggle("Password", Panel::Password))
        .child(auto)
        .child(head_button(w::button(ui, "Open all", move || state::open_all(&s1))).disabled(move || busy.get()))
        .child(head_button(w::button(ui, "Lock", move || state::lock_all(&s2))).variant("danger").disabled(move || busy.get()));
    let header = w::column(ui)
        .gap(unit * 2.5)
        .fill_width()
        .padding(crate::state::pad(unit * 3.5, unit * 4.5, unit * 3.0, unit * 4.5))
        .child(brand)
        .child(actions);

    // Panels.
    let (st_a, ui_a, up_a) = (st.clone(), ui.clone(), updater.clone());
    let about = remedy_ui::show(ui, move || panel.get() == Panel::About, move || about(&st_a, &ui_a, &up_a, panel)).el();
    let (st_p, ui_p) = (st.clone(), ui.clone());
    let password = remedy_ui::show(ui, move || panel.get() == Panel::Password, move || password(&st_p, &ui_p, panel)).el();
    let (st_l, ui_l) = (st.clone(), ui.clone());
    let auto_lock = remedy_ui::show(ui, move || panel.get() == Panel::AutoLock, move || auto_lock(&st_l, &ui_l, panel)).el();

    // New note: one swatch per color.
    let swatches = COLORS.iter().fold(w::wrap(ui).gap(unit * 1.5), |row, (c, name)| {
        let (st, c2) = (st.clone(), c.clone());
        row.child(swatch(ui, c, 28.0, format!("New {} note", name.to_lowercase()), None, move || state::new_note(&st, c2.clone())))
    });
    let new_row = w::row(ui)
        .gap(unit * 3.0)
        .align(Align::Center)
        .fill_width()
        .child(w::text(ui, "New note").variant("muted"))
        .child(swatches.grow(1.0));

    // The list.
    let list = w::column(ui).gap(unit * 1.5).fill_width();
    let (st2, ui2) = (st.clone(), ui.clone());
    let list = w::for_each(
        &list,
        notes,
        // Keyed by what a row shows, so a rename or a new color rebuilds it.
        |p: &Preview| (p.id.clone(), p.title.clone(), p.color.as_css()),
        move |p| note_row(&st2, &ui2, p, panel),
    );
    let ui3 = ui.clone();
    let empty = remedy_ui::show(ui, move || notes.with(Vec::is_empty), move || {
        w::text(&ui3, "No notes yet — pick a color to create one.")
            .variant("muted")
            .padding(Insets::symmetric(unit * 6.0, unit * 3.0))
    }).el();

    let middle = w::column(ui)
        .gap(unit * 3.0)
        .fill_width()
        .padding(crate::state::pad(unit * 3.0, unit * 4.5, unit * 3.0, unit * 4.5))
        .child(about.fill_width())
        .child(password.fill_width())
        .child(auto_lock.fill_width())
        .child(new_row)
        .child(error_line(st, ui))
        .child(list)
        .child(empty.fill_width().align(Align::Center));

    // Footer.
    let count = w::text(ui, move || {
        let n = notes.with(Vec::len);
        format!("{n} note{} · local vault", if n == 1 { "" } else { "s" })
    });
    let small = ui.theme().type_scale.small;
    let meta = w::column(ui)
        .gap(unit * 0.5)
        .child(count.style(StylePatch::new().text_size(small)))
        .child(fineprint(ui, "X → tray · stickies hide manager"))
        .child(fineprint(ui, "Copy clears in 30s"));
    let (s3, s4) = (st.clone(), st.clone());
    let hide = w::button(ui, "Hide to tray", move || state::hide_manager(&s3)).variant("quiet").disabled(move || busy.get());
    w::tooltip(&hide, "Hide manager; app stays in the tray");
    let quit = w::button(ui, "Quit", move || state::quit(&s4)).variant("danger").disabled(move || busy.get());
    w::tooltip(&quit, "Close all stickies and quit");
    let footer = w::column(ui)
        .gap(unit * 2.0)
        .fill_width()
        .padding(crate::state::pad(unit * 3.0, unit * 4.5, unit * 3.5, unit * 4.5))
        .child(meta)
        .child(w::row(ui).gap(unit * 1.5).fill_width().justify(Justify::End).child(hide).child(quit));

    w::column(ui)
        .grow(1.0)
        .fill_width()
        .layout(|l| l.min_height = remedy_ui::layout::Length::Px(0.0))
        .child(header)
        .child(w::divider(ui, false))
        .child(w::scroll(ui).grow(1.0).fill_width().child(middle))
        .child(w::divider(ui, false))
        .child(footer)
}

/// A round color swatch: a `dot`-sized circle on a target at least 24 px square (the
/// accessibility bar), ringed when it is the `chosen` color.
pub fn swatch(
    ui: &Ui,
    c: &crate::vault::NoteColor,
    dot: f32,
    label: String,
    chosen: Option<Signal<crate::vault::NoteColor>>,
    on_press: impl Fn() + 'static,
) -> El {
    let target = dot.max(24.0);
    let (color, dark, me) = (fill(c), state::is_dark(c), c.clone());
    let edge = if dark { remedy_ui::core::Color::WHITE.with_alpha(0.25) } else { remedy_ui::core::Color::BLACK.with_alpha(0.15) };
    let picked = remedy_ui::core::Color::from_hex(if dark { "#f3f4f6" } else { "#111111" }).unwrap_or(remedy_ui::core::Color::BLACK);
    let paint = w::canvas(ui, move |cx| {
        let on = chosen.as_ref().is_some_and(|s| s.get() == me);
        let center = Point::new(cx.size.width / 2.0, cx.size.height / 2.0);
        let circle = |r: f32| remedy_ui::core::Rect::new(center.x - r, center.y - r, center.x + r, center.y + r);
        let r = dot / 2.0;
        cx.list.fill_rounded(circle(r), remedy_ui::core::CornerRadii::all(r), remedy_ui_paint::Brush::Solid(color));
        cx.list.border(circle(r), remedy_ui::core::CornerRadii::all(r), if dot > 20.0 { 2.0 } else { 1.0 }, edge);
        if on {
            cx.list.border(circle(r + 2.5), remedy_ui::core::CornerRadii::all(r + 2.5), 2.0, picked);
        }
    })
    .width(target)
    .height(target);
    let b = w::button(ui, "", on_press)
        .width(target)
        .height(target)
        .style(
            StylePatch::new()
                .background(remedy_ui::core::Color::TRANSPARENT)
                .border(remedy_ui::core::Color::TRANSPARENT)
                .radius(target / 2.0)
                .padding(Insets::all(0.0)),
        )
        .child(paint);
    let b = match chosen {
        Some(sel) => {
            let me = c.clone();
            b.semantics(move || Semantics::new(Role::Button).label(label.clone()).checked(sel.get() == me).action(Action::Click))
        }
        None => b.semantics(Semantics::new(Role::Button).label(label).action(Action::Click)),
    };
    w::tooltip(&b, color_name(c));
    b
}

fn note_row(st: &St, ui: &Ui, p: &Preview, panel: Signal<Panel>) -> El {
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
    .width(14.0)
    .height(14.0);
    let title = p.shown_title();
    let sub = format!("{} · open to edit", color_name(&p.color));
    let (st1, st2, id1, id2, t2, ui2) = (st.clone(), st.clone(), p.id.clone(), p.id.clone(), title.clone(), ui.clone());
    let open = ui
        .el("list_item")
        .row()
        .gap(unit * 2.5)
        .align(Align::Center)
        .grow(1.0)
        .padding(crate::state::pad(unit * 2.5, unit * 2.0, unit * 2.5, unit * 3.0))
        .child(dot)
        .child(
            w::column(ui)
                .grow(1.0)
                .gap(unit * 0.5)
                .child(w::text(ui, title.clone()).data_text().nowrap().style(StylePatch::new().weight(600.0)))
                .child(fineprint(ui, &sub)),
        )
        .on_click(move || state::open_note(&st1, &id1))
        .semantics(Semantics::new(Role::Button).label(format!("Open {title}")).action(Action::Click));
    let delete = w::icon_button(ui, w::Icon::Close, format!("Delete {title}"), move || {
        panel.set(Panel::None);
        let (st, id) = (st2.clone(), id2.clone());
        let _ = w::dialog(
            &ui2,
            "Delete note?",
            format!("{t2} will be removed permanently from the vault."),
            vec![
                DialogButton::new("Cancel", "quiet", || {}),
                DialogButton::new("Delete", "danger_solid", move || state::delete_note(&st, &id)),
            ],
        );
    })
    .variant("quiet")
    .style(StylePatch::new().foreground(ui.theme().colors.danger));
    w::card(ui)
        .row()
        .fill_width()
        .align(Align::Center)
        .padding(Insets::all(0.0))
        .child(open)
        .child(delete)
}

/// A panel under the header: a card with a title, a line of help, the content and a footer.
fn panel_card(ui: &Ui, title: &str, help: &str) -> El {
    let unit = ui.theme().spacing.unit;
    w::card(ui)
        .fill_width()
        .gap(unit * 2.0)
        .semantics(Semantics::new(Role::Group).label(title))
        .child(w::heading(ui, 2, title))
        .child(fineprint(ui, help))
}

fn panel_footer(ui: &Ui, meta: &str, close: &'static str, panel: Signal<Panel>) -> El {
    let unit = ui.theme().spacing.unit;
    w::column(ui).gap(unit * 2.0).fill_width().child(w::divider(ui, false)).child(
        w::row(ui)
            .gap(unit * 2.0)
            .fill_width()
            .align(Align::Center)
            .child(fineprint(ui, meta).grow(1.0))
            .child(w::button(ui, close, move || panel.set(Panel::None)).variant("quiet")),
    )
}

fn about(st: &St, ui: &Ui, updater: &Rc<crate::update::Updater>, panel: Signal<Panel>) -> El {
    let unit = ui.theme().spacing.unit;
    let links = LINKS.iter().enumerate().fold(w::row(ui).gap(unit * 1.5).fill_width(), |row, (i, (label, url))| {
        let (app, err, url) = (st.app.clone(), st.error, (*url).to_owned());
        let b = w::button(ui, *label, move || {
            app.open_url(url.clone(), move |r| {
                if r.is_err() {
                    err.set(Some("This system cannot open links.".to_owned()));
                }
            });
        })
        .grow(1.0);
        row.child(if i == 0 { b.variant("primary") } else { b })
    });
    let body = w::column(ui)
        .gap(unit * 1.5)
        .grow(1.0)
        .child(w::heading(ui, 2, "About"))
        .child(w::text(ui, "Hi I'm Ahmi, hope this helps!").style(StylePatch::new().weight(600.0)))
        .child(fineprint(ui, "Local-only encrypted sticky notes for Windows. MIT licensed."))
        .child(fineprint(ui, &format!("Version {}", env!("CARGO_PKG_VERSION"))))
        .child(links)
        .child(crate::update::section(st, ui, updater))
        .child(panel_footer(ui, "Local vault · no cloud", "Close", panel));
    w::card(ui)
        .fill_width()
        .semantics(Semantics::new(Role::Group).label("About SecretSticky"))
        .child(w::row(ui).gap(unit * 3.0).align(Align::Start).fill_width().child(mark(ui, 44.0)).child(body))
}

fn password(st: &St, ui: &Ui, panel: Signal<Panel>) -> El {
    let unit = ui.theme().spacing.unit;
    let current = w::secret_input(ui, "Current password");
    let new = w::secret_input(ui, "New password");
    let again = w::secret_input(ui, "Confirm new password");
    let busy = Signal::new(false);
    let msg = Signal::new(None::<(bool, String)>);
    let toast_ui = ui.clone();
    let save: Rc<dyn Fn()> = {
        let (st, current, new, again) = (st.clone(), current.clone(), new.clone(), again.clone());
        Rc::new(move || {
            if busy.get_untracked().unwrap_or(false) {
                return;
            }
            let (c, n, a) = (current.take_secret(), new.take_secret(), again.take_secret());
            if n.expose().chars().count() < MIN_PASSWORD {
                msg.set(Some((false, format!("New password must be at least {MIN_PASSWORD} characters."))));
                return;
            }
            if n.expose() != a.expose() {
                msg.set(Some((false, "New passwords do not match.".to_owned())));
                return;
            }
            let Some(vault) = st.shared() else { return };
            busy.set(true);
            st.busy.set(true);
            let (s, toast_ui) = (st.clone(), toast_ui.clone());
            st.app.background(
                move || vault.lock().unwrap_or_else(|e| e.into_inner()).change_password(c.expose(), n.expose()),
                move |r| {
                    busy.set(false);
                    s.busy.set(false);
                    match r {
                        Ok(()) => {
                            panel.set(Panel::None);
                            let _ = w::toast(&toast_ui, PASSWORD_CHANGED, Duration::from_secs(4));
                        }
                        Err(e) => msg.set(Some((false, e.to_string()))),
                    }
                },
            );
        })
    };
    let (s1, s2, s3) = (save.clone(), save.clone(), save.clone());
    let field = |label: &str, f: &TextField, s: Rc<dyn Fn()>| {
        w::field(ui, label, f.clone().on_submit(move || s()).el().fill_width().disabled(move || busy.get()))
    };
    let u = ui.clone();
    let message = remedy_ui::show(ui, move || msg.with(Option::is_some), move || {
        let m = msg.get_untracked().ok().flatten().unwrap_or_default();
        w::text(&u, m.1.clone()).variant(if m.0 { "ok" } else { "error" }).semantics(Semantics::new(Role::Status).label(m.1))
    }).el();
    let actions = w::row(ui)
        .gap(unit * 2.0)
        .child(
            w::button(ui, move || if busy.get() { "Updating…".to_owned() } else { "Update password".to_owned() }, move || {
                s3()
            })
            .variant("primary")
            .disabled(move || busy.get()),
        )
        .child(
            w::button(ui, "Cancel", move || {
                msg.set(None);
                panel.set(Panel::None);
            })
            .variant("quiet")
            .disabled(move || busy.get()),
        );
    current.el().focus();
    panel_card(ui, "Change master password", "Re-wraps the vault key. Your recovery key stays valid.")
        .child(field("Current password", &current, s1))
        .child(field("New password", &new, s2))
        .child(field("Confirm new password", &again, save))
        .child(message.fill_width())
        .child(actions)
}

fn auto_lock(st: &St, ui: &Ui, panel: Signal<Panel>) -> El {
    let unit = ui.theme().spacing.unit;
    let idle = st.idle_secs;
    let busy = st.busy;
    let now = w::badge(ui, move || {
        let s = idle.get();
        if s == 0 { "Off".to_owned() } else { format!("Now: {}", format_auto_lock(s)) }
    })
    .nowrap()
    // Keeps its width; the help text beside it wraps instead.
    .layout(|l| l.shrink = 0.0)
    .semantics(move || {
        let s = idle.get();
        Semantics::new(Role::Status).label(if s == 0 { "Auto-lock is off".to_owned() } else { format!("Auto-lock now {}", format_auto_lock(s)) })
    });
    let presets = AUTO_LOCK.iter().fold(w::wrap(ui).gap(unit * 1.5).fill_width(), |row, &(secs, label)| {
        let st = st.clone();
        let b = w::button(ui, label, move || set_auto_lock(&st, secs))
            .width(68.0)
            .variant(move || if idle.get() == secs { "primary" } else { "" })
            .disabled(move || busy.get());
        let b = b.semantics(move || {
            Semantics::new(Role::Button).label(label).checked(idle.get() == secs).action(Action::Click)
        });
        w::tooltip(&b, if secs == 0 { "Never auto-lock".to_owned() } else { format!("Lock after {label} of inactivity") });
        row.child(b)
    });
    let presets = presets.semantics(Semantics::new(Role::Group).label("Auto-lock presets"));
    let head = w::row(ui).gap(unit * 2.0).align(Align::Start).fill_width().child(
        w::column(ui).gap(unit).grow(1.0).child(w::heading(ui, 2, "Auto-lock")).child(fineprint(
            ui,
            "Lock the vault after inactivity. Idle is checked every few seconds, so locking can lag slightly behind the timer.",
        )),
    );
    w::card(ui)
        .fill_width()
        .gap(unit * 2.5)
        .semantics(Semantics::new(Role::Group).label("Auto-lock timer"))
        .child(head.child(now))
        .child(presets)
        .child(panel_footer(
            ui,
            "Off keeps notes open until you lock or quit. Existing stickies are never touched by this setting.",
            "Done",
            panel,
        ))
}

/// Saves an auto-lock time (clamped to the vault's 0 ..= 12 h) and applies it.
pub fn set_auto_lock(st: &St, secs: u64) {
    let secs = secs.min(crate::vault::MAX_IDLE_LOCK_SECS);
    if checked(st, st.with(|v| v.set_idle_lock_secs(secs))).is_some() {
        st.idle_secs.set(secs);
        st.app.set_idle_timeout(state::idle_timeout(secs));
    }
}

#[cfg(test)]
mod tests {
    use super::format_auto_lock;

    #[test]
    fn auto_lock_reads_as_the_shipped_app_wrote_it() {
        assert_eq!(format_auto_lock(0), "Off");
        assert_eq!(format_auto_lock(60), "1 min");
        assert_eq!(format_auto_lock(900), "15 min");
        assert_eq!(format_auto_lock(3600), "1 hr");
        assert_eq!(format_auto_lock(5400), "1.5 hrs");
        assert_eq!(format_auto_lock(43_200), "12 hrs");
    }
}
