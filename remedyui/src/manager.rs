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
use crate::lang::{LOCALES, Lang, Msg};
use crate::state::{self, COLORS, Preview, Screen, St, checked, color_name, fill};

/// How long a copied recovery key stays on the clipboard.
const RECOVERY_COPY_KEEP: Duration = Duration::from_secs(120);
/// The shortest master password the app accepts.
pub const MIN_PASSWORD: usize = 12;

/// Auto-lock presets in seconds, Off … 12 h, as the shipped app offers them
/// (`AUTO_LOCK_PRESET_SECS`); their names come from [`Lang::duration`].
pub const AUTO_LOCK: [u64; 10] = [0, 60, 300, 900, 1800, 3600, 7200, 14_400, 28_800, 43_200];

/// The project links About offers (the shipped app's allowlist), by catalog key.
pub const LINKS: [(&str, &str); 3] = [
    ("about.githubProfile", "https://github.com/AhmiDarrow"),
    ("about.projectRepo", "https://github.com/AhmiDarrow/SecretSticky"),
    ("about.releases", "https://github.com/AhmiDarrow/SecretSticky/releases"),
];

/// Which panel the header has open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    None,
    About,
    Password,
    AutoLock,
    Language,
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
    let (e, l) = (st.error, st.lang);
    let u = ui.clone();
    let now = move || e.get().map(|m| m.text(l)).unwrap_or_default();
    remedy_ui::show(ui, move || e.with(Option::is_some), move || {
        w::text(&u, now).variant("error").semantics(move || Semantics::new(Role::Status).label(now()))
    }).el()
    .fill_width()
}

fn fineprint(ui: &Ui, s: impl remedy_ui::view::IntoProp<String>) -> El {
    w::text(ui, s).variant("muted").style(StylePatch::new().text_size(ui.theme().type_scale.small))
}

/// Setup (a new vault) or unlock (the master password, or the recovery key).
fn gate(st: &St, ui: &Ui, setup: bool, recovery: bool, use_recovery: Signal<bool>) -> El {
    let unit = ui.theme().spacing.unit;
    let (busy, l) = (st.busy, st.lang);
    let (title, hint) =
        if setup { ("unlock.titleSetup", "unlock.blurbSetup") } else { ("unlock.titleUnlock", "unlock.blurbUnlock") };
    let label = if recovery { "unlock.recoveryKey" } else { "unlock.masterPassword" };
    let pass = w::secret_input(ui, l.t(label));
    l.name_field(&pass, label, None);
    if recovery {
        // The key is typed from paper: shown as typed, as the shipped app did.
        pass.set_reveal(true);
    }
    let confirm = setup.then(|| {
        let c = w::secret_input(ui, l.t("unlock.confirmPassword"));
        l.name_field(&c, "unlock.confirmPassword", None);
        c
    });

    let submit: Rc<dyn Fn()> = {
        let (st, pass, confirm) = (st.clone(), pass.clone(), confirm.clone());
        Rc::new(move || submit(&st, &pass, confirm.as_ref(), setup, recovery))
    };
    let s1 = submit.clone();
    let pass = pass.on_submit(move || s1());
    let go = w::button(
        ui,
        move || {
            l.t(if busy.get() {
                "common.working"
            } else if setup {
                "unlock.createVault"
            } else {
                "common.unlock"
            })
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
        .child(w::field(ui, l.k(label), pass.el().fill_width().disabled(move || busy.get())));
    if let Some(c) = &confirm {
        let s2 = submit.clone();
        let c = c.clone().on_submit(move || s2());
        form = form.child(w::field(ui, l.k("unlock.confirmPassword"), c.el().fill_width().disabled(move || busy.get())));
    }
    form = form.child(error_line(st, ui)).child(go);

    let mut escapes = w::column(ui).gap(unit).fill_width().align(Align::Start);
    if !setup && st.has_recovery.get_untracked().unwrap_or(false) {
        let (st, label) = (st.clone(), if recovery { "unlock.useMaster" } else { "unlock.useRecovery" });
        escapes = escapes.child(
            w::button(ui, l.k(label), move || {
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
            tip(
                w::button(ui, l.k(if setup { "unlock.hideTraySetup" } else { "unlock.hideTray" }), move || state::hide_manager(&s1))
                    .variant("quiet")
                    .disabled(move || busy.get()),
                l.k("unlock.hideTrayTitle"),
            ),
        )
        .child(tip(
            w::button(ui, l.k("common.quit"), move || state::quit(&s2)).variant("danger").disabled(move || busy.get()),
            l.k("unlock.quitTitle"),
        ));

    let card = w::card(ui).fill_width().gap(unit * 3.0).child(mark(ui, 44.0)).child(title_text(ui, l.k(title))).child(
        w::text(ui, l.k(hint)).variant("muted"),
    );
    let card = card
        .child(form)
        .child(escapes)
        .child(language_select(st, ui))
        .child(fineprint(ui, l.k("unlock.fineprint")));
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
            st.error.set(Some(Msg::key("unlock.errShort")));
            return;
        }
        if again.as_ref().map(|a| a.expose()) != Some(pw.expose()) {
            st.error.set(Some(Msg::key("unlock.errMismatch")));
            return;
        }
    } else if pw.is_empty() {
        // What the shipped app said for an empty password, without spending a key derivation.
        st.error.set(Some(Msg::key("error.badPassword")));
        return;
    } else if let Err(e) = st.throttle.borrow_mut().check() {
        st.error.set(Some(e.into()));
        return;
    }
    let Some(vault) = st.shared() else {
        st.error.set(Some(Msg::Error(st.load_error.clone())));
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
                    s.error.set(Some(e.into()));
                }
            }
        },
    );
}

