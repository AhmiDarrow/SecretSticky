use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::error::{AppError, AppResult};
use crate::vault::{
    sanitize_size, NoteColor, NoteDto, NotePreviewDto, VaultState, VaultStatus,
    NOTE_DEFAULT_HEIGHT, NOTE_MAX_SIZE, NOTE_MIN_HEIGHT, NOTE_MIN_WIDTH,
};

/// Note window labels are `note-{uuid}`. Extract the uuid for ACL checks.
pub(crate) fn note_id_from_label(label: &str) -> Option<&str> {
    label.strip_prefix("note-")
}

/// Pure ACL: which window labels may read/update a given note id.
/// Manager (`main`) may update metadata via notes_update but must not
/// receive full note bodies (see `notes_get` — note windows only).
pub(crate) fn note_access_allowed(window_label: &str, note_id: &str) -> bool {
    if window_label == "main" {
        return true;
    }
    match note_id_from_label(window_label) {
        Some(id) => id == note_id,
        None => false,
    }
}

/// Full body read is restricted to the matching sticky window only.
pub(crate) fn note_body_access_allowed(window_label: &str, note_id: &str) -> bool {
    match note_id_from_label(window_label) {
        Some(id) => id == note_id,
        None => false,
    }
}

/// Pure ACL: manager label or tray/backend (no window) may run admin actions.
pub(crate) fn manager_or_tray_allowed(window_label: Option<&str>) -> bool {
    match window_label {
        None => true,
        Some("main") => true,
        Some(_) => false,
    }
}

/// Manager or any sticky (`note-*`) may read vault status / show unlock UI.
/// Status has no secrets; stickies need `unlocked` on boot and Unlock → manager.
pub(crate) fn main_or_note_allowed(window_label: &str) -> bool {
    window_label == "main" || window_label.starts_with("note-")
}

fn ensure_note_window_acl(window: &tauri::WebviewWindow, note_id: &str) -> AppResult<()> {
    if note_access_allowed(window.label(), note_id) {
        Ok(())
    } else if note_id_from_label(window.label()).is_some() {
        Err(AppError::Message(
            "note window cannot access another note".into(),
        ))
    } else {
        Err(AppError::Message("unauthorized window".into()))
    }
}

/// Manager / tray may create notes; note windows may not create siblings.
fn ensure_manager_or_tray(window: Option<&tauri::WebviewWindow>) -> AppResult<()> {
    if manager_or_tray_allowed(window.map(|w| w.label())) {
        Ok(())
    } else {
        Err(AppError::Message(
            "only the manager can create or delete notes".into(),
        ))
    }
}

/// Admin / vault-wide actions: manager window or tray/backend only.
fn ensure_manager_only(window: Option<&tauri::WebviewWindow>) -> AppResult<()> {
    if manager_or_tray_allowed(window.map(|w| w.label())) {
        Ok(())
    } else {
        Err(AppError::Message(
            "only the manager window may perform this action".into(),
        ))
    }
}

/// Boot / unlock path: manager + sticky windows only.
fn ensure_main_or_note(window: &tauri::WebviewWindow) -> AppResult<()> {
    if main_or_note_allowed(window.label()) {
        Ok(())
    } else {
        Err(AppError::Message("unauthorized window".into()))
    }
}

#[tauri::command]
pub fn vault_status(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
) -> AppResult<VaultStatus> {
    // Stickies call this on boot to know if the vault is unlocked.
    // No secrets in VaultStatus — still deny unknown window labels.
    ensure_main_or_note(&window)?;
    let v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    Ok(v.status())
}

#[tauri::command]
pub fn vault_setup(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    password: String,
) -> AppResult<String> {
    ensure_manager_only(Some(&window))?;
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    v.setup(&password)
}

