import { resolveLocale as resolve, translate } from './i18n-core.mjs';
import en from './locales/en.json' with { type: 'json' };
import vi from './locales/vi.json' with { type: 'json' };
import zh from './locales/zh-CN.json' with { type: 'json' };
import ko from './locales/ko.json' with { type: 'json' };
import ja from './locales/ja.json' with { type: 'json' };
import fr from './locales/fr.json' with { type: 'json' };
import de from './locales/de.json' with { type: 'json' };
import es from './locales/es.json' with { type: 'json' };
import pt from './locales/pt-BR.json' with { type: 'json' };
import ru from './locales/ru.json' with { type: 'json' };
export type TranslationKey = keyof typeof en;
export const catalogs = { en, vi, 'zh-CN':zh, ko, ja, fr, de, es, 'pt-BR':pt, ru };
export type Locale = keyof typeof catalogs;
export const languages: {code:Locale;name:string}[] = [
 {code:'en',name:'English'}, {code:'vi',name:'Tiếng Việt'}, {code:'zh-CN',name:'简体中文'},
 {code:'ko',name:'한국어'}, {code:'ja',name:'日本語'}, {code:'fr',name:'Français'},
 {code:'de',name:'Deutsch'}, {code:'es',name:'Español'}, {code:'pt-BR',name:'Português (Brasil)'}, {code:'ru',name:'Русский'}
];
const storageKey='inzoimm.language';
export function resolveLocale(value:string|null):Locale {return resolve(value) as Locale;}
function initialLocale():Locale {try{return resolveLocale(localStorage.getItem(storageKey))}catch{return 'en'}}
export const language=$state({code:initialLocale()});
export function setLanguage(code:string){language.code=resolveLocale(code);try{localStorage.setItem(storageKey,language.code)}catch{/* Keep the session preference when storage is unavailable. */}if(typeof document!=='undefined')document.documentElement.lang=language.code;}
export function t(key:TranslationKey,params:Record<string,string|number>={}){return translate(catalogs[language.code],key,params);}
// Backend errors are either plain text, a bare translation key, or `gameFilesLocked:<mod name>`.
const lockedPrefix='gameFilesLocked:';
/** Backend error prefix: the archive holds the inZOI Mod Enabler and needs confirmation. */
export const enablerPrefix='modEnablerConfirm:';
/** Backend error prefix: the archive has nothing the app can install. */
export const nothingInstallablePrefix='noInstallableMod:';
export function errorText(raw:string,keys:readonly string[]=[]){const text=raw.replace(/^Error: /,'');if(text.startsWith(lockedPrefix))return t('gameFilesLocked',{name:text.slice(lockedPrefix.length)});if(text.startsWith(enablerPrefix))return text.slice(enablerPrefix.length);if(text.startsWith(nothingInstallablePrefix))return text.slice(nothingInstallablePrefix.length);return keys.includes(text)?t(text as TranslationKey):text;}
if(typeof document!=='undefined')document.documentElement.lang=language.code;