/// The recovery key, shown once after setup.
fn recovery_key(st: &St, ui: &Ui) -> El {
    let (unit, l) = (ui.theme().spacing.unit, st.lang);
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
        .semantics(move || Semantics::new(Role::Label).label(l.t("unlock.recoveryKey")));
    w::card(ui)
        .fill_width()
        .gap(unit * 3.0)
        .child(mark(ui, 44.0))
        .child(title_text(ui, l.k("recovery.title")))
        .child(strong_text(ui, l, "recovery.body"))
        .child(w::card(ui).fill_width().variant("inset").child(shown))
        .child(w::button(ui, l.k("recovery.copyContinue"), copy).variant("primary").fill_width())
        .child(w::button(ui, l.k("recovery.savedContinue"), done).variant("quiet").fill_width())
        .child(language_select(st, ui))
        .child(fineprint(ui, l.k("recovery.fineprint")))
}

fn broken(st: &St, ui: &Ui) -> El {
    let (s, l, why) = (st.clone(), st.lang, st.load_error.clone());
    w::card(ui)
        .fill_width()
        .gap(ui.theme().spacing.unit * 3.0)
        .child(mark(ui, 44.0))
        .child(title_text(ui, "SecretSticky"))
        .child(w::text(ui, move || crate::lang::localize_error(l, &why)).variant("error"))
        .child(w::button(ui, l.k("common.quit"), move || state::quit(&s)).variant("danger"))
}

