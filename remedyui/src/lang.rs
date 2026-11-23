//! The interface language, as SecretSticky 0.2.0 has it: the shipped catalogs
//! (`src/locales/*.json`) through the shipped `i18n.rs`, the choice kept in `settings.json`
//! beside the vault, and every label following a switch at once (labels read [`Lang`],
//! which is tracked).

use remedy_ui::reactive::Signal;

use crate::error::AppError;

/// The languages SecretSticky ships, with their own names (`src/i18n/locales.ts`).
pub const LOCALES: [(&str, &str); 11] = [
    ("en", "English"),
    ("es", "Español"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("pt-BR", "Português (Brasil)"),
    ("it", "Italiano"),
    ("ru", "Русский"),
    ("pl", "Polski"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("zh-CN", "简体中文"),
];

/// The language every window shows (a shipped locale id).
#[derive(Clone, Copy)]
pub struct Lang(pub Signal<String>);

impl Lang {
    /// The current id, tracked.
    pub fn id(&self) -> String {
        self.0.get()
    }

    /// The catalog text for `key` (English, then the key itself, when missing), tracked.
    pub fn t(&self, key: &str) -> String {
        secretsticky_shipped_i18n::t(&self.id(), key)
    }

    /// `key`'s text as a live label: widgets that take a closure redraw on a switch.
    pub fn k(self, key: &'static str) -> impl Fn() -> String + Clone + 'static {
        move || self.t(key)
    }

    /// Names `field` (for assistive technology) and its placeholder in the current language,
    /// again after every switch.
    pub fn name_field(self, field: &remedy_ui::TextField, label: &'static str, placeholder: Option<&'static str>) {
        let f = field.clone();
        let _ = field.el().bind(move || {
            let f = f.clone().label(self.t(label));
            if let Some(p) = placeholder {
                let _ = f.placeholder(self.t(p));
            }
        });
    }

    /// [`Lang::t`] with `{name}` placeholders filled.
    pub fn tv(&self, key: &str, vars: &[(&str, &str)]) -> String {
        fill(&self.t(key), vars)
    }

    /// A count in the locale's plural form (`<key>.one`, `.few`, `.many` or `.other`).
    pub fn count(&self, key: &str, var: &str, n: f64) -> String {
        let id = self.id();
        let form = format!("{key}.{}", plural(&id, n));
        fill(&secretsticky_shipped_i18n::t(&id, &form), &[(var, &number(&id, n))])
    }

    /// An auto-lock time in words: "Off", "15 min", "1 hr", "1,5 Std." (`formatDuration`).
    pub fn duration(&self, secs: u64) -> String {
        if secs == 0 {
            return self.t("autoLock.off");
        }
        let mins = secs as f64 / 60.0;
        if mins < 60.0 {
            return self.count("duration.min", "n", mins.round());
        }
        self.count("duration.hr", "n", (mins / 60.0 * 10.0).round() / 10.0)
    }

    /// The own name of the current language.
    pub fn native(&self) -> &'static str {
        let id = self.id();
        LOCALES.iter().find(|(i, _)| *i == id).map_or("English", |(_, n)| n)
    }
}

/// `text` with each `{name}` in `vars` replaced; unknown placeholders stay as written.
pub fn fill(text: &str, vars: &[(&str, &str)]) -> String {
    let mut out = text.to_owned();
    for (name, value) in vars {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// The CLDR plural category of `n` for a shipped locale (what `Intl.PluralRules` says for
/// these eleven). Fractions take "other" in every one of them except French and Portuguese
/// below 2, which take "one".
pub fn plural(lang: &str, n: f64) -> &'static str {
    let whole = n.fract() == 0.0;
    let i = n.trunc() as u64;
    match lang {
        "ja" | "ko" | "zh-CN" => "other",
        "fr" | "pt-BR" => {
            if i <= 1 {
                "one"
            } else if whole && i != 0 && i % 1_000_000 == 0 {
                "many"
            } else {
                "other"
            }
        }
        "ru" | "pl" if whole => {
            let (m10, m100) = (i % 10, i % 100);
            if lang == "ru" && m10 == 1 && m100 != 11 || lang == "pl" && i == 1 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "ru" | "pl" => "other",
        "es" | "it" if whole && i != 0 && i % 1_000_000 == 0 => "many",
        _ if whole && i == 1 => "one",
        _ => "other",
    }
}

/// `n` written the locale's way: whole numbers bare, fractions to one place with the
/// locale's decimal sign (`formatNumber`; these counts never reach thousands).
pub fn number(lang: &str, n: f64) -> String {
    if !n.is_finite() {
        return n.to_string();
    }
    if n.fract() == 0.0 {
        return format!("{n:.0}");
    }
    let s = format!("{n:.1}");
    match lang {
        "en" | "ja" | "ko" | "zh-CN" => s,
        _ => s.replace('.', ","),
    }
}

/// Something the manager says, translated when drawn so it follows a language switch.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// A catalog key with its placeholders.
    Key(&'static str, Vec<(&'static str, String)>),
    /// A vault error, localized as the shipped `localizeMessage` does.
    Error(String),
}

impl Msg {
    /// A catalog key with no placeholders.
    pub fn key(key: &'static str) -> Msg {
        Msg::Key(key, Vec::new())
    }

    /// The text in `lang`, tracked.
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Msg::Key(k, vars) => {
                let vars: Vec<(&str, &str)> = vars.iter().map(|(n, v)| (*n, v.as_str())).collect();
                lang.tv(k, &vars)
            }
            Msg::Error(raw) => localize_error(lang, raw),
        }
    }
}

