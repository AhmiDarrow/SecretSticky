mod commands;
mod crypto;
mod error;
mod vault;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

use commands::*;
use vault::VaultState;

/// When false, the process stays alive in the tray (manager X / last window hide).
static QUITTING: AtomicBool = AtomicBool::new(false);

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let vault = VaultState::new().expect("failed to open vault store");

    tauri::Builder::default()
        .manage(vault)
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
            show_main,
            hide_main,
            quit_app,
        ])
        .setup(|app| {
            let new_i = MenuItem::with_id(app, "new", "New note", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show manager", true, None::<&str>)?;
            let open_all_i =
                MenuItem::with_id(app, "open_all", "Open all notes", true, None::<&str>)?;
            let lock_i = MenuItem::with_id(app, "lock", "Lock vault", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&new_i, &show_i, &open_all_i, &lock_i, &quit_i])?;

            let icon = app
                .default_window_icon()
                .cloned()
                .unwrap_or_else(|| Image::new_owned(vec![0; 4], 1, 1));

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("SecretSticky — tray (click to show manager)")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "new" => {
                        let unlocked = app
                            .state::<VaultState>()
                            .0
                            .lock()
                            .map(|v| v.status().unlocked)
                            .unwrap_or(false);
                        if unlocked {
                            let _ =
                                notes_create_from_tray(app.clone(), &*app.state::<VaultState>());
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
                            .0
                            .lock()
                            .map(|v| v.status().unlocked)
                            .unwrap_or(false);
                        if unlocked {
                            let _ =
                                notes_open_all_from_tray(app.clone(), &*app.state::<VaultState>());
                        } else {
                            let _ = show_main(app.clone());
                        }
                    }
                    "lock" => {
                        let _ = vault_lock_from_tray(app, &*app.state::<VaultState>());
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

            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(30));
                let state = handle.state::<VaultState>();
                let _ = vault_check_idle(handle.clone(), state);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // Manager X → hide to tray (keep stickies + process alive).
                    // Real exit is tray Quit only.
                    if window.label() == "main" {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                _ => {}
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
