// UI language. English source strings are the keys; a table per language maps
// them to translations, so `tr("New host")` reads naturally in the code and
// falls back to English for anything untranslated.

import { createElement, Fragment, useSyncExternalStore, type ReactNode } from "react";
import { ru } from "./ru";

/** `system` follows the OS / webview locale. */
export type Language = "system" | "en" | "ru";
export type Locale = "en" | "ru";

export const LANGUAGES: readonly Language[] = ["system", "en", "ru"];

/** Names shown in the language picker, each in its own language. */
export const LOCALE_NAMES: Record<Locale, string> = { en: "English", ru: "Русский" };

const STORAGE_KEY = "termoso.language";

const TABLES: Record<Locale, Readonly<Record<string, string>> | undefined> = {
  en: undefined,
  ru,
};

export type Vars = Record<string, string | number>;

function isLanguage(v: unknown): v is Language {
  return typeof v === "string" && (LANGUAGES as readonly string[]).includes(v);
}

function load(): Language {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return isLanguage(v) ? v : "system";
  } catch {
    return "system";
  }
}

export function systemLocale(): Locale {
  const tags =
    typeof navigator === "undefined"
      ? []
      : navigator.languages.length
        ? navigator.languages
        : [navigator.language];
  for (const tag of tags) {
    if (tag.toLowerCase().startsWith("ru")) return "ru";
  }
  return "en";
}

let setting: Language = load();
let locale: Locale = setting === "system" ? systemLocale() : setting;
let table = TABLES[locale];
let plurals = new Intl.PluralRules(locale);
const listeners = new Set<() => void>();

function apply() {
  locale = setting === "system" ? systemLocale() : setting;
  table = TABLES[locale];
  plurals = new Intl.PluralRules(locale);
  if (typeof document !== "undefined") document.documentElement.lang = locale;
}
apply();

/** The stored preference (`system` | `en` | `ru`). */
export function language(): Language {
  return setting;
}

/** The locale actually in use. */
export function activeLocale(): Locale {
  return locale;
}

export function setLanguage(next: Language) {
  if (next === setting) return;
  setting = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // private mode / storage disabled: the choice lives for this run only
  }
  apply();
  for (const l of listeners) l();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Runs `listener` after every language change; for text living outside React (tray menu). */
export const onLanguageChange = subscribe;

/** Re-renders the caller when the language changes; the root uses it so the
 *  whole tree picks up new strings. */
export function useLanguage(): Language {
  return useSyncExternalStore(subscribe, language, language);
}

function interpolate(s: string, vars: Vars | undefined): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

/** Marks a module-level English string for extraction; translate it at render time with `tr()`. */
export const msg = (text: string): string => text;

/** Translate `text` (an English source string); `{name}` placeholders are
 *  filled from `vars`. */
export function tr(text: string, vars?: Vars): string {
  return interpolate(table?.[text] ?? text, vars);
}

/** Like `tr`, but placeholders may be React elements (e.g. `<b>{name}</b>`);
 *  returns the pieces to render as children. */
export function trx(text: string, vars: Record<string, ReactNode>): ReactNode[] {
  const s = table?.[text] ?? text;
  const out: ReactNode[] = [];
  const re = /\{(\w+)\}/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(s)) !== null) {
    if (m.index > last) out.push(s.slice(last, m.index));
    const k = m[1] ?? "";
    out.push(k in vars ? createElement(Fragment, { key: m.index }, vars[k]) : m[0]);
    last = m.index + m[0].length;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

/** Pluralised message. `one` / `other` are the English forms with `{count}`;
 *  `other` is the lookup key, and a translation lists its forms separated by
 *  `|` in CLDR order (ru: one|few|many). */
export function trn(count: number, one: string, other: string, vars?: Vars): string {
  const all = { count, ...vars };
  const translated = table?.[other];
  if (translated === undefined) return interpolate(count === 1 ? one : other, all);
  const forms = translated.split("|");
  const order = locale === "ru" ? ["one", "few", "many"] : ["one", "other"];
  const i = order.indexOf(plurals.select(count));
  const form = forms[i === -1 ? forms.length - 1 : Math.min(i, forms.length - 1)] ?? other;
  return interpolate(form, all);
}