#[tauri::command]
pub fn vault_unlock(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    password: String,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    {
        let mut throttle = state
            .unlock_throttle
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        throttle.check()?;
    }
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    match v.unlock(&password) {
        Ok(()) => {
            let mut throttle = state
                .unlock_throttle
                .lock()
                .map_err(|e| AppError::Message(e.to_string()))?;
            throttle.record_success();
            Ok(())
        }
        Err(e) => {
            let mut throttle = state
                .unlock_throttle
                .lock()
                .map_err(|e| AppError::Message(e.to_string()))?;
            let count = throttle.record_failure();
            eprintln!(
                "[throttle] vault_unlock failed ({} failures in window)",
                count
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub fn vault_unlock_recovery(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    recovery_key: String,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    {
        let mut throttle = state
            .unlock_throttle
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        throttle.check()?;
    }
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    match v.unlock_with_recovery(&recovery_key) {
        Ok(()) => {
            let mut throttle = state
                .unlock_throttle
                .lock()
                .map_err(|e| AppError::Message(e.to_string()))?;
            throttle.record_success();
            Ok(())
        }
        Err(e) => {
            let mut throttle = state
                .unlock_throttle
                .lock()
                .map_err(|e| AppError::Message(e.to_string()))?;
            let count = throttle.record_failure();
            eprintln!(
                "[throttle] vault_unlock_recovery failed ({} failures in window)",
                count
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub fn vault_lock(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    vault_lock_inner(&app, &state)
}

/// Tray / internal lock (no webview window context).
pub fn vault_lock_from_tray(app: &AppHandle, state: &VaultState) -> AppResult<()> {
    vault_lock_inner(app, state)
}

fn vault_lock_inner(app: &AppHandle, state: &VaultState) -> AppResult<()> {
    // Give note windows a beat to flush debounced saves before we tear them down.
    std::thread::sleep(Duration::from_millis(120));
    {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.lock();
    }
    close_all_note_windows(app);
    let _ = app.emit("vault-locked", ());
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn vault_touch(window: tauri::WebviewWindow, state: State<'_, VaultState>) -> AppResult<()> {
    // Only manager + note-* windows may refresh idle activity.
    let label = window.label();
    if label != "main" && !label.starts_with("note-") {
        return Err(AppError::Message("unauthorized window".into()));
    }
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    v.touch();
    Ok(())
}

#[tauri::command]
pub fn vault_check_idle(app: AppHandle, state: State<'_, VaultState>) -> AppResult<bool> {
    let locked = {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.check_idle_lock()
    };
    if locked {
        close_all_note_windows(&app);
        let _ = app.emit("vault-locked", ());
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.show();
            let _ = main.set_focus();
        }
    }
    Ok(locked)
}

/// Manager list — titles only (no body plaintext over IPC).
#[tauri::command]
pub fn notes_list(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
) -> AppResult<Vec<NotePreviewDto>> {
    ensure_manager_only(Some(&window))?;
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    v.list_note_previews()
}

#[tauri::command]
pub fn notes_get(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    id: String,
) -> AppResult<NoteDto> {
    // Bodies only for the matching sticky — not the manager (preview list only).
    if !note_body_access_allowed(window.label(), &id) {
        return Err(AppError::Message(
            "note body only available to its sticky window".into(),
        ));
    }
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    v.get_note(&id)
}

#[tauri::command]
pub fn notes_create(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    color: Option<String>,
) -> AppResult<NotePreviewDto> {
    ensure_manager_or_tray(Some(&window))?;
    let color = color.and_then(|c| parse_color(&c));
    let note = {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        if !v.status().unlocked {
            return Err(AppError::Locked);
        }
        v.create_note(color)?
    };
    let preview = NotePreviewDto {
        id: note.id.clone(),
        title: note.title.clone(),
        color: note.color.clone(),
        color_css: note.color_css.clone(),
        color_text_css: note.color_text_css.clone(),
        x: note.x,
        y: note.y,
        width: note.width,
        height: note.height,
        always_on_top: note.always_on_top,
        created_at: note.created_at,
        updated_at: note.updated_at,
    };
    let _ = app.emit("notes-changed", ());
    schedule_open_note_window(app, note);
    Ok(preview)
}

/// Tray / internal create without a webview window context.
pub fn notes_create_from_tray(app: AppHandle, state: &VaultState) -> AppResult<NoteDto> {
    let note = {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        if !v.status().unlocked {
            return Err(AppError::Locked);
        }
        v.create_note(None)?
    };
    let _ = app.emit("notes-changed", ());
    schedule_open_note_window(app, note.clone());
    Ok(note)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn notes_update(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    id: String,
    title: Option<String>,
    body: Option<String>,
    color: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    always_on_top: Option<bool>,
) -> AppResult<NoteDto> {
    ensure_note_window_acl(&window, &id)?;
    let color = color.and_then(|c| parse_color(&c));
    let note = {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.update_note(
            &id,
            title,
            body,
            color.clone(),
            x,
            y,
            width,
            height,
            always_on_top,
        )?
    };
    let label = format!("note-{}", id);
    if let Some(w) = app.get_webview_window(&label) {
        if let Some(top) = always_on_top {
            let _ = w.set_always_on_top(top);
        }
        if color.is_some() {
            let _ = w.set_background_color(Some(color_to_rgba(&note.color)));
        }
        let title = if note.title.trim().is_empty() {
            "Sticky note".to_string()
        } else {
            note.title.clone()
        };
        let _ = w.set_title(&title);
    }
    let _ = app.emit("notes-changed", ());
    let _ = app.emit(&format!("note-updated-{}", id), &note);
    Ok(note)
}

#[tauri::command]
pub fn notes_delete(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    id: String,
) -> AppResult<()> {
    // Manager may delete any note; a sticky may only delete itself.
    let label = window.label().to_string();
    if label != "main" {
        ensure_note_window_acl(&window, &id)?;
    }
    {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.delete_note(&id)?;
    }
    let label = format!("note-{}", id);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.hide();
        let _ = w.destroy();
    }
    let _ = app.emit("notes-changed", ());
    Ok(())
}

#[tauri::command]
pub fn notes_open_window(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    id: String,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    let unlocked = {
        let v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.status().unlocked
    };
    if !unlocked {
        let _ = show_main(app);
        return Err(AppError::Locked);
    }
    let note = {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.get_note(&id)?
    };
    let label = format!("note-{}", note.id);
    if app.get_webview_window(&label).is_some() {
        open_note_window(&app, &note)?;
    } else {
        schedule_open_note_window(app, note);
    }
    Ok(())
}

/// Tray / internal open-all without a webview window context.
pub fn notes_open_all_from_tray(app: AppHandle, state: &VaultState) -> AppResult<()> {
    notes_open_all_inner(app, state)
}

#[tauri::command]
pub fn notes_open_all(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    notes_open_all_inner(app, &state)
}

fn notes_open_all_inner(app: AppHandle, state: &VaultState) -> AppResult<()> {
    let unlocked = {
        let v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.status().unlocked
    };
    if !unlocked {
        let _ = show_main(app);
        return Err(AppError::Locked);
    }
    let notes = {
        let mut v = state
            .vault
            .lock()
            .map_err(|e| AppError::Message(e.to_string()))?;
        v.list_notes()?
    };
    for (i, n) in notes.into_iter().enumerate() {
        let app2 = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40 + (i as u64) * 80));
            let note2 = n.clone();
            let app3 = app2.clone();
            let _ = app2.run_on_main_thread(move || {
                if let Err(e) = open_note_window(&app3, &note2) {
                    eprintln!("open_note_window failed: {e}");
                }
            });
        });
    }
    Ok(())
}

#[tauri::command]
pub fn set_idle_lock_secs(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    secs: u64,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    v.set_idle_lock_secs(secs)
}

#[tauri::command]
pub fn change_password(
    window: tauri::WebviewWindow,
    state: State<'_, VaultState>,
    current: String,
    new_password: String,
) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    let mut v = state
        .vault
        .lock()
        .map_err(|e| AppError::Message(e.to_string()))?;
    v.change_password(&current, &new_password)
}

/// Internal show (tray / backend). Prefer the gated command from webviews.
pub fn show_main(app: AppHandle) -> AppResult<()> {
    if let Some(main) = app.get_webview_window("main") {
        if let Some(icon) = app.default_window_icon() {
            let _ = main.set_icon(icon.clone());
        }
        let _ = main.set_title("SecretSticky");
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    Ok(())
}

#[tauri::command(rename = "show_main")]
pub fn show_main_cmd(window: tauri::WebviewWindow, app: AppHandle) -> AppResult<()> {
    // Stickies use this for "Unlock" when the vault is locked.
    ensure_main_or_note(&window)?;
    show_main(app)
}

/// Internal hide (tray / backend).
pub fn hide_main(app: AppHandle) -> AppResult<()> {
    hide_main_window(&app);
    Ok(())
}

#[tauri::command(rename = "hide_main")]
pub fn hide_main_cmd(window: tauri::WebviewWindow, app: AppHandle) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    hide_main(app)
}

/// Internal quit (tray / backend).
pub fn quit_app(app: AppHandle) -> AppResult<()> {
    crate::request_quit(&app);
    Ok(())
}

#[tauri::command(rename = "quit_app")]
pub fn quit_app_cmd(window: tauri::WebviewWindow, app: AppHandle) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    quit_app(app)
}

/// Allowlisted external links only (About → GitHub). No free-form URLs from the webview.
const ALLOWED_EXTERNAL_URLS: &[&str] = &[
    "https://github.com/AhmiDarrow",
    "https://github.com/AhmiDarrow/SecretSticky",
    "https://github.com/AhmiDarrow/SecretSticky/releases",
    "https://github.com/AhmiDarrow/SecretSticky/issues",
];

#[tauri::command]
pub fn open_external_url(window: tauri::WebviewWindow, url: String) -> AppResult<()> {
    ensure_manager_only(Some(&window))?;
    open_external_url_inner(&url)
}

/// Allowlist check used by the command and unit tests (no window context).
fn open_external_url_inner(url: &str) -> AppResult<()> {
    let trimmed = url.trim();
    if !ALLOWED_EXTERNAL_URLS.contains(&trimmed) {
        return Err(AppError::Message("url not allowed".into()));
    }
    open_url_in_browser(trimmed)
}

fn open_url_in_browser(url: &str) -> AppResult<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW — avoid a flashing console when launching the browser.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| AppError::Message(format!("open url: {e}")))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err(AppError::Message(
            "open url unsupported on this platform".into(),
        ))
    }
}

