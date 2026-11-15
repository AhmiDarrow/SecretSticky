/** Supported UI languages. Native names are not translated per locale. */
export const LOCALES = [
  { id: "en", native: "English" },
  { id: "es", native: "Español" },
  { id: "fr", native: "Français" },
  { id: "de", native: "Deutsch" },
  { id: "pt-BR", native: "Português (Brasil)" },
  { id: "it", native: "Italiano" },
  { id: "ru", native: "Русский" },
  { id: "pl", native: "Polski" },
  { id: "ja", native: "日本語" },
  { id: "ko", native: "한국어" },
  { id: "zh-CN", native: "简体中文" },
] as const;

export type LocaleId = (typeof LOCALES)[number]["id"];

const EXACT = new Map<string, LocaleId>(
  LOCALES.map((locale) => [locale.id.toLowerCase(), locale.id]),
);

export function isLocaleId(value: string): value is LocaleId {
  return LOCALES.some((locale) => locale.id === value);
}

/**
 * Map an OS or saved tag onto a supported locale.
 * Traditional Chinese stays on English (we ship zh-CN only).
 * Any Portuguese tag maps to Brazilian Portuguese.
 */
export function matchLocale(tag: string | null | undefined): LocaleId {
  if (!tag) return "en";
  const lower = tag.trim().replace(/_/g, "-").toLowerCase();
  if (!lower) return "en";
  const exact = EXACT.get(lower);
  if (exact) return exact;
  if (
    lower === "zh-tw" ||
    lower === "zh-hk" ||
    lower === "zh-mo" ||
    lower.includes("hant")
  ) {
    return "en";
  }
  const primary = lower.split("-")[0] ?? "";
  switch (primary) {
    case "es":
      return "es";
    case "fr":
      return "fr";
    case "de":
      return "de";
    case "pt":
      return "pt-BR";
    case "it":
      return "it";
    case "ru":
      return "ru";
    case "pl":
      return "pl";
    case "ja":
      return "ja";
    case "ko":
      return "ko";
    case "zh":
      return "zh-CN";
    default:
      return "en";
  }
}
