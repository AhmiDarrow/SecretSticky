//! A note window: frameless, in the note's color, pinned on top when asked, saved as you
//! type. The title bar holds the color swatches and the shipped app's actions.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use remedy_ui::WindowSpec;
use remedy_ui::core::{Color, Insets, Point, Size};
use remedy_ui::host::ResizeEdge;
use remedy_ui::input::{Buttons, Key, PointerPhase};
use remedy_ui::reactive::Signal;
use remedy_ui::style::{States, StylePatch};
use remedy_ui::view::Align;
use remedy_ui_a11y::{Action, Role, Semantics};
use remedy_ui_widgets_core::{self as w, Icon};
use zeroize::Zeroizing;

use crate::state::{self, COLORS, Debounce, OpenNote, St, checked, fill, ink, is_dark, window_title};
use crate::vault::{NOTE_DEFAULT_HEIGHT, NOTE_MAX_SIZE, NOTE_MIN_HEIGHT, NOTE_MIN_WIDTH, NoteColor, NoteDto};

/// How long typing pauses before a save.
pub const SAVE_DELAY: Duration = Duration::from_millis(400);
/// How long a copy from a note stays on the clipboard.
pub const COPY_KEEP: Duration = Duration::from_secs(30);
/// Where a new note starts (the vault's default) before it cascades.
const DEFAULT_ORIGIN: (f64, f64) = (120.0, 120.0);

/// Edits not yet written to the vault.
#[derive(Default)]
struct Pending {
    title: Option<Zeroizing<String>>,
    body: Option<Zeroizing<String>>,
    color: Option<NoteColor>,
    geometry: Option<(f64, f64, f64, f64)>,
    pinned: Option<bool>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.body.is_none()
            && self.color.is_none()
            && self.geometry.is_none()
            && self.pinned.is_none()
    }
}

/// Where a note opens: its saved place, or for a brand-new note at the default origin, a
/// step down and right picked from its id, so new notes do not stack exactly (the Tauri
/// app's cascade).
pub fn placement(dto: &NoteDto) -> (f64, f64, f64, f64) {
    let at_default = (dto.x - DEFAULT_ORIGIN.0).abs() < 0.5 && (dto.y - DEFAULT_ORIGIN.1).abs() < 0.5;
    let (x, y) = if at_default {
        let step = f64::from(dto.id.bytes().fold(0u32, |a, b| a.wrapping_add(u32::from(b))) % 8) * 28.0;
        (dto.x + step, dto.y + step)
    } else {
        (dto.x, dto.y)
    };
    let size = |v: f64, min: f64, default: f64| if v.is_finite() { v.clamp(min, NOTE_MAX_SIZE) } else { default };
    (x, y, size(dto.width, NOTE_MIN_WIDTH, NOTE_MIN_WIDTH), size(dto.height, NOTE_MIN_HEIGHT, NOTE_DEFAULT_HEIGHT))
}

/// Whether `e` is the platform's copy (`c`) or cut (`x`) shortcut.
fn is_clip_key(e: &remedy_ui::input::KeyEvent, letter: &str) -> bool {
    let primary = if cfg!(target_os = "macos") { e.modifiers.meta } else { e.modifiers.ctrl };
    e.pressed && primary && !e.modifiers.alt && matches!(&e.key, Key::Character(c) if c.eq_ignore_ascii_case(letter))
}