pub fn hide_main_window(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
}

pub fn close_all_note_windows(app: &AppHandle) {
    let windows: Vec<String> = app
        .webview_windows()
        .keys()
        .filter(|l| l.starts_with("note-"))
        .cloned()
        .collect();
    for label in windows {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.hide();
            let _ = w.destroy();
        }
    }
}

pub(crate) fn parse_color(s: &str) -> Option<NoteColor> {
    match s.to_lowercase().as_str() {
        "yellow" => Some(NoteColor::Yellow),
        "green" => Some(NoteColor::Green),
        "pink" => Some(NoteColor::Pink),
        "blue" => Some(NoteColor::Blue),
        "purple" => Some(NoteColor::Purple),
        "gray" | "grey" => Some(NoteColor::Gray),
        "black" => Some(NoteColor::Black),
        "darkgreen" | "dark_green" | "dark-green" => Some(NoteColor::DarkGreen),
        _ => None,
    }
}

fn color_to_rgba(c: &NoteColor) -> tauri::window::Color {
    // Keep RGB in sync with NoteColor::as_css / src/types.ts COLORS
    let (r, g, b) = match c {
        NoteColor::Yellow => (255, 229, 102),
        NoteColor::Green => (184, 224, 138),
        NoteColor::Pink => (245, 168, 192),
        NoteColor::Blue => (126, 196, 245),
        NoteColor::Purple => (199, 155, 224),
        NoteColor::Gray => (212, 212, 216),
        NoteColor::Black => (18, 18, 18),
        NoteColor::DarkGreen => (22, 61, 44),
    };
    tauri::window::Color(r, g, b, 255)
}

