import { LOCALES } from "./locales";
import { useI18n } from "./I18nProvider";

/** Compact picker for the unlock and recovery screens (before a vault exists). */
export function LanguageSelect({ compact = false }: { compact?: boolean }) {
  const { locale, setLocale, t } = useI18n();
  return (
    <label className={compact ? "lang-select compact" : "lang-select"}>
      <span className="muted">{t("language.current")}</span>
      <select
        aria-label={t("language.aria")}
        value={locale}
        onChange={(event) => {
          void setLocale(event.target.value);
        }}
      >
        {LOCALES.map((item) => (
          <option key={item.id} value={item.id}>
            {item.native}
          </option>
        ))}
      </select>
    </label>
  );
}
