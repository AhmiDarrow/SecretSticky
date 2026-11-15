mod commands;
mod crypto;
mod error;
mod i18n;
mod vault;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener, Manager, RunEvent, WindowEvent,
};

use commands::*;
use vault::VaultState;

/// When false, the process stays alive in the tray (manager X / last window hide).
static QUITTING: AtomicBool = AtomicBool::new(false);

pub struct TrayState {
    pub icon: tauri::tray::TrayIcon,
}

pub fn request_quit(app: &tauri::AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    close_all_note_windows(app);
    // Scrub again shortly in case destroy is async on WebView2.
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        close_all_note_windows(&app2);
        app2.exit(0);
    });
}

fn build_tray_menu(app: &tauri::AppHandle, lang: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let new_i = MenuItem::with_id(app, "new", i18n::t(lang, "tray.new"), true, None::<&str>)?;
    let show_i = MenuItem::with_id(app, "show", i18n::t(lang, "tray.show"), true, None::<&str>)?;
    let open_all_i = MenuItem::with_id(
        app,
        "open_all",
        i18n::t(lang, "tray.openAll"),
        true,
        None::<&str>,
    )?;
    let lock_i = MenuItem::with_id(app, "lock", i18n::t(lang, "tray.lock"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", i18n::t(lang, "tray.quit"), true, None::<&str>)?;
    Menu::with_items(app, &[&new_i, &show_i, &open_all_i, &lock_i, &quit_i])
}

fn apply_tray_strings(app: &tauri::AppHandle, lang: &str) {
    let Ok(menu) = build_tray_menu(app, lang) else {
        return;
    };
    let tooltip = i18n::t(lang, "tray.tooltip");
    if let Some(tray) = app.try_state::<TrayState>() {
        let _ = tray.icon.set_menu(Some(menu));
        let _ = tray.icon.set_tooltip(Some(tooltip));
    }
}

/// Save the language, rebuild the tray, and tell every webview to switch now.
pub fn apply_language(app: &tauri::AppHandle, requested: &str) -> Result<String, String> {
    let state = app.state::<i18n::I18nState>();
    let saved = i18n::set_lang(&state.data_dir, requested)?;
    if let Ok(mut lang) = state.lang.lock() {
        *lang = saved.clone();
    }
    apply_tray_strings(app, &saved);
    let _ = app.emit("ui-lang-changed", serde_json::json!({ "lang": saved }));
    Ok(saved)
}

#[cfg(all(windows, target_pointer_width = "64"))]
#[link(name = "user32")]
extern "system" {
    fn ShowWindow(hwnd: *mut std::ffi::c_void, cmd: i32) -> i32;
    fn SetWindowPos(
        hwnd: *mut std::ffi::c_void,
        insert_after: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn GetWindowLongPtrW(hwnd: *mut std::ffi::c_void, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: *mut std::ffi::c_void, index: i32, new_long: isize) -> isize;
    fn GetSystemMetrics(index: i32) -> i32;
}

/// Normal launches show the manager. CI shows the webview without taking focus.
///
/// `Window::show` only shows the native frame. WebView2 stays `IsVisible=false`
/// until its own `show()`, and the controller is created asynchronously, so a
/// single early show can lose to the creation callback. CI drops the title bar
/// (so the sliver is client area), pins an 8px corner, and repeats `webview.show()`.
fn present_main_window(window: &tauri::WebviewWindow) {
    if !i18n::ci_mode() {
        let _ = window.show();
        return;
    }
    let data_dir = window
        .try_state::<i18n::I18nState>()
        .map(|state| state.data_dir.clone());
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_focusable(false);
    let _ = window.set_decorations(false);
    let (x, y) = ci_sliver_origin();
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = window.set_size(tauri::PhysicalSize::new(320u32, 240u32));
    pin_ci_window(window, true);
    let window_show = window.show();
    let webview: &tauri::Webview = std::convert::AsRef::as_ref(window);
    let web_show = webview.show();
    if let Some(dir) = &data_dir {
        let _ = std::fs::write(
            dir.join("ci-reveal.txt"),
            format!("window_show={window_show:?}\nweb_show={web_show:?}\norigin={x},{y}\n"),
        );
    }
    let pinned = window.clone();
    std::thread::spawn(move || {
        for i in 0..24 {
            std::thread::sleep(Duration::from_millis(250));
            let webview: &tauri::Webview = std::convert::AsRef::as_ref(&pinned);
            let _ = webview.show();
            pin_ci_window(&pinned, false);
            if i == 4 || i == 12 || i == 20 {
                if let Some(dir) = data_dir.clone() {
                    let dir2 = dir.clone();
                    let _ = pinned.eval_with_callback(
                        "(function(){return {ready:document.readyState,lang:document.documentElement.lang,href:String(location.href),title:document.title,text:(document.body&&document.body.innerText||'').slice(0,800)};})()",
                        move |result| {
                            let _ = std::fs::write(dir2.join("ci-i18n-eval.json"), result);
                        },
                    );
                }
            }
        }
    });
}

#[cfg(all(windows, target_pointer_width = "64"))]
fn ci_sliver_origin() -> (i32, i32) {
    const SM_XVIRTUALSCREEN: i32 = 76;
    const SM_YVIRTUALSCREEN: i32 = 77;
    const SM_CXVIRTUALSCREEN: i32 = 78;
    const SM_CYVIRTUALSCREEN: i32 = 79;
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);
        // 8px of undecorated client area stays on the virtual desktop.
        (vx + vw - 8, vy + vh - 8)
    }
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
fn ci_sliver_origin() -> (i32, i32) {
    (0, 0)
}

#[cfg(all(windows, target_pointer_width = "64"))]
fn pin_ci_window(window: &tauri::WebviewWindow, show: bool) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let raw = hwnd.0 as *mut std::ffi::c_void;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x80;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const WS_EX_APPWINDOW: isize = 0x0004_0000;
    const SW_SHOWNOACTIVATE: i32 = 4;
    const SWP_NOACTIVATE: u32 = 0x0010;
    // HWND_BOTTOM keeps the sliver under other top-level windows.
    let hwnd_bottom = 1isize as *mut std::ffi::c_void;
    let (x, y) = ci_sliver_origin();
    unsafe {
        let style = GetWindowLongPtrW(raw, GWL_EXSTYLE);
        let next = (style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) & !WS_EX_APPWINDOW;
        SetWindowLongPtrW(raw, GWL_EXSTYLE, next);
        SetWindowPos(raw, hwnd_bottom, x, y, 320, 240, SWP_NOACTIVATE);
        if show {
            ShowWindow(raw, SW_SHOWNOACTIVATE);
        }
    }
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
fn pin_ci_window(_window: &tauri::WebviewWindow, _show: bool) {}

fn write_rust_probe(data_dir: &std::path::Path, lang: &str) {
    let proof = serde_json::json!({
        "lang": lang,
        "trayNew": i18n::t(lang, "tray.new"),
        "windowTitle": i18n::t(lang, "note.windowTitle"),
        "tooltip": i18n::t(lang, "tray.tooltip"),
        "hello": i18n::t(lang, "about.hello"),
    });
    if let Ok(text) = serde_json::to_string_pretty(&proof) {
        let _ = std::fs::write(data_dir.join("ci-i18n-rust.json"), text);
    }
}

fn arm_ci_probe(app: &tauri::AppHandle, data_dir: PathBuf) {
    let dir = data_dir.clone();
    let app_listen = app.clone();
    let _id = app.listen("ci-i18n-sample", move |event| {
        let _ = std::fs::write(dir.join("ci-i18n-sample.json"), event.payload());
        let _ = std::fs::write(dir.join("ci-i18n-ready"), "ok");
        request_quit(&app_listen);
    });

    let app_timeout = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(25));
        if data_dir.join("ci-i18n-ready").exists() {
            return;
        }
        let _ = std::fs::write(data_dir.join("ci-i18n-failed"), "timeout");
        request_quit(&app_timeout);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let vault = VaultState::new().expect("failed to open vault store");
    let data_dir = vault.vault.lock().expect("vault lock").data_dir();
    let lang = i18n::load_or_init_lang(&data_dir);
    let i18n_state = i18n::I18nState {
        lang: Mutex::new(lang.clone()),
        data_dir: data_dir.clone(),
    };

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());
    // A CI process must not attach to a running SecretSticky and focus it.
    if !i18n::ci_mode() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = show_main(app.clone());
        }));
    }

    builder
        .manage(vault)
        .manage(i18n_state)
        .invoke_handler(tauri::generate_handler![
            vault_status,
            vault_setup,
            vault_unlock,
            vault_unlock_recovery,
            vault_lock,
            vault_touch,
            vault_check_idle,
            notes_list,
            notes_get,
            notes_create,
            notes_update,
            notes_delete,
            notes_open_window,
            notes_open_all,
            set_idle_lock_secs,
            change_password,
            show_main_cmd,
            hide_main_cmd,
            quit_app_cmd,
            open_external_url,
            ui_get_boot,
            ui_set_lang,
        ])
        .setup(move |app| {
            if i18n::ci_mode() {
                write_rust_probe(&data_dir, &lang);
                arm_ci_probe(app.handle(), data_dir);
            }

            let menu = build_tray_menu(app.handle(), &lang)?;

            // Brand icon for tray + any window that missed the PE resource default.
            let icon = app
                .default_window_icon()
                .cloned()
                .unwrap_or_else(|| Image::new_owned(vec![0; 4], 1, 1));

            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_icon(icon.clone());
                let _ = main.set_title("SecretSticky");
            }

            let tray = TrayIconBuilder::new()
                .icon(icon)
                .icon_as_template(false)
                .menu(&menu)
                .tooltip(i18n::t(&lang, "tray.tooltip"))
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "new" => {
                        let unlocked = app
                            .state::<VaultState>()
                            .vault
                            .lock()
                            .map(|v| v.status().unlocked)
                            .unwrap_or(false);
                        if unlocked {
                            let _ = notes_create_from_tray(app.clone(), &app.state::<VaultState>());
                        } else {
                            let _ = show_main(app.clone());
                        }
                    }
                    "show" => {
                        let _ = show_main(app.clone());
                    }
                    "open_all" => {
                        let unlocked = app
                            .state::<VaultState>()
                            .vault
                            .lock()
                            .map(|v| v.status().unlocked)
                            .unwrap_or(false);
                        if unlocked {
                            let _ =
                                notes_open_all_from_tray(app.clone(), &app.state::<VaultState>());
                        } else {
                            let _ = show_main(app.clone());
                        }
                    }
                    "lock" => {
                        // Menu events run on the main thread; lock off it so the
                        // pre-lock grace period lets stickies flush their saves.
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let _ = vault_lock_from_tray(&app, &app.state::<VaultState>());
                        });
                    }
                    "quit" => {
                        request_quit(app);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = show_main(tray.app_handle().clone());
                    }
                })
                .build(app)?;
            app.manage(TrayState { icon: tray });

            if let Some(main) = app.get_webview_window("main") {
                present_main_window(&main);
            }

            let handle = app.handle().clone();
            // Short tick so auto-lock fires within a few seconds of the deadline.
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(5));
                let state = handle.state::<VaultState>();
                let _ = vault_check_idle(handle.clone(), state);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Manager X → hide to tray (keep stickies + process alive).
                // Real exit is tray Quit only.
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building SecretSticky")
        .run(|app_handle, event| match event {
            RunEvent::ExitRequested { api, .. } => {
                // Stay resident in the tray when the user hides the manager or
                // closes the last sticky. Only tray Quit / quit_app sets QUITTING.
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_exit();
                } else {
                    close_all_note_windows(app_handle);
                }
            }
            RunEvent::Exit => {
                close_all_note_windows(app_handle);
            }
            _ => {}
        });
}