fn schedule_open_note_window(app: AppHandle, note: NoteDto) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let app2 = app.clone();
        let note2 = note.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            if let Err(err) = open_note_window(&app2, &note2) {
                eprintln!("open_note_window failed: {err}");
            }
        }) {
            eprintln!("run_on_main_thread failed: {e}");
        }
    });
}

fn open_note_window(app: &AppHandle, note: &NoteDto) -> AppResult<()> {
    let label = format!("note-{}", note.id);
    if let Some(existing) = app.get_webview_window(&label) {
        // Keep manager visible until the sticky is focused. Hiding main first
        // lets Windows activate the parent `tauri dev` console (black cmd box).
        let _ = existing.show();
        let _ = existing.unminimize();
        let _ = existing.set_always_on_top(note.always_on_top);
        let _ = existing.set_focus();
        hide_main_window(app);
        let _ = existing.set_focus();
        refocus_note_soon(app, &label);
        return Ok(());
    }

    let url = WebviewUrl::App("index.html".into());
    // Use saved geometry as-is. Cascade offset only for brand-new defaults
    // (create_note starts at 120,120) so reopen does not walk down-right.
    let is_default_origin = (note.x - 120.0).abs() < 0.5 && (note.y - 120.0).abs() < 0.5;
    let (x, y) = if is_default_origin {
        let offset =
            (note.id.bytes().fold(0u32, |a, b| a.wrapping_add(b as u32)) % 8) as f64 * 28.0;
        (note.x + offset, note.y + offset)
    } else {
        (note.x, note.y)
    };
    // Whole-pixel min from live sticky measure (~343×247) → 345×250.
    let width = sanitize_size(note.width, NOTE_MIN_WIDTH, NOTE_MAX_SIZE, NOTE_MIN_WIDTH);
    let height = sanitize_size(
        note.height,
        NOTE_MIN_HEIGHT,
        NOTE_MAX_SIZE,
        NOTE_DEFAULT_HEIGHT,
    );

    let id_js = note.id.replace('\\', "\\\\").replace('\'', "\\'");
    let (bg_r, bg_g, bg_b) = match &note.color {
        NoteColor::Yellow => (255, 229, 102),
        NoteColor::Green => (184, 224, 138),
        NoteColor::Pink => (245, 168, 192),
        NoteColor::Blue => (126, 196, 245),
        NoteColor::Purple => (199, 155, 224),
        NoteColor::Gray => (212, 212, 216),
        NoteColor::Black => (18, 18, 18),
        NoteColor::DarkGreen => (22, 61, 44),
    };
    let init_script = format!(
        r#"(function(){{
  try {{
    Object.defineProperty(window, '__SECRETSTICKY_NOTE_ID__', {{
      value: '{id}',
      writable: false,
      configurable: false
    }});
    document.documentElement.dataset.mode = 'note';
    document.documentElement.style.background = 'rgb({r},{g},{b})';
    if (document.body) {{
      document.body.classList.add('is-note-window');
      document.body.style.background = 'rgb({r},{g},{b})';
    }} else {{
      document.addEventListener('DOMContentLoaded', function(){{
        document.body.classList.add('is-note-window');
        document.body.style.background = 'rgb({r},{g},{b})';
      }});
    }}
  }} catch (e) {{}}
}})();"#,
        id = id_js,
        r = bg_r,
        g = bg_g,
        b = bg_b
    );

    let mut builder = WebviewWindowBuilder::new(app, &label, url)
        .title(if note.title.trim().is_empty() {
            "Sticky note".into()
        } else {
            note.title.clone()
        })
        .inner_size(width, height)
        .min_inner_size(NOTE_MIN_WIDTH, NOTE_MIN_HEIGHT)
        .position(x, y)
        .resizable(true)
        .decorations(false)
        .transparent(false)
        .always_on_top(note.always_on_top)
        .skip_taskbar(false)
        .focused(true)
        .visible(true)
        .initialization_script(&init_script)
        .background_color(color_to_rgba(&note.color));

    // Force brand icon on every sticky (taskbar / Alt-Tab) — do not rely on
    // process default alone; some WebView2 paths still showed the old mark.
    if let Some(icon) = app.default_window_icon() {
        builder = builder
            .icon(icon.clone())
            .map_err(|e| AppError::Message(format!("window icon: {e}")))?;
    }

    let window = builder
        .build()
        .map_err(|e| AppError::Message(format!("window: {e}")))?;

    if let Some(icon) = app.default_window_icon() {
        let _ = window.set_icon(icon.clone());
    }
    let _ = window.set_background_color(Some(color_to_rgba(&note.color)));
    let _ = window.show();
    let _ = window.set_focus();
    // Hide manager only after the sticky owns focus, then re-assert focus so
    // the parent console cannot steal activation.
    hide_main_window(app);
    let _ = window.set_focus();
    refocus_note_soon(app, &label);
    Ok(())
}

