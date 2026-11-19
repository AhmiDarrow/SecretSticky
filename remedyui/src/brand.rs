//! The shipped app's own artwork: the app mark (a yellow note with a padlock) for the
//! manager and gate, and the tray icon, decoded from the same PNGs the Tauri app ships.

use remedy_ui::host::{TrayImage, WindowIcon};
use remedy_ui_paint::Image;

/// `src/assets/app-mark.png` (128 px), shown in the header, the gate and About.
const MARK: &[u8] = include_bytes!("../../src/assets/app-mark.png");
/// `src-tauri/icons/32x32.png`, the tray and window icon of the shipped app.
const ICON_32: &[u8] = include_bytes!("../../src-tauri/icons/32x32.png");
/// `src-tauri/icons/64x64.png`, for trays on high-density screens.
const ICON_64: &[u8] = include_bytes!("../../src-tauri/icons/64x64.png");
/// `src-tauri/icons/128x128@2x.png` (256 px), the largest of the shipped window icons; each
/// platform scales it to the sizes it shows.
const ICON_256: &[u8] = include_bytes!("../../src-tauri/icons/128x128@2x.png");

thread_local! {
    static MARK_IMAGE: Option<Image> = remedy_ui_media_image::decode(MARK, None).ok().map(|d| d.image);
    static WINDOW_ICON: Option<WindowIcon> = remedy_ui_media_image::decode(ICON_256, None)
        .ok()
        .and_then(|d| WindowIcon::new(d.image.width, d.image.pixels.to_vec()).filter(|_| d.image.width == d.image.height));
}

/// The shipped app's icon for every window (taskbar, Alt+Tab), decoded once per thread.
pub fn window_icon() -> Option<WindowIcon> {
    WINDOW_ICON.with(Clone::clone)
}

/// The app mark, premultiplied (decoded once per thread).
pub fn mark() -> Option<Image> {
    MARK_IMAGE.with(Clone::clone)
}

/// The tray icon for a tray drawn at `px` pixels: the 64 px artwork on dense screens.
pub fn tray_icon(px: u32) -> Option<TrayImage> {
    let bytes = if px > 32 { ICON_64 } else { ICON_32 };
    let d = remedy_ui_media_image::decode(bytes, None).ok()?;
    let img = d.image;
    (img.width == img.height).then(|| TrayImage { size: img.width, rgba: img.pixels.to_vec() })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_shipped_artwork_decodes() {
        let mark = super::mark().expect("app mark");
        assert_eq!((mark.width, mark.height), (128, 128));
        let tray = super::tray_icon(32).expect("tray icon");
        assert_eq!(tray.size, 32);
        assert_eq!(tray.rgba.len(), 32 * 32 * 4);
        let px: Vec<&[u8]> = tray.rgba.chunks(4).collect();
        assert!(px.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]), "premultiplied");
        assert!(px.iter().any(|p| p[3] == 255), "has opaque artwork");
        assert_eq!(super::tray_icon(64).map(|t| t.size), Some(64));
        assert_eq!(super::window_icon().map(|i| i.size()), Some(256), "the window icon");
    }
}
