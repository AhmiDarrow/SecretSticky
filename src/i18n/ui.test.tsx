import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { catalogs } from "./catalogs";
import { I18nProvider, useI18n } from "./I18nProvider";
import { LanguageSelect } from "./LanguageSelect";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}));

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function Probe() {
  const { t, locale } = useI18n();
  return (
    <div>
      <span data-probe="locale">{locale}</span>
      <span data-probe="tagline">{t("manager.tagline")}</span>
      <span data-probe="hello">{t("about.hello")}</span>
      <LanguageSelect />
    </div>
  );
}

describe("language UI", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
  });

  async function render(locale: string) {
    if (root) {
      act(() => {
        root?.unmount();
      });
      host?.remove();
      root = null;
      host = null;
    }
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root?.render(
        <I18nProvider initialLocale={locale} persist={false}>
          <Probe />
        </I18nProvider>,
      );
    });
  }

  it("renders Spanish, German, and Japanese from the catalogs", async () => {
    await render("es");
    expect(host?.querySelector("[data-probe=locale]")?.textContent).toBe("es");
    expect(host?.querySelector("[data-probe=tagline]")?.textContent).toBe(
      catalogs.es["manager.tagline"],
    );
    expect(host?.querySelector("[data-probe=tagline]")?.textContent).not.toBe(
      catalogs.en["manager.tagline"],
    );
    expect(document.documentElement.lang).toBe("es");

    await render("de");
    expect(host?.querySelector("[data-probe=tagline]")?.textContent).toBe(
      catalogs.de["manager.tagline"],
    );
    expect(document.documentElement.lang).toBe("de");

    await render("ja");
    expect(host?.querySelector("[data-probe=tagline]")?.textContent).toBe(
      catalogs.ja["manager.tagline"],
    );
    expect(host?.querySelector("[data-probe=hello]")?.textContent).toBe(
      "Hi I'm Ahmi, hope this helps!",
    );
    expect(document.documentElement.lang).toBe("ja");

    const names = [...(host?.querySelectorAll("option") ?? [])].map(
      (option) => option.textContent,
    );
    expect(names).toContain("Deutsch");
    expect(names).toContain("日本語");
    expect(names).toContain("简体中文");
  });
});
