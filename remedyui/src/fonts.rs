//! Inter, the typeface the shipped SecretSticky uses: Fontsource's `@fontsource/inter` 5.3.0 as the
//! React app imported it (`src/main.tsx`: weights 400, 500, 600, 700 and 800, normal style),
//! every script subset the browser loaded from it (Latin, Latin Extended, Cyrillic, Cyrillic
//! Extended, Greek, Greek Extended and Vietnamese), as the same WOFF2 files. SIL Open Font
//! License 1.1 (`assets/fonts/inter/OFL.txt`).

use remedy_ui::AppConfig;
use remedy_ui::text::FontFile;

/// The family the files hold.
pub const FAMILY: &str = "Inter";

/// Every file, Latin first: each weight's subsets join as one family, and each character is
/// drawn from the subset that covers it, as CSS's `unicode-range` does.
const FILES: [&[u8]; 35] = [
    include_bytes!("../assets/fonts/inter/inter-latin-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-800-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-ext-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-ext-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-ext-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-ext-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-latin-ext-800-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-800-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-ext-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-ext-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-ext-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-ext-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-cyrillic-ext-800-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-800-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-ext-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-ext-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-ext-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-ext-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-greek-ext-800-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-vietnamese-400-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-vietnamese-500-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-vietnamese-600-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-vietnamese-700-normal.woff2"),
    include_bytes!("../assets/fonts/inter/inter-vietnamese-800-normal.woff2"),
];

/// The typefaces a person can choose for every window: Inter (the shipped one, the default)
/// and Ahmi Darrow's own families, each in five weights, SIL Open Font License 1.1
/// (`assets/fonts/*/OFL.txt`). Each is (setting id, family name in the files, shown name).
pub const CHOICES: [(&str, &str, &str); 5] = [
    ("inter", FAMILY, "Inter"),
    ("remedy-sans", "Remedy Sans", "Remedy Sans"),
    ("remedy-sans-ui", "Remedy Sans UI", "Remedy Sans UI"),
    ("ahmi-print", "AhmiPrint", "Ahmi Print"),
    ("ahmi-pixel", "Ahmi Pixel", "Ahmi Pixel"),
];

/// The family for a saved choice; Inter for none or one this version does not know.
pub fn family(id: &str) -> &'static str {
    CHOICES.iter().find(|(i, _, _)| *i == id).map_or(FAMILY, |(_, f, _)| f)
}

/// The chosen families' files: Light, Semilight, Regular, Semibold and Bold of each.
const OWN: [&[u8]; 20] = [
    include_bytes!("../assets/fonts/remedy-sans/RemedySans-Light.woff2"),
    include_bytes!("../assets/fonts/remedy-sans/RemedySans-Semilight.woff2"),
    include_bytes!("../assets/fonts/remedy-sans/RemedySans-Regular.woff2"),
    include_bytes!("../assets/fonts/remedy-sans/RemedySans-Semibold.woff2"),
    include_bytes!("../assets/fonts/remedy-sans/RemedySans-Bold.woff2"),
    include_bytes!("../assets/fonts/remedy-sans-ui/RemedySansUI-Light.woff2"),
    include_bytes!("../assets/fonts/remedy-sans-ui/RemedySansUI-Semilight.woff2"),
    include_bytes!("../assets/fonts/remedy-sans-ui/RemedySansUI-Regular.woff2"),
    include_bytes!("../assets/fonts/remedy-sans-ui/RemedySansUI-Semibold.woff2"),
    include_bytes!("../assets/fonts/remedy-sans-ui/RemedySansUI-Bold.woff2"),
    include_bytes!("../assets/fonts/ahmi-print/AhmiPrint-Light.woff2"),
    include_bytes!("../assets/fonts/ahmi-print/AhmiPrint-Semilight.woff2"),
    include_bytes!("../assets/fonts/ahmi-print/AhmiPrint-Regular.woff2"),
    include_bytes!("../assets/fonts/ahmi-print/AhmiPrint-Semibold.woff2"),
    include_bytes!("../assets/fonts/ahmi-print/AhmiPrint-Bold.woff2"),
    include_bytes!("../assets/fonts/ahmi-pixel/AhmiPixel-Light.woff2"),
    include_bytes!("../assets/fonts/ahmi-pixel/AhmiPixel-Semilight.woff2"),
    include_bytes!("../assets/fonts/ahmi-pixel/AhmiPixel-Regular.woff2"),
    include_bytes!("../assets/fonts/ahmi-pixel/AhmiPixel-Semibold.woff2"),
    include_bytes!("../assets/fonts/ahmi-pixel/AhmiPixel-Bold.woff2"),
];

/// `config` with Inter on every window, first in the theme's families (the Remedy families stay
/// behind it, as `system-ui` and `sans-serif` were behind it in the shipped CSS).
pub fn with_inter(mut config: AppConfig) -> AppConfig {
    for data in FILES {
        // Each file is checked by `every_inter_file_loads`; one that would not read is left
        // out rather than stopping the app.
        if let Ok(file) = FontFile::new(data) {
            config = config.font(file);
        }
    }
    for data in OWN {
        if let Ok(file) = FontFile::new(data) {
            config = config.font(file);
        }
    }
    if !config.theme.font_families.iter().any(|f| f == FAMILY) {
        config.theme.font_families.insert(0, FAMILY.to_owned());
    }
    config
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_inter_file_loads() {
        for data in super::FILES {
            let file = remedy_ui::text::FontFile::new(data).expect("a WOFF2 font");
            assert_eq!(file.families(), [super::FAMILY]);
        }
        let config = super::with_inter(remedy_ui::AppConfig::new("Inter"));
        assert_eq!(config.theme.font_families.first().map(String::as_str), Some(super::FAMILY));
    }

    #[test]
    fn every_choice_loads_as_one_family_in_five_weights() {
        for (i, data) in super::OWN.iter().enumerate() {
            let file = remedy_ui::text::FontFile::new(data).expect("a WOFF2 font");
            let (_, family, _) = super::CHOICES[1 + i / 5];
            assert_eq!(file.families(), [family], "file {i} joins its family");
        }
        assert_eq!(super::family("ahmi-pixel"), "Ahmi Pixel");
        assert_eq!(super::family("from-a-newer-version"), super::FAMILY);
    }
}
