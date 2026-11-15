import de from "../locales/de.json";
import en from "../locales/en.json";
import es from "../locales/es.json";
import fr from "../locales/fr.json";
import it from "../locales/it.json";
import ja from "../locales/ja.json";
import ko from "../locales/ko.json";
import pl from "../locales/pl.json";
import ptBR from "../locales/pt-BR.json";
import ru from "../locales/ru.json";
import zhCN from "../locales/zh-CN.json";
import type { LocaleId } from "./locales";

export type Catalog = Record<string, string>;

export const catalogs: Record<LocaleId, Catalog> = {
  en: en as Catalog,
  es: es as Catalog,
  fr: fr as Catalog,
  de: de as Catalog,
  "pt-BR": ptBR as Catalog,
  it: it as Catalog,
  ru: ru as Catalog,
  pl: pl as Catalog,
  ja: ja as Catalog,
  ko: ko as Catalog,
  "zh-CN": zhCN as Catalog,
};
