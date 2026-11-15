import { describe, expect, it } from "vitest";
import { catalogs } from "./catalogs";
import {
  formatDateTime,
  formatDuration,
  formatNumber,
  pluralCategory,
} from "./format";
import { localizeMessage } from "./localizeError";
import { LOCALES, matchLocale, type LocaleId } from "./locales";
import { translate, translateFrom } from "./translate";

const PLACEHOLDER = /\{([A-Za-z0-9_]+)\}/g;
const BRANDS = [
  "SecretSticky",
  "Ahmi",
  "Argon2id",
  "XChaCha20-Poly1305",
  "GitHub",
  "MIT",
  "Windows",
];
const HELLO = "Hi I'm Ahmi, hope this helps!";

function placeholderSet(value: string): string[] {
  const names = new Set<string>();
  for (const match of value.matchAll(PLACEHOLDER)) {
    names.add(match[1]);
  }
  return [...names].sort();
}

function strongCounts(value: string): [number, number] {
  return [value.split("<strong>").length - 1, value.split("</strong>").length - 1];
}

const en = catalogs.en;
const enKeys = Object.keys(en).sort();

describe("locale catalogs", () => {
  it("ships English plus the ten target languages", () => {
    expect(LOCALES.map((locale) => locale.id)).toEqual([
      "en",
      "es",
      "fr",
      "de",
      "pt-BR",
      "it",
      "ru",
      "pl",
      "ja",
      "ko",
      "zh-CN",
    ]);
    expect(Object.keys(catalogs).sort()).toEqual(
      [...LOCALES.map((locale) => locale.id)].sort(),
    );
  });

  it("gives every locale the English key set, placeholders, and markup", () => {
    expect(enKeys.length).toBeGreaterThan(100);
    for (const locale of LOCALES) {
      const table = catalogs[locale.id];
      expect(Object.keys(table).sort(), locale.id).toEqual(enKeys);
      for (const key of enKeys) {
        const value = table[key];
        expect(value.length, `${locale.id}:${key}`).toBeGreaterThan(0);
        expect(placeholderSet(value), `${locale.id}:${key}`).toEqual(
          placeholderSet(en[key]),
        );
        expect(strongCounts(value), `${locale.id}:${key}`).toEqual(
          strongCounts(en[key]),
        );
        for (const brand of BRANDS) {
          if (en[key].includes(brand)) {
            expect(value, `${locale.id}:${key}:${brand}`).toContain(brand);
          }
        }
      }
      expect(table["about.hello"]).toBe(HELLO);
    }
  });

  it("uses the expected scripts for Japanese, Korean, Chinese, and Russian", () => {
    expect(catalogs.ja["manager.tagline"]).toMatch(/\p{Script=Han}|\p{Script=Hiragana}/u);
    expect(catalogs.ko["manager.tagline"]).toMatch(/\p{Script=Hangul}/u);
    expect(catalogs["zh-CN"]["manager.tagline"]).toMatch(/\p{Script=Han}/u);
    expect(catalogs.ru["manager.tagline"]).toMatch(/\p{Script=Cyrillic}/u);
    expect(catalogs.de["manager.tagline"]).not.toBe(en["manager.tagline"]);
  });
});

describe("matchLocale", () => {
  it("maps OS tags and falls back to English", () => {
    const cases: Array<[string, LocaleId]> = [
      ["de-DE", "de"],
      ["pt-BR", "pt-BR"],
      ["pt-PT", "pt-BR"],
      ["zh-CN", "zh-CN"],
      ["zh-TW", "en"],
      ["ja-JP", "ja"],
      ["en-US", "en"],
      ["fr-CA", "fr"],
      ["unknown", "en"],
      ["zh-Hans", "zh-CN"],
      ["zh-hant-tw", "en"],
      ["PT_br", "pt-BR"],
      ["zh-HK", "en"],
      ["zh-SG", "zh-CN"],
    ];
    for (const [tag, id] of cases) {
      expect(matchLocale(tag), tag).toBe(id);
    }
    expect(matchLocale(null)).toBe("en");
    expect(matchLocale("")).toBe("en");
  });
});

describe("translate", () => {
  it("falls back to English, then the key, and keeps unknown placeholders", () => {
    const tables = {
      en: { "a.b": "Hello {name}", "only.en": "English" },
      es: { "a.b": "", "only.en": "" },
    };
    expect(translateFrom(tables, "es", "a.b", { name: "Ada" })).toBe("Hello Ada");
    expect(translateFrom(tables, "es", "only.en")).toBe("English");
    expect(translateFrom(tables, "es", "missing.key")).toBe("missing.key");
    expect(translate("en", "about.hello")).toBe(HELLO);
    expect(translate("en", "about.version", { version: "0.1.8", extra: 1 })).toBe(
      "Version 0.1.8",
    );
    expect(translate("en", "about.version", {})).toBe("Version {version}");
  });
});

describe("format", () => {
  const t = (key: string, vars?: Record<string, string | number>) =>
    translate("en", key, vars);

  it("formats durations with plural categories", () => {
    expect(formatDuration("en", 0, t)).toBe("Off");
    expect(formatDuration("en", 60, t)).toBe("1 min");
    expect(formatDuration("en", 3600, t)).toBe("1 hr");
    expect(formatDuration("en", 7200, t)).toBe("2 hrs");
    expect(pluralCategory("ru", 2)).toBe("few");
    expect(formatNumber("de", 1.5)).toMatch(/1,5|1\.5/);
  });

  it("formats dates for the locale without being shown in the UI", () => {
    const stamp = new Date("2026-11-15T15:04:00Z");
    const english = formatDateTime("en", stamp);
    const german = formatDateTime("de", stamp);
    expect(english.length).toBeGreaterThan(0);
    expect(german.length).toBeGreaterThan(0);
    expect(german).not.toBe(english);
  });
});

describe("localizeMessage", () => {
  const t = (key: string, vars?: Record<string, string | number>) =>
    translate("en", key, vars);

  it("maps known backend English and leaves other text alone", () => {
    expect(localizeMessage("vault is locked", "en", t)).toBe("The vault is locked.");
    expect(localizeMessage("invalid master password", "en", t)).toBe(
      "That password is not correct.",
    );
    expect(localizeMessage("incorrect password", "en", t)).toBe("incorrect password");
    expect(
      localizeMessage(
        "vault format version 3 is newer than this app supports (1) — upgrade SecretSticky",
        "en",
        t,
      ),
    ).toContain("SecretSticky");
    expect(localizeMessage("Too many failed attempts. Retry in 12s.", "de", t)).toContain(
      "12",
    );
    expect(localizeMessage("io: disk full", "en", t)).toContain("disk full");
    expect(localizeMessage("", "en", t)).toBe("Something went wrong");
    expect(localizeMessage("Passwords do not match", "en", t)).toBe(
      "Passwords do not match",
    );
  });
});