/// The unlocked manager.
fn unlocked(st: &St, ui: &Ui, updater: &Rc<crate::update::Updater>) -> El {
    let unit = ui.theme().spacing.unit;
    let panel = Signal::new(Panel::None);
    let busy = st.busy;
    let notes = st.notes;
    let idle = st.idle_secs;
    let l = st.lang;

    // Header: brand and the six actions.
    let brand = w::row(ui)
        .gap(unit * 3.0)
        .align(Align::Center)
        .child(mark(ui, 40.0))
        .child(
            w::column(ui)
                .gap(unit * 0.5)
                .child(title_text(ui, "SecretSticky"))
                .child(w::text(ui, l.k("manager.tagline")).variant("muted")),
        );
    // Equal buttons in small type that wrap when a language's words are long (0.2.0's
    // `flex: 1 1 5.5rem` header).
    let compact = StylePatch::new()
        .text_size(ui.theme().type_scale.small)
        .padding(remedy_ui::core::Insets::symmetric(unit * 1.5, unit));
    let head_button = |b: El| b.grow(1.0).style(compact.clone()).layout(|l| l.basis = remedy_ui::layout::Length::Px(88.0));
    let toggle = |key: &'static str, which: Panel| {
        let b = head_button(w::button(ui, l.k(key), move || panel.update(|p| *p = if *p == which { Panel::None } else { which })))
            .disabled(move || busy.get());
        b.semantics(move || Semantics::new(Role::Button).label(l.t(key)).expanded(panel.get() == which).action(Action::Click))
    };
    let auto = toggle("manager.autoLock", Panel::AutoLock).variant(move || if idle.get() == 0 { "" } else { "primary" });
    w::tooltip(&auto, move || {
        let secs = idle.get();
        if secs == 0 { l.t("autoLock.titleOff") } else { l.tv("autoLock.titleAfter", &[("duration", &l.duration(secs))]) }
    });
    let (s1, s2) = (st.clone(), st.clone());
    let actions = w::wrap(ui)
        .gap(unit * 1.5)
        .fill_width()
        .child(toggle("manager.about", Panel::About))
        .child(toggle("manager.password", Panel::Password))
        .child(auto)
        .child(toggle("manager.language", Panel::Language))
        .child(head_button(w::button(ui, l.k("manager.openAll"), move || state::open_all(&s1))).disabled(move || busy.get()))
        .child(
            head_button(w::button(ui, l.k("manager.lock"), move || state::lock_all(&s2)))
                .variant("danger")
                .disabled(move || busy.get()),
        );
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
    let (st_g, ui_g) = (st.clone(), ui.clone());
    let language = remedy_ui::show(ui, move || panel.get() == Panel::Language, move || language(&st_g, &ui_g, panel)).el();

    // New note: one swatch per color.
    let swatches = COLORS.iter().fold(w::wrap(ui).gap(unit * 1.5), |row, (c, _)| {
        let (st, c2, c3) = (st.clone(), c.clone(), c.clone());
        let label = move || l.tv("notes.newColor", &[("color", &color_name(l, &c3).to_lowercase())]);
        row.child(swatch(ui, l, c, 28.0, label, None, move || state::new_note(&st, c2.clone())))
    });
    let new_row = w::row(ui)
        .gap(unit * 3.0)
        .align(Align::Center)
        .fill_width()
        .child(w::text(ui, l.k("notes.new")).variant("muted"))
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
        w::text(&ui3, l.k("notes.empty"))
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
        .child(language.fill_width())
        .child(new_row)
        .child(error_line(st, ui))
        .child(list)
        .child(empty.fill_width().align(Align::Center));

    // Footer.
    let count = w::text(ui, move || l.count("manager.count", "count", notes.with(Vec::len) as f64));
    let small = ui.theme().type_scale.small;
    let meta = w::column(ui)
        .gap(unit * 0.5)
        .child(count.style(StylePatch::new().text_size(small)))
        .child(fineprint(ui, l.k("notes.trayHint")))
        .child(fineprint(ui, move || l.tv("notes.copyClears", &[("seconds", &crate::note::COPY_KEEP.as_secs().to_string())])));
    let (s3, s4) = (st.clone(), st.clone());
    let hide = w::button(ui, l.k("notes.hide"), move || state::hide_manager(&s3)).variant("quiet").disabled(move || busy.get());
    w::tooltip(&hide, l.k("notes.hideTitle"));
    let quit = w::button(ui, l.k("common.quit"), move || state::quit(&s4)).variant("danger").disabled(move || busy.get());
    w::tooltip(&quit, l.k("notes.quitTitle"));
    let footer = w::column(ui)
        .gap(unit * 2.0)
        .fill_width()
        .padding(crate::state::pad(unit * 3.0, unit * 4.5, unit * 3.5, unit * 4.5))
        .child(meta)
        .child(w::wrap(ui).gap(unit * 1.5).fill_width().justify(Justify::End).child(hide).child(quit));

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
    l: Lang,
    c: &crate::vault::NoteColor,
    dot: f32,
    label: impl Fn() -> String + 'static,
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
            b.semantics(move || Semantics::new(Role::Button).label(label()).checked(sel.get() == me).action(Action::Click))
        }
        None => b.semantics(move || Semantics::new(Role::Button).label(label()).action(Action::Click)),
    };
    let named = c.clone();
    w::tooltip(&b, move || color_name(l, &named));
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
    let l = st.lang;
    let raw = p.title.clone();
    let title = move || state::shown_title(l, &raw);
    let pc = p.color.clone();
    let sub = move || l.tv("notes.openToEdit", &[("color", &color_name(l, &pc))]);
    let (st1, st2, id1, id2, t2, ui2) = (st.clone(), st.clone(), p.id.clone(), p.id.clone(), title.clone(), ui.clone());
    let (t3, t4) = (title.clone(), title.clone());
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
                .child(fineprint(ui, sub)),
        )
        .on_click(move || state::open_note(&st1, &id1))
        // "Open <title>": the shipped row's own name (its text), not a catalog string.
        .semantics(move || Semantics::new(Role::Button).label(format!("Open {}", t3())).action(Action::Click));
    let delete = w::icon_button(ui, w::Icon::Close, move || l.tv("notes.deleteAria", &[("title", &t4())]), move || {
        panel.set(Panel::None);
        let (st, id) = (st2.clone(), id2.clone());
        let _ = w::dialog(
            &ui2,
            l.t("delete.title"),
            l.tv("delete.body", &[("title", &t2())]),
            vec![
                DialogButton::new(l.t("common.cancel"), "quiet", || {}),
                DialogButton::new(l.t("common.delete"), "danger_solid", move || state::delete_note(&st, &id)),
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
fn panel_card(ui: &Ui, l: Lang, title: &'static str, help: &'static str) -> El {
    let unit = ui.theme().spacing.unit;
    w::card(ui)
        .fill_width()
        .gap(unit * 2.0)
        .semantics(move || Semantics::new(Role::Group).label(l.t(title)))
        .child(w::heading(ui, 2, l.k(title)))
        .child(fineprint(ui, l.k(help)))
}

fn panel_footer(
    ui: &Ui,
    meta: impl remedy_ui::view::IntoProp<String>,
    close: impl remedy_ui::view::IntoProp<String>,
    panel: Signal<Panel>,
) -> El {
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
    let (unit, l) = (ui.theme().spacing.unit, st.lang);
    let links = LINKS.iter().enumerate().fold(w::wrap(ui).gap(unit * 1.5).fill_width(), |row, (i, (key, url))| {
        let (app, err, url) = (st.app.clone(), st.error, (*url).to_owned());
        let b = w::button(ui, l.k(key), move || {
            app.open_url(url.clone(), move |r| {
                if let Err(e) = r {
                    err.set(Some(Msg::Key("error.openUrl", vec![("detail", format!("{e:?}"))])));
                }
            });
        })
        .grow(1.0)
        .layout(|l| l.basis = remedy_ui::layout::Length::Px(112.0));
        row.child(if i == 0 { b.variant("primary") } else { b })
    });
    let body = w::column(ui)
        .gap(unit * 1.5)
        .grow(1.0)
        .child(w::heading(ui, 2, l.k("about.title")))
        .child(w::text(ui, l.k("about.hello")).style(StylePatch::new().weight(600.0)))
        .child(fineprint(ui, l.k("about.blurb")))
        .child(fineprint(ui, move || l.tv("about.version", &[("version", env!("CARGO_PKG_VERSION"))])))
        .child(links)
        .child(crate::update::section(st, ui, updater))
        .child(panel_footer(ui, l.k("about.localVault"), l.k("common.close"), panel));
    w::card(ui)
        .fill_width()
        .semantics(move || Semantics::new(Role::Group).label(l.t("about.aria")))
        .child(w::row(ui).gap(unit * 3.0).align(Align::Start).fill_width().child(mark(ui, 44.0)).child(body))
}

fn password(st: &St, ui: &Ui, panel: Signal<Panel>) -> El {
    let (unit, l) = (ui.theme().spacing.unit, st.lang);
    let input = |key: &'static str| {
        let f = w::secret_input(ui, l.t(key));
        l.name_field(&f, key, None);
        f
    };
    let (current, new, again) = (input("password.current"), input("password.new"), input("password.confirm"));
    let busy = Signal::new(false);
    let msg = Signal::new(None::<Msg>);
    let toast_ui = ui.clone();
    let save: Rc<dyn Fn()> = {
        let (st, current, new, again) = (st.clone(), current.clone(), new.clone(), again.clone());
        Rc::new(move || {
            if busy.get_untracked().unwrap_or(false) {
                return;
            }
            let (c, n, a) = (current.take_secret(), new.take_secret(), again.take_secret());
            if n.expose().chars().count() < MIN_PASSWORD {
                msg.set(Some(Msg::key("password.errShort")));
                return;
            }
            if n.expose() != a.expose() {
                msg.set(Some(Msg::key("password.errMismatch")));
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
                            let _ = w::toast(&toast_ui, l.t("password.updated"), Duration::from_secs(4));
                        }
                        Err(e) => msg.set(Some(e.into())),
                    }
                },
            );
        })
    };
    let (s1, s2, s3) = (save.clone(), save.clone(), save.clone());
    let field = |key: &'static str, f: &TextField, s: Rc<dyn Fn()>| {
        w::field(ui, l.k(key), f.clone().on_submit(move || s()).el().fill_width().disabled(move || busy.get()))
    };
    let u = ui.clone();
    let now = move || msg.get().map(|m| m.text(l)).unwrap_or_default();
    let message = remedy_ui::show(ui, move || msg.with(Option::is_some), move || {
        w::text(&u, now).variant("error").semantics(move || Semantics::new(Role::Status).label(now()))
    }).el();
    let actions = w::row(ui)
        .gap(unit * 2.0)
        .child(
            w::button(ui, move || l.t(if busy.get() { "password.updating" } else { "password.submit" }), move || {
                s3()
            })
            .variant("primary")
            .disabled(move || busy.get()),
        )
        .child(
            w::button(ui, l.k("common.cancel"), move || {
                msg.set(None);
                panel.set(Panel::None);
            })
            .variant("quiet")
            .disabled(move || busy.get()),
        );
    current.el().focus();
    panel_card(ui, l, "password.title", "password.blurb")
        .child(field("password.current", &current, s1))
        .child(field("password.new", &new, s2))
        .child(field("password.confirm", &again, save))
        .child(message.fill_width())
        .child(actions)
}

