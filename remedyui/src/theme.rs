//! SecretSticky's look as RemedyUI design tokens: the shipped app's dark charcoal with the
//! amber accent (`src/App.css`), on the Remedy type scale, spacing and radii.

use remedy_ui::core::Color;
use remedy_ui::style::{States, StylePatch, StyleSheet, Theme};
use remedy_ui_themes::Palette;

/// The shipped app's colors (`:root` in `src/App.css`).
pub const BRAND: Palette = Palette {
    id: "secretsticky",
    name: "SecretSticky",
    dark: true,
    bg: "#1a1b1e",
    panel: "#24262b",
    card: "#2f3238",
    line: "#3f3f46",
    accent: "#fbbf24",
    text: "#f3f4f6",
    muted: "#9ca3af",
    faint: "#71717a",
    accent_hover: "#f59e0b",
    ok: "#4ade80",
    danger: "#f87171",
    warn: "#fde68a",
};

/// Text on the amber accent: the shipped `--accent-text`.
const ON_ACCENT: &str = "#1a1b1e";

/// The SecretSticky theme.
pub fn theme() -> Theme {
    let mut t = remedy_ui_themes::theme(&BRAND);
    if let Ok(c) = Color::from_hex(ON_ACCENT) {
        t.colors.on_accent = c;
    }
    t
}

/// The well a recovery key sits in: the shipped `.recovery` background.
const INSET: &str = "#111214";

/// The Remedy widget sheet on the SecretSticky theme, with the shipped app's surfaces: cards
/// on the panel color (buttons keep the lighter one), an inset well, and success text.
pub fn sheet(t: &Theme) -> StyleSheet {
    let mut s = remedy_ui_themes::sheet(t);
    let c = &t.colors;
    s.rule("card", None, States::NONE, StylePatch::new().background(c.panel));
    let inset = Color::from_hex(INSET).unwrap_or(c.bg);
    s.rule("card", Some("inset"), States::NONE, StylePatch::new().background(inset));
    s.rule("label", Some("ok"), States::NONE, StylePatch::new().foreground(c.ok));
    // A note's title and text sit on the note's own color: no frame until focused, then a
    // frame in the note's ink (dark on light notes, light on dark ones).
    for w in ["text_input", "text_area"] {
        for (variant, focus) in [("note_light", Color::BLACK.with_alpha(0.35)), ("note_dark", Color::WHITE.with_alpha(0.4))] {
            let clear = StylePatch::new().background(Color::TRANSPARENT).border(Color::TRANSPARENT);
            s.rule(w, Some(variant), States::NONE, clear.clone());
            s.rule(w, Some(variant), States::HOVER, StylePatch::new().border(focus.with_alpha(0.15)));
            s.rule(w, Some(variant), States::FOCUS, StylePatch::new().border(focus).outline_width(0.0));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_brand_colors_are_the_shipped_ones() {
        let t = super::theme();
        let hex = |c: remedy_ui::core::Color| c.to_hex();
        assert_eq!(hex(t.colors.bg), "#1A1B1E");
        assert_eq!(hex(t.colors.panel), "#24262B");
        assert_eq!(hex(t.colors.accent), "#FBBF24");
        assert_eq!(hex(t.colors.on_accent), "#1A1B1E");
        assert_eq!(hex(t.colors.danger), "#F87171");
    }
}
