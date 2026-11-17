//! A note window: frameless, always on top when pinned, saved as you type.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use remedy_ui::core::{Color, Insets, Point, Size};
use remedy_ui::host::ResizeEdge;
use remedy_ui::input::{Buttons, PointerPhase};
use remedy_ui::reactive::Signal;
use remedy_ui::style::{States, StylePatch};
use remedy_ui::view::Align;
use remedy_ui::WindowSpec;
use remedy_ui_widgets_core::{self as w, Icon, MenuItem};
use zeroize::Zeroizing;

use crate::state::{self, COLORS, Debounce, OpenNote, St, checked, fill, ink};
use crate::vault::{NOTE_MIN_HEIGHT, NOTE_MIN_WIDTH, NoteColor, NoteDto};

const SAVE_DELAY: Duration = Duration::from_millis(400);
const COPY_KEEP: Duration = Duration::from_secs(30);

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

/// Opens the window for `dto`.
pub fn open(st: &St, dto: NoteDto) {
    let id = dto.id.clone();
    let title = if dto.title.is_empty() { "Untitled note".to_owned() } else { dto.title.clone() };
    let spec = WindowSpec::new(title)
        .size(dto.width as f32, dto.height as f32)
        .min_size(NOTE_MIN_WIDTH as f32, NOTE_MIN_HEIGHT as f32)
        .position(dto.x as f32, dto.y as f32)
        .frameless()
        .always_on_top(dto.always_on_top)
        .capture_protected(true);

    let pending: Rc<RefCell<Pending>> = Rc::default();
    let saved = Signal::new(true);
    let color = Signal::new(dto.color.clone());
    let pinned = Signal::new(dto.always_on_top);
    let debounce = Debounce::default();

    // Writes the pending edits; the manager's list follows title and color changes.
    let flush: Rc<dyn Fn()> = {
        let (st, pending, id, debounce) = (st.clone(), pending.clone(), id.clone(), debounce.clone());
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
            if checked(&st, r).is_some() {
                saved.set_if_changed(true);
                if list_changed {
                    state::reload_notes(&st);
                }
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
        let note_style = move || {
            let c = color.get();
            StylePatch { background: Some(fill(&c)), foreground: Some(ink(&c)), ..StylePatch::default() }
        };
        let field_style = move || {
            let c = color.get();
            StylePatch {
                background: Some(Color::TRANSPARENT),
                foreground: Some(ink(&c)),
                border_width: Some(0.0),
                ..StylePatch::default()
            }
        };
        let tool_style = move || {
            let c = color.get();
            StylePatch { background: Some(Color::TRANSPARENT), foreground: Some(ink(&c)), ..StylePatch::default() }
        };
        let tool = |glyph: Icon, label: &str, f: Box<dyn Fn()>| {
            w::icon_button(ui, glyph, label, f).variant("quiet").style_dyn(tool_style)
        };

        // Title bar: drag anywhere that is not a control.
        let colors: Vec<MenuItem> = COLORS
            .iter()
            .map(|(c, name)| {
                let (c, pending, schedule) = (c.clone(), pending2.clone(), schedule2.clone());
                MenuItem::new(*name, move || {
                    color.set(c.clone());
                    pending.borrow_mut().color = Some(c.clone());
                    schedule();
                })
            })
            .collect();
        let dot = w::canvas(ui, move |cx| {
            let r = 4.0;
            let center = Point::new(cx.size.width / 2.0, cx.size.height / 2.0);
            let c = if saved.get() { ink(&color.get()).with_alpha(0.35) } else { ink(&color.get()) };
            cx.list.fill_rounded(
                remedy_ui::core::Rect::new(center.x - r, center.y - r, center.x + r, center.y + r),
                remedy_ui::core::CornerRadii::all(r),
                remedy_ui_paint::Brush::Solid(c),
            );
        })
        .width(16.0)
        .height(16.0)
        .semantics_dyn(move || {
            remedy_ui_a11y::Semantics::new(remedy_ui_a11y::Role::Status)
                .label(if saved.get() { "Saved" } else { "Saving" })
        });
        let pin = {
            let (win, pending, schedule) = (win.clone(), pending2.clone(), schedule2.clone());
            tool(
                Icon::Pin,
                "Keep on top",
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
        let pin = pin.bind(move || pin2.state(States::CHECKED, pinned.get()));
        let (s1, s2, s3) = (st.clone(), st.clone(), st.clone());
        let copy_body = Rc::new(RefCell::new(None::<remedy_ui::TextField>));
        let cb = copy_body.clone();
        let app = st.app.clone();
        let (closing_win, closing_flush) = (win.clone(), flush2.clone());
        let bar = w::row(ui)
            .height(36.0)
            .fill_width()
            .gap(unit)
            .padding(Insets::symmetric(unit, unit * 2.0))
            .align(Align::Center)
            .child(w::menu_button(ui, "Color", colors).variant("quiet").style_dyn(tool_style))
            .child(dot)
            .child(w::spacer(ui))
            .child(tool(Icon::Plus, "New note", Box::new(move || state::new_note(&s1, color.get()))))
            .child(tool(Icon::Note, "Open manager", Box::new(move || state::show_manager(&s2))))
            .child(tool(Icon::Lock, "Lock vault", Box::new(move || state::lock_all(&s3))))
            .child(tool(
                Icon::Copy,
                "Copy note (clears in 30 s)",
                Box::new(move || {
                    if let Some(f) = cb.borrow().as_ref() {
                        app.copy_secret(f.value(), COPY_KEEP);
                    }
                }),
            ))
            .child(pin)
            .child(tool(
                Icon::Close,
                "Close note",
                Box::new(move || {
                    closing_flush();
                    closing_win.close();
                }),
            ));
        let mover = win.clone();
        let bar = bar.drag_region(move || mover.begin_move());

        let title = w::text_input(ui, "Title").placeholder("Title");
        title.set_value(&title_text);
        let body = w::text_area(ui, "Note").placeholder("Write something secret…");
        body.set_value(&body_text);
        *copy_body.borrow_mut() = Some(body.clone());
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
        let title_el = title.el().style_dyn(field_style).fill_width().style(StylePatch {
            text_size: Some(ui.theme().type_scale.body + 2.0),
            weight: Some(600.0),
            ..StylePatch::default()
        });
        let body_el = body.el().style_dyn(field_style).fill_width().grow(1.0);

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
        .on_pointer(move |e| {
            if e.phase == PointerPhase::Down && e.buttons.contains(Buttons::PRIMARY) {
                sizer.begin_resize(ResizeEdge::BottomRight);
                return true;
            }
            false
        });
        let footer = w::row(ui).fill_width().child(w::spacer(ui)).child(grip);

        let content = w::column(ui)
            .grow(1.0)
            .fill_width()
            .padding(Insets::symmetric(0.0, unit * 3.0))
            .child(title_el)
            .child(body_el);
        ui.root().child(w::column(ui).grow(1.0).fill_width().style_dyn(note_style).child(bar).child(content).child(footer));
        body.el().focus();
    });

    // Moves and resizes are saved too, after they settle.
    {
        let (pending, schedule) = (pending.clone(), schedule.clone());
        window.on_geometry(move |p: Point, s: Size| {
            pending.borrow_mut().geometry = Some((p.x as f64, p.y as f64, s.width as f64, s.height as f64));
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