fn auto_lock(st: &St, ui: &Ui, panel: Signal<Panel>) -> El {
    let (unit, l) = (ui.theme().spacing.unit, st.lang);
    let idle = st.idle_secs;
    let busy = st.busy;
    let now = w::badge(ui, move || {
        let s = idle.get();
        if s == 0 { l.t("autoLock.off") } else { l.tv("autoLock.now", &[("duration", &l.duration(s))]) }
    })
    .nowrap()
    // Keeps its width; the help text beside it wraps instead.
    .layout(|l| l.shrink = 0.0)
    .semantics(move || {
        let s = idle.get();
        Semantics::new(Role::Status).label(if s == 0 {
            l.t("autoLock.titleOff")
        } else {
            l.tv("autoLock.titleAfter", &[("duration", &l.duration(s))])
        })
    });
    let presets = AUTO_LOCK.iter().fold(w::wrap(ui).gap(unit * 1.5).fill_width(), |row, &secs| {
        let st = st.clone();
        let label = move || l.duration(secs);
        let b = w::button(ui, label, move || set_auto_lock(&st, secs))
            .grow(1.0)
            .layout(|l| l.basis = remedy_ui::layout::Length::Px(68.0))
            .variant(move || if idle.get() == secs { "primary" } else { "" })
            .disabled(move || busy.get());
        let b = b.semantics(move || {
            Semantics::new(Role::Button).label(label()).checked(idle.get() == secs).action(Action::Click)
        });
        w::tooltip(&b, move || if secs == 0 { l.t("autoLock.never") } else { l.tv("autoLock.lockAfter", &[("duration", &label())]) });
        row.child(b)
    });
    let presets = presets.semantics(move || Semantics::new(Role::Group).label(l.t("autoLock.presetsAria")));
    let head = w::row(ui).gap(unit * 2.0).align(Align::Start).fill_width().child(
        w::column(ui)
            .gap(unit)
            .grow(1.0)
            .child(w::heading(ui, 2, l.k("autoLock.title")))
            .child(fineprint(ui, l.k("autoLock.blurb"))),
    );
    w::card(ui)
        .fill_width()
        .gap(unit * 2.5)
        .semantics(move || Semantics::new(Role::Group).label(l.t("autoLock.aria")))
        .child(head.child(now))
        .child(presets)
        .child(panel_footer(ui, l.k("autoLock.footnote"), l.k("autoLock.done"), panel))
}

