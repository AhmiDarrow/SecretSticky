//! Signed updates from GitHub releases, checked against the same minisign key the Tauri
//! app's updater trusts, in the shipped app's words.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use remedy_ui::reactive::Signal;
use remedy_ui::{El, Ui};
use remedy_ui_services_update::{self as up, Apply, BannerText, Feed, Release, UpdateConfig, UpdateState};
use remedy_ui_widgets_core as w;

use crate::state::St;

/// The release signing key (the minisign public key in `tauri.conf.json`).
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

/// The banner in the shipped app's words; the installer restarts the app.
fn texts() -> BannerText {
    let mut t = BannerText::default();
    t.checking = "Checking…".into();
    t.up_to_date = format!("You're on the latest version ({}).", env!("CARGO_PKG_VERSION"));
    t.available = "Update {version} is ready to install.".into();
    t.downloading = "Downloading update…".into();
    t.ready = "Installing…".into();
    t.install = "Install & restart".into();
    t.restart = None;
    t
}

fn busy(up_: &Updater) -> bool {
    matches!(up_.state.get_untracked().ok(), Some(UpdateState::Checking | UpdateState::Downloading(_) | UpdateState::Ready))
}

fn check(st: &St, up_: &Rc<Updater>) {
    if busy(up_) {
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

/// Downloads (showing how far it got), verifies, then installs and restarts.
fn install(st: &St, up_: &Rc<Updater>) {
    let Some(rel) = up_.release.borrow().clone() else { return };
    up_.state.set(UpdateState::Downloading(None));
    // Received and total bytes, written by the download and read by a timer here.
    let progress = Arc::new((AtomicU64::new(0), AtomicU64::new(0)));
    poll(st, up_, progress.clone());
    let (u, s) = (up_.clone(), st.clone());
    let dir = std::env::temp_dir().join("SecretSticky-update");
    st.app.background(
        move || {
            up::download(&config(), &rel, &dir, |got, total| {
                progress.0.store(got, Ordering::Relaxed);
                progress.1.store(total.unwrap_or(0), Ordering::Relaxed);
            })
        },
        move |r| match r {
            Ok(file) => {
                *u.file.borrow_mut() = Some(file);
                u.state.set(UpdateState::Ready);
                restart(&s, &u);
            }
            Err(e) => u.state.set(UpdateState::Failed(e.to_string())),
        },
    );
}

/// Shows the download's progress while it runs.
fn poll(st: &St, up_: &Rc<Updater>, progress: Arc<(AtomicU64, AtomicU64)>) {
    if !matches!(up_.state.get_untracked().ok(), Some(UpdateState::Downloading(_))) {
        return;
    }
    let (got, total) = (progress.0.load(Ordering::Relaxed), progress.1.load(Ordering::Relaxed));
    let pct = (total > 0).then(|| (got as f32 / total as f32).min(1.0));
    up_.state.set_if_changed(UpdateState::Downloading(pct));
    let (s, u) = (st.clone(), up_.clone());
    st.app.after(Duration::from_millis(150), move || poll(&s, &u, progress));
}

fn restart(st: &St, up_: &Rc<Updater>) {
    let Some(file) = up_.file.borrow().clone() else { return };
    // Save and lock first: the installer replaces this program.
    crate::state::lock_all(st);
    match up::apply(&file, &Apply::RunInstaller(Vec::new())) {
        Ok(()) => crate::state::quit(st),
        Err(e) => up_.state.set(UpdateState::Failed(e.to_string())),
    }
}

/// Check for updates, and the banner that follows it.
pub fn section(st: &St, ui: &Ui, up_: &Rc<Updater>) -> El {
    let unit = ui.theme().spacing.unit;
    let (s1, u1, s2, u2, s3, u3) = (st.clone(), up_.clone(), st.clone(), up_.clone(), st.clone(), up_.clone());
    let banner = up::update_banner_with(ui, up_.state, &texts(), move || install(&s1, &u1), move || restart(&s2, &u2));
    let state = up_.state;
    let check_label = move || {
        if matches!(state.get(), UpdateState::Checking) { "Checking…".to_owned() } else { "Check for updates".to_owned() }
    };
    w::column(ui)
        .gap(unit * 2.0)
        .fill_width()
        .child(
            w::button(ui, check_label, move || check(&s3, &u3)).disabled(move || {
                matches!(state.get(), UpdateState::Checking | UpdateState::Downloading(_) | UpdateState::Ready)
            }),
        )
        .child(banner)
}