impl From<AppError> for Msg {
    fn from(e: AppError) -> Msg {
        Msg::Error(e.to_string())
    }
}

/// A vault or system error in the interface language: the shipped `localizeMessage`
/// (`src/i18n/localizeError.ts`), which reads the vault's English messages.
pub fn localize_error(lang: Lang, raw: &str) -> String {
    let msg = raw.trim();
    if msg.is_empty() {
        return lang.t("error.somethingWrong");
    }
    let exact = [
        ("vault is locked", "error.vaultLocked"),
        ("vault already unlocked", "error.alreadyUnlocked"),
        ("vault not initialized", "error.notInitialized"),
        ("vault already exists", "error.alreadyExists"),
        ("note not found", "error.noteNotFound"),
        ("cannot resolve app data dir", "error.noDataDir"),
        ("password must be at least 12 characters", "error.passwordShort"),
        ("no recovery key configured", "error.noRecovery"),
        ("Something went wrong", "error.somethingWrong"),
        ("url not allowed", "error.urlNotAllowed"),
    ];
    if let Some((_, key)) = exact.iter().find(|(m, _)| *m == msg) {
        return lang.t(key);
    }
    let lower = msg.to_lowercase();
    if ["invalid master password", "bad password", "invalid password", "wrong password", "invalid recovery", "bad recovery"]
        .iter()
        .any(|p| lower.contains(p))
    {
        return lang.t("error.badPassword");
    }
    let digits = |s: &str| s.chars().take_while(char::is_ascii_digit).collect::<String>();
    if let Some(rest) = msg.strip_prefix("vault format version ") {
        let version = digits(rest);
        let after = &rest[version.len()..];
        if let Some(sup) = after.strip_prefix(" is newer than this app supports (") {
            return lang.tv("error.vaultVersionNewer", &[("version", &version), ("supported", &digits(sup))]);
        }
        if after.starts_with(" is invalid") && !version.is_empty() {
            return lang.tv("error.vaultVersionInvalid", &[("version", &version)]);
        }
    }
    if let Some(rest) = msg.strip_prefix("Too many failed attempts. Retry in ") {
        let secs: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let n = secs.parse::<f64>().unwrap_or(0.0);
        return lang.tv("error.tooMany", &[("seconds", &number(&lang.id(), n))]);
    }
    let prefixes = [
        ("atomic vault replace failed (saved stickies left untouched): ", "error.atomicReplace"),
        ("io: ", "error.io"),
        ("crypto: ", "error.crypto"),
        ("serialize: ", "error.serialize"),
        ("open url: ", "error.openUrl"),
        ("window: ", "error.window"),
    ];
    for (prefix, key) in prefixes {
        if let Some(detail) = msg.strip_prefix(prefix) {
            return lang.tv(key, &[("detail", detail)]);
        }
    }
    msg.to_owned()
}