/// The Language panel (0.2.0): every shipped language by its own name, the current one
/// highlighted; a choice applies at once.
fn language(st: &St, ui: &Ui, panel: Signal<Panel>) -> El {
    let (unit, l) = (ui.theme().spacing.unit, st.lang);
    let grid = LOCALES.iter().fold(w::wrap(ui).gap(unit * 1.5).fill_width(), |row, &(id, native)| {
        let st = st.clone();
        let b = w::button(ui, native, move || state::set_language(&st, id))
            .grow(1.0)
            .layout(|l| l.basis = remedy_ui::layout::Length::Px(136.0))
            .variant(move || if l.id() == id { "primary" } else { "" });
        let b = b.semantics(move || Semantics::new(Role::Button).label(native).checked(l.id() == id).action(Action::Click));
        row.child(b)
    });
    let grid = grid.semantics(move || Semantics::new(Role::Group).label(l.t("language.aria")));
    // The typeface for every window: Inter (as shipped) or one of Ahmi Darrow's families.
    let font = st.font;
    let fonts = crate::fonts::CHOICES.iter().fold(w::wrap(ui).gap(unit * 1.5).fill_width(), |row, &(id, _, name)| {
        let st = st.clone();
        let b = w::button(ui, name, move || state::set_font(&st, id))
            .grow(1.0)
            .layout(|l| l.basis = remedy_ui::layout::Length::Px(136.0))
            .variant(move || if font.get() == id { "primary" } else { "" });
        row.child(b.semantics(move || Semantics::new(Role::Button).label(name).checked(font.get() == id).action(Action::Click)))
    });
    let fonts = fonts.semantics(move || Semantics::new(Role::Group).label(l.t("font.title")));
    panel_card(ui, l, "language.title", "language.blurb")
        .child(grid)
        .child(w::heading(ui, 3, l.k("font.title")))
        .child(fineprint(ui, l.k("font.blurb")))
        .child(fonts)
        .child(panel_footer(ui, move || format!("{}: {}", l.t("language.current"), l.native()), l.k("common.close"), panel))
}

