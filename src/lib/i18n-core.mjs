import en from './locales/en.json' with { type: 'json' };
export const supportedLocales = ['en','vi','zh-CN','ko','ja','fr','de','es','pt-BR','ru'];
export function resolveLocale(value) { return supportedLocales.includes(value) ? value : 'en'; }
export function interpolate(text, params = {}) { return text.replace(/\{(\w+)\}/g, (match, name) => params[name] === undefined ? match : String(params[name])); }
export function translate(catalog, key, params = {}) { return interpolate(catalog[key] ?? en[key] ?? key, params); }