/// Splits a catalog string on its one `<strong>` pair (`RichText` in `src/i18n/text.tsx`):
/// the runs in order, each with whether it is strong. Only catalog text goes through this.
pub fn strong_runs(text: &str) -> Vec<(String, bool)> {
    let mut runs = Vec::new();
    let mut strong = false;
    let mut rest = text;
    while !rest.is_empty() {
        let tag = if strong { "</strong>" } else { "<strong>" };
        let (run, next) = match rest.find(tag) {
            Some(at) => (&rest[..at], &rest[at + tag.len()..]),
            None => (rest, ""),
        };
        if !run.is_empty() {
            runs.push((run.to_owned(), strong));
        }
        strong = !strong;
        rest = next;
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(id: &str) -> Lang {
        Lang(Signal::new(id.to_owned()))
    }

    #[test]
    fn plural_forms_follow_cldr_for_every_shipped_locale() {
        for (l, n, want) in [
            ("en", 1.0, "one"),
            ("en", 2.0, "other"),
            ("en", 1.5, "other"),
            ("fr", 0.0, "one"),
            ("fr", 1.5, "one"),
            ("fr", 2.0, "other"),
            ("pt-BR", 1.0, "one"),
            ("ru", 1.0, "one"),
            ("ru", 21.0, "one"),
            ("ru", 11.0, "many"),
            ("ru", 3.0, "few"),
            ("ru", 12.0, "many"),
            ("ru", 1.5, "other"),
            ("pl", 1.0, "one"),
            ("pl", 21.0, "many"),
            ("pl", 22.0, "few"),
            ("pl", 5.0, "many"),
            ("ja", 1.0, "other"),
            ("de", 1.0, "one"),
            ("de", 0.0, "other"),
        ] {
            assert_eq!(plural(l, n), want, "{l} {n}");
        }
    }

    #[test]
    fn durations_read_as_the_shipped_app_wrote_them() {
        let en = lang("en");
        assert_eq!(en.duration(0), "Off");
        assert_eq!(en.duration(60), "1 min");
        assert_eq!(en.duration(900), "15 min");
        assert_eq!(en.duration(3600), "1 hr");
        assert_eq!(en.duration(5400), "1.5 hrs");
        assert_eq!(en.duration(43_200), "12 hrs");
        let de = lang("de");
        assert!(de.duration(5400).starts_with("1,5"), "{}", de.duration(5400));
        assert_ne!(de.duration(0), "Off");
        assert_eq!(lang("ru").count("manager.count", "count", 3.0), secretsticky_shipped_i18n::t("ru", "manager.count.few").replace("{count}", "3"));
    }

    #[test]
    fn vault_errors_are_localized_as_shipped() {
        let en = lang("en");
        assert_eq!(localize_error(en, "invalid master password"), "That password is not correct.");
        assert_eq!(localize_error(en, "vault is locked"), "The vault is locked.");
        assert_eq!(localize_error(en, "Too many failed attempts. Retry in 30s."), "Too many failed attempts. Retry in 30s.");
        assert_eq!(localize_error(en, "io: disk full"), "Could not read or write a file. disk full");
        assert_eq!(
            localize_error(en, "vault format version 9 is newer than this app supports (1)"),
            "Vault format version 9 is newer than this app supports (1). Upgrade SecretSticky. Saved stickies were not changed."
        );
        assert_eq!(localize_error(en, "something new"), "something new");
        let de = lang("de");
        assert_ne!(localize_error(de, "invalid master password"), "That password is not correct.");
    }

    #[test]
    fn the_recovery_text_keeps_its_one_strong_run() {
        let runs = strong_runs("This is shown <strong>once</strong>. Store it.");
        assert_eq!(runs, vec![("This is shown ".into(), false), ("once".into(), true), (". Store it.".into(), false)]);
        for (id, _) in LOCALES {
            let runs = strong_runs(&secretsticky_shipped_i18n::t(id, "recovery.body"));
            assert_eq!(runs.iter().filter(|r| r.1).count(), 1, "{id}");
        }
    }
}