/// Opens the window for `dto`.
pub fn open(st: &St, dto: NoteDto) {
    let id = dto.id.clone();
    let (x, y, width, height) = placement(&dto);
    let spec = WindowSpec::new(window_title(&dto.title))
        .size(width as f32, height as f32)
        .min_size(NOTE_MIN_WIDTH as f32, NOTE_MIN_HEIGHT as f32)
        .position(x as f32, y as f32)
        .frameless()
        .always_on_top(dto.always_on_top)
        .capture_protected(true);

    let pending: Rc<RefCell<Pending>> = Rc::default();
    let saved = Signal::new(true);
    let color = Signal::new(dto.color.clone());
    let pinned = Signal::new(dto.always_on_top);
    let status = Signal::new(None::<(bool, String)>);
    let debounce = Debounce::default();
    let window_slot: Rc<RefCell<Option<remedy_ui::Window>>> = Rc::default();

    // Writes the pending edits; the manager's list and the title bar follow.
    let flush: Rc<dyn Fn()> = {
        let (st, pending, id, debounce, slot) = (st.clone(), pending.clone(), id.clone(), debounce.clone(), window_slot.clone());
        Rc::new(move || {
            debounce.cancel();
            let p = std::mem::take(&mut *pending.borrow_mut());
            if p.is_empty() {
                saved.set_if_changed(true);
                return;
            }
            let list_changed = p.title.is_some() || p.color.is_some();
            let take = |z: Option<Zeroizing<String>>| z.map(|mut z| std::mem::take(&mut *z));
            let (x, y, wd, ht) = match p.geometry {
                Some((x, y, wd, ht)) => (Some(x), Some(y), Some(wd), Some(ht)),
                None => (None, None, None, None),
            };
            let r = st.with(|v| v.update_note(&id, take(p.title), take(p.body), p.color, x, y, wd, ht, p.pinned));
            match r {
                Ok(note) => {
                    saved.set_if_changed(true);
                    if let Some(win) = slot.borrow().as_ref() {
                        win.set_title(window_title(&note.title));
                    }
                    if list_changed {
                        state::reload_notes(&st);
                    }
                }
                Err(crate::error::AppError::Locked) => {
                    let _ = checked(&st, Err::<(), _>(crate::error::AppError::Locked));
                }
                Err(e) => status.set(Some((true, e.to_string()))),
            }
        })
    };
    let schedule: Rc<dyn Fn()> = {
        let (app, flush, debounce) = (st.app.clone(), flush.clone(), debounce.clone());
        Rc::new(move || {
            saved.set_if_changed(false);
            let f = flush.clone();
            debounce.call(&app, SAVE_DELAY, move || f());
        })
    };

    let (st2, flush2, schedule2, pending2) = (st.clone(), flush.clone(), schedule.clone(), pending.clone());
    let body_text = Zeroizing::new(dto.body.clone());
    let title_text = Zeroizing::new(dto.title.clone());
    drop(dto);
    let window = st.app.open(spec, move |ui, win| {
        let st = st2;
        let unit = ui.theme().spacing.unit;
        let tool_style = move || StylePatch::new().background(Color::TRANSPARENT).foreground(ink(&color.get()));
        let tool = |glyph: Icon, label: &str, f: Box<dyn Fn()>| w::icon_button(ui, glyph, label, f).variant("quiet").style(tool_style);

        // A short message under the text (copies, errors), gone after a moment.
        let hint = Debounce::default();
        let say: Rc<dyn Fn(bool, String, Duration)> = {
            let (app, hint) = (st.app.clone(), hint.clone());
            Rc::new(move |error, text, keep| {
                status.set(Some((error, text)));
                hint.call(&app, keep, move || status.set(None));
            })
        };

        let title = w::text_input(ui, "Title").placeholder("Title");
        title.set_value(&title_text);
        let body = w::text_area(ui, "Note").placeholder("Secrets, API keys, passwords…");
        body.set_value(&body_text);

        // Copying from a note goes through the secret clipboard: kept out of clipboard
        // history and cleared after 30 seconds, as the shipped app's copy did.
        let copy_out: Rc<dyn Fn(String, &str)> = {
            let (app, say) = (st.app.clone(), say.clone());
            Rc::new(move |text: String, what: &str| {
                if text.is_empty() {
                    say(false, "Nothing to copy".to_owned(), Duration::from_millis(1500));
                    return;
                }
                app.copy_secret(text, COPY_KEEP);
                say(false, format!("{what} · clears in {}s", COPY_KEEP.as_secs()), Duration::from_millis(2500));
            })
        };
        for f in [&title, &body] {
            let (f2, copy_out) = (f.clone(), copy_out.clone());
            f.intercept_keys(move |e| {
                let (copy, cut) = (is_clip_key(e, "c"), is_clip_key(e, "x"));
                if !copy && !cut {
                    return false;
                }
                let range = f2.selection();
                if range.is_empty() {
                    return true;
                }
                let text = f2.value().get(range.clone()).unwrap_or_default().to_owned();
                copy_out(text, "Copied");
                if cut {
                    f2.replace_range(range, "");
                }
                true
            });
        }

        // Title bar: the colors, then the actions. Drag anywhere that is not a control.
        let swatches = COLORS.iter().fold(w::wrap(ui).gap(0.0), |row, (c, name)| {
            let (pick, pending, schedule) = (c.clone(), pending2.clone(), schedule2.clone());
            row.child(crate::manager::swatch(ui, c, 16.0, (*name).to_owned(), Some(color), move || {
                color.set(pick.clone());
                pending.borrow_mut().color = Some(pick.clone());
                schedule();
            }))
        });
        let saving = w::text(ui, move || if saved.get() { "✓".to_owned() } else { "…".to_owned() })
            .style(move || StylePatch::new().foreground(ink(&color.get()).with_alpha(0.7)))
            .semantics(move || Semantics::new(Role::Status).label(if saved.get() { "Saved" } else { "Saving…" }));
        let pin = {
            let (win, pending, schedule) = (win.clone(), pending2.clone(), schedule2.clone());
            tool(
                Icon::Pin,
                "Always on top",
                Box::new(move || {
                    let on = !pinned.get();
                    pinned.set(on);
                    win.set_always_on_top(on);
                    pending.borrow_mut().pinned = Some(on);
                    schedule();
                }),
            )
        };
        let pin2 = pin.clone();
        let pin = pin.bind(move || pin2.state(States::CHECKED, pinned.get())).semantics(move || {
            Semantics::new(Role::Button)
                .label(if pinned.get() { "Unpin" } else { "Always on top" })
                .checked(pinned.get())
                .action(Action::Click)
        });
        let (s1, s2, s3) = (st.clone(), st.clone(), st.clone());
        let copy = {
            let (body, copy_out) = (body.clone(), copy_out.clone());
            tool(
                Icon::Copy,
                "Copy selection or full body (auto-clears in 30s)",
                Box::new(move || {
                    let range = body.selection();
                    let text = body.value();
                    match text.get(range.clone()).filter(|s| !s.is_empty()) {
                        Some(sel) => copy_out(sel.to_owned(), "Copied selection"),
                        None => copy_out(text, "Copied body"),
                    }
                }),
            )
        };
        let (closing_win, closing_flush) = (win.clone(), flush2.clone());
        let actions = w::row(ui)
            .gap(unit * 0.5)
            .align(Align::Center)
            .child(tool(Icon::Plus, "New note (same color)", Box::new(move || state::new_note(&s1, color.get()))))
            .child(tool(Icon::Note, "Open manager", Box::new(move || state::show_manager(&s2))))
            .child(tool(Icon::Lock, "Lock vault", Box::new(move || state::lock_all(&s3))))
            .child(saving)
            .child(copy)
            .child(pin)
            .child(tool(
                Icon::Close,
                "Close",
                Box::new(move || {
                    closing_flush();
                    closing_win.close();
                }),
            ));
        let mover = win.clone();
        let bar = w::row(ui)
            .fill_width()
            .gap(unit * 2.0)
            .padding(Insets::symmetric(unit, unit * 2.0))
            .align(Align::Center)
            .style(move || StylePatch::new().background(ink(&color.get()).with_alpha(0.06)))
            .child(swatches.grow(1.0))
            .child(actions)
            .drag_region(move || mover.begin_move());

        {
            let (t, pending, schedule) = (title.clone(), pending2.clone(), schedule2.clone());
            let _ = title.clone().on_change(move || {
                pending.borrow_mut().title = Some(Zeroizing::new(t.value()));
                schedule();
            });
            let (b, pending, schedule) = (body.clone(), pending2.clone(), schedule2.clone());
            let _ = body.clone().on_change(move || {
                pending.borrow_mut().body = Some(Zeroizing::new(b.value()));
                schedule();
            });
        }
        let field_style = move || StylePatch::new().foreground(ink(&color.get()));
        let field_variant = move || if is_dark(&color.get()) { "note_dark" } else { "note_light" };
        let title_size = ui.theme().type_scale.body + 2.0;
        let title_el = title
            .el()
            .variant(field_variant)
            .style(move || field_style().text_size(title_size).weight(700.0))
            .fill_width();
        let body_el = body.el().variant(field_variant).style(field_style).fill_width().grow(1.0);

        let u = ui.clone();
        let message = crate::state::when(ui, move || status.with(Option::is_some), move || {
            let (error, text) = status.get_untracked().ok().flatten().unwrap_or_default();
            let small = u.theme().type_scale.small;
            w::text(&u, text.clone())
                .style(move || {
                    let fg = if error { Color::from_hex("#b91c1c").unwrap_or(Color::BLACK) } else { ink(&color.get()) };
                    StylePatch::new().foreground(fg).text_size(small)
                })
                .semantics(Semantics::new(Role::Status).label(text))
        });

        // A corner grip resizes the frameless window.
        let sizer = win.clone();
        let grip = w::canvas(ui, move |cx| {
            let c = ink(&color.get()).with_alpha(0.4);
            // Six dots in a triangle against the corner.
            for (i, j) in [(1, 1), (2, 1), (3, 1), (1, 2), (2, 2), (1, 3)] {
                let (x, y) = (cx.size.width - 4.0 * i as f32 - 1.0, cx.size.height - 4.0 * j as f32 - 1.0);
                cx.list.fill_rect(remedy_ui::core::Rect::new(x, y, x + 2.0, y + 2.0), remedy_ui_paint::Brush::Solid(c));
            }
        })
        .width(16.0)
        .height(16.0)
        // Keyboard users resize through the system's window menu (Alt+Space, then Size).
        .keyboard_alternative("The window menu's Size command")
        .on_pointer(move |e| {
            if e.phase == PointerPhase::Down && e.buttons.contains(Buttons::PRIMARY) {
                sizer.begin_resize(ResizeEdge::BottomRight);
                return true;
            }
            false
        });
        let footer = w::row(ui)
            .fill_width()
            .align(Align::End)
            .padding(crate::state::pad(0.0, 0.0, 0.0, unit * 3.0))
            .child(message.grow(1.0))
            .child(grip);

        let content = w::column(ui)
            .grow(1.0)
            .fill_width()
            .gap(unit)
            .padding(crate::state::pad(unit * 2.0, unit * 3.0, 0.0, unit * 3.0))
            .child(title_el)
            .child(body_el);
        let note_style = move || {
            let c = color.get();
            StylePatch::new().background(fill(&c)).foreground(ink(&c))
        };
        ui.root().child(w::column(ui).grow(1.0).fill_width().style(note_style).child(bar).child(content).child(footer));
        body.el().focus();
    });
    *window_slot.borrow_mut() = Some(window.clone());

    // Moves and resizes are saved too, after they settle. The place the window opened at is
    // not a move: a note nobody moved keeps its saved geometry, as in the Tauri app.
    {
        let (pending, schedule) = (pending.clone(), schedule.clone());
        let opened = std::cell::Cell::new(Some((x, y, width, height)));
        window.on_geometry(move |p: Point, s: Size| {
            let now = (f64::from(p.x), f64::from(p.y), f64::from(s.width), f64::from(s.height));
            let same = |a: (f64, f64, f64, f64)| {
                (a.0 - now.0).abs() < 0.5 && (a.1 - now.1).abs() < 0.5 && (a.2 - now.2).abs() < 0.5 && (a.3 - now.3).abs() < 0.5
            };
            if opened.get().is_some_and(same) {
                return;
            }
            opened.set(None);
            pending.borrow_mut().geometry = Some(now);
            schedule();
        });
    }
    {
        let (st, id, flush) = (st.clone(), id.clone(), flush.clone());
        // Closed by the user: save. Lock and delete take the note out of `open` first (lock
        // has saved already; a deleted note has nothing to save to).
        window.on_closed(move || {
            let was_open = st.open.borrow_mut().remove(&id).is_some();
            if was_open {
                flush();
            }
        });
    }
    st.open.borrow_mut().insert(id, OpenNote { window, flush });
}

