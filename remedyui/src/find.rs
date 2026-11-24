//! Searching text, and the chords the windows answer to.

use std::ops::Range;

use remedy_ui::input::{KeyEvent, Modifiers, Platform};

/// Where `needle` occurs in `hay`, ignoring case in every script, as byte ranges of `hay`
/// that do not overlap. An empty needle is found nowhere.
pub fn matches(hay: &str, needle: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    // Each lower-cased character with the bytes of the character it came from.
    let folded: Vec<(char, usize, usize)> = hay
        .char_indices()
        .flat_map(|(i, c)| {
            let end = i + c.len_utf8();
            c.to_lowercase().map(move |l| (l, i, end))
        })
        .collect();
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut k = 0;
    while let Some(window) = folded.get(k..k + needle.len()) {
        let (Some(first), Some(last)) = (window.first(), window.last()) else { break };
        // Lower-casing can turn one character into several: a match covers whole characters
        // of the text and never overlaps the match before it.
        if window.iter().zip(&needle).all(|((c, ..), n)| c == n) && out.last().is_none_or(|r| r.end <= first.1) {
            out.push(first.1..last.2);
            k += needle.len();
        } else {
            k += 1;
        }
    }
    out
}

/// True when `needle` occurs in `hay`, ignoring case.
pub fn contains(hay: &str, needle: &str) -> bool {
    !matches(hay, needle).is_empty()
}

/// True for a press of the shortcut modifier (Ctrl; Command on Apple platforms) with
/// `letter`, and with Shift exactly when `shift`. The letter is the one the key types, or
/// on a layout without Latin letters (Russian) the one in its place on a US keyboard.
pub fn chord(e: &KeyEvent, platform: Platform, letter: &str, shift: bool) -> bool {
    let primary = Modifiers::primary(platform);
    e.pressed
        && !e.composing
        && !e.modifiers.alt
        && e.modifiers.ctrl == primary.ctrl
        && e.modifiers.meta == primary.meta
        && e.modifiers.shift == shift
        && e.is_shortcut_key(letter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use remedy_ui::input::Key;

    #[test]
    fn matches_ignore_case_and_never_overlap() {
        assert_eq!(matches("Key: KEY, key", "key"), vec![0..3, 5..8, 10..13]);
        assert_eq!(matches("aaaa", "aa"), vec![0..2, 2..4]);
        assert_eq!(matches("anything", ""), Vec::<Range<usize>>::new());
        assert_eq!(matches("short", "much longer"), Vec::<Range<usize>>::new());
        // Byte ranges on character boundaries, in any script.
        assert_eq!(matches("Пароль: ПАРОЛЬ", "пароль"), vec![0..12, 14..26]);
        assert_eq!(matches("Straße", "STRASSE").len(), 0, "ß is not folded to ss: no false match, no panic");
        assert!(contains("Wi-Fi password", "PASS") && !contains("Wi-Fi", "pass"));
    }

    #[test]
    fn chords_need_the_shortcut_modifier_alone() {
        let press = |k: &str, m: Modifiers| KeyEvent::press(Key::Character(k.into())).with_modifiers(m);
        let p = Platform::Standard;
        assert!(chord(&press("f", Modifiers::CTRL), p, "f", false));
        assert!(!chord(&press("f", Modifiers::NONE), p, "f", false), "typing f is not Ctrl+F");
        assert!(!chord(&press("f", Modifiers { shift: true, ..Modifiers::CTRL }), p, "f", false));
        assert!(chord(&press("g", Modifiers { shift: true, ..Modifiers::CTRL }), p, "g", true));
        assert!(!chord(&press("f", Modifiers { alt: true, ..Modifiers::CTRL }), p, "f", false), "AltGr types a character");
        assert!(!chord(&KeyEvent::release(Key::Character("f".into())).with_modifiers(Modifiers::CTRL), p, "f", false));
        // A Russian layout: the F key types "а".
        assert!(chord(&press("а", Modifiers::CTRL).with_physical("KeyF"), p, "f", false));
    }
}