/// After manager hide, Windows may briefly activate the parent terminal
/// (`tauri dev` / cmd). Push that console behind and re-assert sticky focus.
fn refocus_note_soon(app: &AppHandle, label: &str) {
    #[cfg(windows)]
    demote_parent_console();

    let app = app.clone();
    let label = label.to_string();
    std::thread::spawn(move || {
        for delay_ms in [30_u64, 120, 280] {
            std::thread::sleep(Duration::from_millis(delay_ms));
            #[cfg(windows)]
            demote_parent_console();
            let app2 = app.clone();
            let label2 = label.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(w) = app2.get_webview_window(&label2) {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            });
        }
    });
}

/// If a parent console (cmd / PowerShell hosting `tauri dev`) is the foreground
/// window after a sticky open, push it behind without killing the session.
#[cfg(windows)]
fn demote_parent_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dw_process_id: u32) -> i32;
        fn FreeConsole() -> i32;
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut core::ffi::c_void;
        fn SetWindowPos(
            hwnd: *mut core::ffi::c_void,
            hwnd_insert_after: *mut core::ffi::c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    // HWND_BOTTOM
    const HWND_BOTTOM: isize = 1;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOACTIVATE: u32 = 0x0010;

    unsafe {
        // Temporarily attach only to discover the parent console HWND.
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        let hwnd = GetConsoleWindow();
        let fg = GetForegroundWindow();
        // Only demote when that console actually stole foreground focus.
        if !hwnd.is_null() && hwnd == fg {
            let _ = SetWindowPos(
                hwnd,
                HWND_BOTTOM as *mut core::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let _ = FreeConsole();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_id_from_label_parses() {
        assert_eq!(
            note_id_from_label("note-550e8400-e29b-41d4-a716-446655440000"),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(note_id_from_label("main"), None);
        assert_eq!(note_id_from_label("note-"), Some(""));
        assert_eq!(note_id_from_label("other"), None);
    }

    #[test]
    fn note_acl_main_and_owner_ok_cross_note_denied() {
        let id = "abc-123";
        assert!(note_access_allowed("main", id));
        assert!(note_access_allowed(&format!("note-{id}"), id));
        assert!(!note_access_allowed("note-other-id", id));
        assert!(!note_access_allowed("random", id));
    }

    #[test]
    fn note_body_acl_owner_only_not_main() {
        let id = "abc-123";
        assert!(!note_body_access_allowed("main", id));
        assert!(note_body_access_allowed(&format!("note-{id}"), id));
        assert!(!note_body_access_allowed("note-other-id", id));
        assert!(!note_body_access_allowed("random", id));
    }

    #[test]
    fn manager_or_tray_acl() {
        assert!(manager_or_tray_allowed(None));
        assert!(manager_or_tray_allowed(Some("main")));
        assert!(!manager_or_tray_allowed(Some("note-xyz")));
        assert!(!manager_or_tray_allowed(Some("other")));
    }

    #[test]
    fn main_or_note_acl_allows_manager_and_stickies() {
        assert!(main_or_note_allowed("main"));
        assert!(main_or_note_allowed("note-abc-123"));
        assert!(main_or_note_allowed("note-"));
        assert!(!main_or_note_allowed("other"));
        assert!(!main_or_note_allowed("tray"));
        assert!(!main_or_note_allowed(""));
    }

    #[test]
    fn external_url_allowlist_rejects_unknown() {
        let bad = [
            "https://evil.example",
            "https://github.com/AhmiDarrow/SecretSticky/../../evil",
            "javascript:alert(1)",
            "file:///C:/Windows/System32",
            "https://github.com/AhmiDarrow/SecretSticky/wiki",
            "",
            "  ",
        ];
        for url in bad {
            let err = open_external_url_inner(url).unwrap_err().to_string();
            assert!(
                err.contains("not allowed"),
                "expected reject for {url:?}, got {err}"
            );
        }
    }

    #[test]
    fn external_url_allowlist_accepts_known() {
        // We only assert allowlist membership here — spawning a browser is side-effectful.
        for url in ALLOWED_EXTERNAL_URLS {
            assert!(
                ALLOWED_EXTERNAL_URLS.contains(url),
                "allowlist must include {url}"
            );
            let trimmed = (*url).trim();
            assert!(ALLOWED_EXTERNAL_URLS.contains(&trimmed));
        }
        // Exact match required (no trailing slash drift)
        assert!(!ALLOWED_EXTERNAL_URLS.contains(&"https://github.com/AhmiDarrow/"));
    }

    #[test]
    fn note_min_size_constants_sane() {
        // Shared with vault geometry clamp — single source of truth
        assert_eq!(NOTE_MIN_WIDTH, crate::vault::NOTE_MIN_WIDTH);
        assert_eq!(NOTE_MIN_HEIGHT, crate::vault::NOTE_MIN_HEIGHT);
        assert_eq!(NOTE_MAX_SIZE, crate::vault::NOTE_MAX_SIZE);
        // Sanity bounds as runtime comparisons (avoid clippy::assertions_on_constants)
        let min_w = NOTE_MIN_WIDTH;
        let min_h = NOTE_MIN_HEIGHT;
        let max = NOTE_MAX_SIZE;
        assert!(min_w >= 200.0 && min_h >= 150.0 && max > min_w && max > min_h);
    }

    #[test]
    fn parse_color_accepts_aliases_and_rejects_unknown() {
        assert_eq!(parse_color("yellow"), Some(NoteColor::Yellow));
        assert_eq!(parse_color("YELLOW"), Some(NoteColor::Yellow));
        assert_eq!(parse_color("gray"), Some(NoteColor::Gray));
        assert_eq!(parse_color("grey"), Some(NoteColor::Gray));
        assert_eq!(parse_color("darkgreen"), Some(NoteColor::DarkGreen));
        assert_eq!(parse_color("dark_green"), Some(NoteColor::DarkGreen));
        assert_eq!(parse_color("dark-green"), Some(NoteColor::DarkGreen));
        assert_eq!(parse_color("blue"), Some(NoteColor::Blue));
        assert_eq!(parse_color("not-a-color"), None);
        assert_eq!(parse_color(""), None);
        assert_eq!(parse_color("red"), None);
    }

    #[test]
    fn color_to_rgba_matches_css_hex() {
        // RGB tuples must match NoteColor::as_css hex digits
        let cases = [
            (NoteColor::Yellow, (255, 229, 102)),
            (NoteColor::Green, (184, 224, 138)),
            (NoteColor::Pink, (245, 168, 192)),
            (NoteColor::Blue, (126, 196, 245)),
            (NoteColor::Purple, (199, 155, 224)),
            (NoteColor::Gray, (212, 212, 216)),
            (NoteColor::Black, (18, 18, 18)),
            (NoteColor::DarkGreen, (22, 61, 44)),
        ];
        for (c, (r, g, b)) in cases {
            let rgba = color_to_rgba(&c);
            assert_eq!(rgba.0, r, "{c:?} R");
            assert_eq!(rgba.1, g, "{c:?} G");
            assert_eq!(rgba.2, b, "{c:?} B");
            assert_eq!(rgba.3, 255);
            // Cross-check against as_css hex
            let hex = c.as_css().trim_start_matches('#');
            let hr = u8::from_str_radix(&hex[0..2], 16).unwrap();
            let hg = u8::from_str_radix(&hex[2..4], 16).unwrap();
            let hb = u8::from_str_radix(&hex[4..6], 16).unwrap();
            assert_eq!((r, g, b), (hr, hg, hb), "{c:?} rgba vs css");
        }
    }
}
