/**
 * Specifies the translations for the application for a specific language.
 */
export type Lang = {
  /**
   * the default locale, this will be a fallback if the locale is not found
   * default: {
   *  title: 'Holo Cloud Console',
   * }
   */
  default: Record<string, string>;
  /**
   * The locale in lowercase (gb) followed by the keys and their translation value
   * gb: {
   *  title: 'Holo Cloud Console',
   * }
   */
  [locale: string]: Record<string, string>;
};