/// The compact language picker under the unlock, setup and recovery forms (0.2.0's
/// `LanguageSelect`): there before any vault exists.
fn language_select(st: &St, ui: &Ui) -> El {
    let (unit, l) = (ui.theme().spacing.unit, st.lang);
    let index = |id: &str| LOCALES.iter().position(|(x, _)| *x == id).unwrap_or(0);
    let chosen = Signal::new(index(&remedy_ui::reactive::untrack(|| l.id())));
    let st2 = st.clone();
    let names = LOCALES.iter().map(|(_, n)| (*n).to_owned()).collect();
    let pick = w::select(ui, l.k("language.aria"), names, chosen).fill_width().justify(Justify::SpaceBetween);
    // A choice saves and applies; a switch made elsewhere moves the picker too.
    let _ = pick.clone().bind(move || {
        let (id, _) = LOCALES[chosen.get().min(LOCALES.len() - 1)];
        if remedy_ui::reactive::untrack(|| l.id()) != id {
            remedy_ui::reactive::untrack(|| state::set_language(&st2, id));
        }
    });
    let _ = pick.clone().bind(move || {
        let i = index(&l.id());
        remedy_ui::reactive::untrack(|| chosen.set_if_changed(i));
    });
    w::column(ui).gap(unit).fill_width().child(fineprint(ui, l.k("language.current"))).child(pick)
}

/// A screen or brand title at the shipped `h1` size (1.35rem).
fn title_text(ui: &Ui, s: impl remedy_ui::view::IntoProp<String>) -> El {
    let size = ui.theme().type_scale.body * 1.35;
    w::heading(ui, 1, s).style(StylePatch::new().text_size(size).weight(700.0))
}

/// Muted catalog text with its one `<strong>` run bold (the shipped `RichText`), following
/// the language.
fn strong_text(ui: &Ui, l: Lang, key: &'static str) -> El {
    use remedy_ui::view::IntoProp;
    let rich = (move || {
        let mut r = remedy_ui::text::RichText::new();
        for (run, strong) in crate::lang::strong_runs(&l.t(key)) {
            if strong {
                r.push_styled(&run, remedy_ui::text::TextStylePatch::bold());
            } else {
                r.push_text(&run);
            }
        }
        r
    })
    .into_prop();
    ui.el("label").variant("muted").text(rich)
}

/// `b` with a tooltip.
fn tip(b: El, text: impl Fn() -> String + 'static) -> El {
    w::tooltip(&b, text);
    b
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
    #[test]
    fn auto_lock_presets_are_the_shipped_ones() {
        assert_eq!(super::AUTO_LOCK, [0, 60, 300, 900, 1800, 3600, 7200, 14_400, 28_800, 43_200]);
    }
}
