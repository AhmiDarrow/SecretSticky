import { catalogs, type Catalog } from "./catalogs";
import { isLocaleId, matchLocale, type LocaleId } from "./locales";

export type TranslateVars = Record<string, string | number>;
export type TranslateFn = (key: string, vars?: TranslateVars) => string;

const PLACEHOLDER = /\{([A-Za-z0-9_]+)\}/g;

function lookup(table: Catalog | undefined, key: string): string | undefined {
  if (!table) return undefined;
  const value = table[key];
  if (typeof value !== "string" || value.length === 0) return undefined;
  return value;
}

function applyVars(text: string, vars: TranslateVars | undefined): string {
  if (!vars) return text;
  return text.replace(PLACEHOLDER, (whole, name: string) => {
    if (Object.prototype.hasOwnProperty.call(vars, name)) {
      return String(vars[name]);
    }
    return whole;
  });
}

/** English, then the key itself. Never blank, never throws. */
export function translateFrom(
  tables: Record<string, Catalog>,
  locale: string,
  key: string,
  vars?: TranslateVars,
): string {
  const id: LocaleId = isLocaleId(locale) ? locale : matchLocale(locale);
  const text =
    lookup(tables[id], key) ??
    (id === "en" ? undefined : lookup(tables.en, key)) ??
    key;
  return applyVars(text, vars);
}

export function translate(
  locale: string,
  key: string,
  vars?: TranslateVars,
): string {
  return translateFrom(catalogs, locale, key, vars);
}