#[cfg(test)]
mod tests {
    use super::placement;
    use crate::vault::{NOTE_MAX_SIZE, NOTE_MIN_HEIGHT, NOTE_MIN_WIDTH, NoteColor, NoteDto};

    fn dto(id: &str, x: f64, y: f64, w: f64, h: f64) -> NoteDto {
        let now = chrono::Utc::now();
        NoteDto {
            id: id.into(),
            title: String::new(),
            body: String::new(),
            color: NoteColor::Yellow,
            color_css: String::new(),
            color_text_css: String::new(),
            x,
            y,
            width: w,
            height: h,
            always_on_top: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn new_notes_cascade_and_saved_places_are_kept() {
        let (x, y, ..) = placement(&dto("ab", 120.0, 120.0, 345.0, 280.0));
        let step = f64::from((u32::from(b'a') + u32::from(b'b')) % 8) * 28.0;
        assert_eq!((x, y), (120.0 + step, 120.0 + step));
        assert_eq!(placement(&dto("ab", 300.0, 40.0, 400.0, 300.0)), (300.0, 40.0, 400.0, 300.0));
    }

    #[test]
    fn sizes_are_clamped_like_the_tauri_app() {
        let (_, _, w, h) = placement(&dto("a", 0.0, 0.0, 10.0, f64::NAN));
        assert_eq!((w, h), (NOTE_MIN_WIDTH, crate::vault::NOTE_DEFAULT_HEIGHT));
        let (_, _, w, h) = placement(&dto("a", 0.0, 0.0, 5000.0, 5000.0));
        assert_eq!((w, h), (NOTE_MAX_SIZE, NOTE_MAX_SIZE));
        assert!(NOTE_MIN_HEIGHT > 0.0);
    }
}
