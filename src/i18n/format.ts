import type { TranslateFn } from "./translate";

export const AUTO_LOCK_PRESET_SECS = [
  0, 60, 300, 900, 1800, 3600, 7200, 14400, 28800, 43200,
] as const;

export function formatNumber(locale: string, value: number): string {
  if (!Number.isFinite(value)) return String(value);
  try {
    const digits = Number.isInteger(value) ? 0 : 1;
    return new Intl.NumberFormat(locale, {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    }).format(value);
  } catch {
    return String(value);
  }
}

/** Available for locale-aware dates. No screen renders a date today. */
export function formatDateTime(
  locale: string,
  value: Date | number | string,
): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  try {
    return new Intl.DateTimeFormat(locale, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
  } catch {
    return date.toISOString();
  }
}

export function pluralCategory(
  locale: string,
  n: number,
): "one" | "few" | "many" | "other" {
  try {
    const category = new Intl.PluralRules(locale).select(n);
    if (category === "one" || category === "few" || category === "many") {
      return category;
    }
  } catch {
    /* fall through */
  }
  return "other";
}

export function formatDuration(
  locale: string,
  secs: number,
  t: TranslateFn,
): string {
  if (!Number.isFinite(secs) || secs <= 0) return t("autoLock.off");
  const mins = secs / 60;
  if (mins < 60) {
    const n = Math.round(mins);
    const category = pluralCategory(locale, n);
    return t(`duration.min.${category}`, { n: formatNumber(locale, n) });
  }
  const hrs = Math.round((mins / 60) * 10) / 10;
  const category = pluralCategory(locale, hrs);
  return t(`duration.hr.${category}`, { n: formatNumber(locale, hrs) });
}
