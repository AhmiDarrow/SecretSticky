import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { isLocaleId, matchLocale, type LocaleId } from "./locales";
import { translate, type TranslateFn, type TranslateVars } from "./translate";

interface I18nContextValue {
  locale: LocaleId;
  setLocale: (id: string) => Promise<void>;
  t: TranslateFn;
  bootCi: boolean;
}

const I18nContext = createContext<I18nContextValue | null>(null);

interface ProviderProps {
  children: ReactNode;
  /** Tests and previews can pin a locale without reading settings. */
  initialLocale?: string;
  /** When false, skip Tauri boot/save (unit tests, browser preview without a vault). */
  persist?: boolean;
}

export function I18nProvider({
  children,
  initialLocale,
  persist = true,
}: ProviderProps) {
  const [locale, setLocaleState] = useState<LocaleId>(() =>
    initialLocale
      ? matchLocale(initialLocale)
      : matchLocale(
          typeof navigator !== "undefined" ? navigator.language : "en",
        ),
  );
  const localeRef = useRef(locale);
  localeRef.current = locale;
  const [bootCi, setBootCi] = useState(false);

  useLayoutEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const t = useCallback<TranslateFn>(
    (key: string, vars?: TranslateVars) => translate(locale, key, vars),
    [locale],
  );

  useEffect(() => {
    if (!persist) return;
    let alive = true;
    void api
      .getBoot()
      .then((boot) => {
        if (!alive) return;
        setLocaleState(matchLocale(boot.lang));
        setBootCi(boot.ci);
      })
      .catch(() => {
        /* keep the OS guess */
      });
    return () => {
      alive = false;
    };
  }, [persist]);

  useEffect(() => {
    if (!persist) return;
    let unlisten: (() => void) | undefined;
    void listen<{ lang?: string }>("ui-lang-changed", (event) => {
      if (event.payload?.lang) {
        setLocaleState(matchLocale(event.payload.lang));
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [persist]);

  useEffect(() => {
    if (!bootCi) return;
    void emit("ci-i18n-sample", {
      lang: locale,
      htmlLang: document.documentElement.lang,
      tagline: translate(locale, "manager.tagline"),
      hello: translate(locale, "about.hello"),
      windowTitle: translate(locale, "note.windowTitle"),
      trayNew: translate(locale, "tray.new"),
    });
  }, [bootCi, locale]);

  const setLocale = useCallback(
    async (id: string) => {
      if (!isLocaleId(id)) return;
      const prev = localeRef.current;
      setLocaleState(id);
      if (!persist) return;
      const internals = (window as Window & { __TAURI_INTERNALS__?: unknown })
        .__TAURI_INTERNALS__;
      if (!internals) return;
      try {
        const saved = await api.setLang(id);
        if (isLocaleId(saved)) setLocaleState(saved);
      } catch {
        setLocaleState(prev);
      }
    },
    [persist],
  );

  return (
    <I18nContext.Provider value={{ locale, setLocale, t, bootCi }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    return {
      locale: "en",
      setLocale: async () => {},
      t: (key, vars) => translate("en", key, vars),
      bootCi: false,
    };
  }
  return ctx;
}
