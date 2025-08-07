import * as Langs from "./langs";
import { writable, get } from "svelte/store";

const LS_LANGUAGE_KEY = "language";
const currentLanguage = writable<string>(getDefaultLanguage());

function getDefaultLanguage() {
  const storedLanguage = localStorage.getItem(LS_LANGUAGE_KEY);
  if (storedLanguage) {
    return storedLanguage;
  }
  return navigator.language;
}

export function setLanguage(language: string) {
  localStorage.setItem(LS_LANGUAGE_KEY, language);
  currentLanguage.set(language);
  location.reload();
}

export function getLanguage() {
  return get(currentLanguage);
}

export function getTranslation(key: string): string {
  return getTranslationWithLang(get(currentLanguage), key);
}

export function getTranslationWithLang(language: string, key: string): string {
  const [currentLanguage, currentLocale] = language.split("-");
  if (currentLanguage in Langs) {
    const lang = Langs[currentLanguage as keyof typeof Langs];
    if (lang) {
      if (currentLocale in lang) {
        if (key in lang[currentLocale]) {
          return lang[currentLocale][key];
        }
      }
      if (key in lang.default) {
        return lang.default[key];
      }
    }
  }
  if (Langs.en.default && key in Langs.en.default) {
    return Langs.en.default[key];
  }
  return key;
}

export function getSupportedLanguages(): string[] {
  return Object.keys(Langs).flatMap((key) =>
    Object.keys(Langs[key as keyof typeof Langs]).map((locale) => {
      if (locale === "default") {
        return key;
      }
      return `${key}-${locale}`;
    })
  );
}
