import type { Mod } from './types';

// Canonical groups shared by every source, so a CurseForge "Outfits" mod and a
// local mod whose manifest patches garments land in the same section.
export const categoryKeys = ['outfits', 'hairstyle', 'faceMakeup', 'accessories', 'gameplay', 'other'] as const;
export type CategoryKey = typeof categoryKeys[number];
/** `key` for a known group, otherwise the provider's own category name. */
export type ModCategory = { id: string; key?: CategoryKey; name?: string };

const providerNames: Record<string, CategoryKey> = {
 outfits: 'outfits', outfit: 'outfits', clothing: 'outfits', clothes: 'outfits',
 hairstyle: 'hairstyle', hairstyles: 'hairstyle', hair: 'hairstyle',
 'face & makeup': 'faceMakeup', makeup: 'faceMakeup', 'face and makeup': 'faceMakeup',
 accessories: 'accessories', accessory: 'accessories',
 gameplay: 'gameplay', others: 'other', other: 'other', miscellaneous: 'other',
};

// inZOI ModKit manifests list the game tables each mod patches. Order matters:
// a hairstyle pack that also ships an outfit is still a hairstyle pack.
const manifestRules: [RegExp, CategoryKey][] = [
 [/^appearancehair/i, 'hairstyle'],
 [/^garment/i, 'outfits'],
 [/accessor/i, 'accessories'],
 [/^appearance/i, 'faceMakeup'],
 [/\.lua$|^\.luarc$|^lua_manifest|interaction|^buff|^condition|^action/i, 'gameplay'],
];

const record = (value: unknown): Record<string, unknown> | undefined =>
 value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
/** Sources disagree on casing (`iD`, `id`, `NonAssets`, `nonAssets`). */
function field(value: unknown, key: string): unknown {
 const object = record(value);
 if (!object) return undefined;
 if (key in object) return object[key];
 const lower = key.toLowerCase();
 const match = Object.keys(object).find(name => name.toLowerCase() === lower);
 return match === undefined ? undefined : object[match];
}

function providerCategory(details: unknown): string | undefined {
 const list = field(details, 'categories');
 if (!Array.isArray(list)) return;
 const named = list.filter(entry => typeof field(entry, 'name') === 'string' && String(field(entry, 'name')).trim());
 const primary = field(details, 'primaryCategoryId');
 const chosen = named.find(entry => primary != null && String(field(entry, 'id')) === String(primary)) ?? named[0];
 return chosen ? String(field(chosen, 'name')).trim() : undefined;
}

function manifestCategory(manifest: unknown): CategoryKey | undefined {
 const entries = field(manifest, 'NonAssets');
 if (!Array.isArray(entries)) return;
 const names = entries.filter((entry): entry is string => typeof entry === 'string')
  .map(entry => (entry.split(/[\\/]/).at(-1) ?? '').replace(/_patch.*$/i, '').replace(/\.json$/i, ''));
 for (const [pattern, key] of manifestRules) if (names.some(name => pattern.test(name))) return key;
}

export function modCategory(mod: Mod): ModCategory {
 const metadata = mod.metadata ?? {};
 const curseforge = field(metadata, 'curseforge.json');
 let fallback: ModCategory | undefined;
 for (const details of [field(metadata, 'Game library'), field(field(curseforge, 'Download'), 'mod'), curseforge]) {
  const name = providerCategory(details);
  if (!name) continue;
  const key = providerNames[name.toLowerCase()];
  // A provider's catch-all "Others" is less useful than what the manifest says.
  if (key === 'other') { fallback ??= { id: 'other', key }; continue; }
  return key ? { id: key, key } : { id: `name:${name.toLowerCase()}`, name };
 }
 const key = manifestCategory(field(metadata, 'mod_manifest.json'));
 if (key) return { id: key, key };
 return fallback ?? { id: 'other', key: 'other' };
}

export type ModGroup = { category: ModCategory; mods: Mod[] };

/** Groups keep the caller's sort inside each group; largest groups first, "Other" last. */
export function groupMods(mods: Mod[], categoryOf: (mod: Mod) => ModCategory, label: (category: ModCategory) => string, locale: string): ModGroup[] {
 const groups = new Map<string, ModGroup>();
 for (const mod of mods) {
  const category = categoryOf(mod);
  const group = groups.get(category.id);
  if (group) group.mods.push(mod); else groups.set(category.id, { category, mods: [mod] });
 }
 const names = new Intl.Collator(locale, { sensitivity: 'base' });
 return [...groups.values()].sort((a, b) =>
  Number(a.category.id === 'other') - Number(b.category.id === 'other')
  || b.mods.length - a.mods.length
  || names.compare(label(a.category), label(b.category)));
}
