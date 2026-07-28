// Always GUI subsystem on Windows — prevents a black cmd/console window when
// the app starts or when sticky note windows are created. (The default Tauri
// template only sets this for release, so `tauri dev` still flashed a console.)
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    suppress_console_window();

    secretsticky_lib::run()
}

/// Drop any console this process owns. Never AllocConsole — that is the black cmd box.
#[cfg(windows)]
fn suppress_console_window() {
    #[link(name = "kernel32")]
    extern "system" {
        fn FreeConsole() -> i32;
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }

    unsafe {
        if !GetConsoleWindow().is_null() {
            let _ = FreeConsole();
        }
    }
}
