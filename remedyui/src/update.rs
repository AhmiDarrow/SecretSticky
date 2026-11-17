//! Signed updates from GitHub releases (the same minisign key the Tauri app uses).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use remedy_ui::reactive::Signal;
use remedy_ui::{El, Ui};
use remedy_ui_services_update::{self as up, Apply, Feed, Release, UpdateConfig, UpdateState};
use remedy_ui_widgets_core as w;

use crate::state::St;

/// The release signing key (minisign public key line).
const PUBLIC_KEY: &str = "RWRCCbWCyL1aX4p64dsFYQVfrMxfQON25xh0UGvI5Nze/WsVgz9w1iW2";

fn config() -> UpdateConfig {
    UpdateConfig::new(
        env!("CARGO_PKG_VERSION"),
        Feed::GitHub { owner: "AhmiDarrow".into(), repo: "SecretSticky".into(), asset: "_x64-setup.exe".into() },
        PUBLIC_KEY,
    )
}

/// Where an update stands, across manager rebuilds.
pub struct Updater {
    state: Signal<UpdateState>,
    release: RefCell<Option<Release>>,
    file: RefCell<Option<PathBuf>>,
}

impl Updater {
    pub fn new() -> Self {
        Updater { state: Signal::new(UpdateState::Idle), release: RefCell::new(None), file: RefCell::new(None) }
    }
}

fn check(st: &St, up_: &Rc<Updater>) {
    if matches!(up_.state.get_untracked().ok(), Some(UpdateState::Checking | UpdateState::Downloading(_))) {
        return;
    }
    up_.state.set(UpdateState::Checking);
    let u = up_.clone();
    st.app.background(
        || up::check(&config()),
        move |r| match r {
            Ok(Some(rel)) => {
                *u.release.borrow_mut() = Some(rel.clone());
                u.state.set(UpdateState::Available(rel));
            }
            Ok(None) => u.state.set(UpdateState::UpToDate),
            Err(e) => u.state.set(UpdateState::Failed(e.to_string())),
        },
    );
}

fn install(st: &St, up_: &Rc<Updater>) {
    let Some(rel) = up_.release.borrow().clone() else { return };
    up_.state.set(UpdateState::Downloading(None));
    let u = up_.clone();
    let dir = std::env::temp_dir().join("SecretSticky-update");
    st.app.background(
        move || up::download(&config(), &rel, &dir, |_, _| {}),
        move |r| match r {
            Ok(file) => {
                *u.file.borrow_mut() = Some(file);
                u.state.set(UpdateState::Ready);
            }
            Err(e) => u.state.set(UpdateState::Failed(e.to_string())),
        },
    );
}

fn restart(st: &St, up_: &Rc<Updater>) {
    let Some(file) = up_.file.borrow().clone() else { return };
    // Save and lock first: the installer replaces this program.
    crate::state::lock_all(st);
    match up::apply(&file, &Apply::RunInstaller(Vec::new())) {
        Ok(()) => st.app.quit(),
        Err(e) => up_.state.set(UpdateState::Failed(e.to_string())),
    }
}

/// The About and updates section of the manager.
pub fn section(st: &St, ui: &Ui, up_: &Rc<Updater>) -> El {
    let unit = ui.theme().spacing.unit;
    let (s1, u1, s2, u2, s3, u3) = (st.clone(), up_.clone(), st.clone(), up_.clone(), st.clone(), up_.clone());
    let banner = up::update_banner(ui, up_.state, move || install(&s1, &u1), move || restart(&s2, &u2));
    w::section(ui, "About").child(
        w::column(ui)
            .gap(unit * 2.0)
            .fill_width()
            .child(w::label(ui, format!("SecretSticky {} · built with RemedyUI", env!("CARGO_PKG_VERSION")), "muted"))
            .child(w::button(ui, "Check for updates", move || check(&s3, &u3)))
            .child(banner),
    )
}
