import type { Mod, ModUpdate } from './types';

export const librarySortModes = ['recommended', 'newest', 'oldest', 'nameAsc', 'nameDesc'] as const;
export type LibrarySort = typeof librarySortModes[number];
const storageKey = 'inzoimm.librarySort';

export function initialLibrarySort(): LibrarySort {
 try {
  const saved = localStorage.getItem(storageKey);
  return librarySortModes.includes(saved as LibrarySort) ? saved as LibrarySort : 'recommended';
 } catch { return 'recommended'; }
}

export function saveLibrarySort(mode: LibrarySort) {
 try { localStorage.setItem(storageKey, mode); } catch { /* Keep the session preference. */ }
}

export function sortLibrary(mods: Mod[], mode: LibrarySort, checks: Record<string, ModUpdate | undefined>, locale: string): Mod[] {
 const names = new Intl.Collator(locale, { numeric: true, sensitivity: 'base' });
 const timestamp = (mod: Mod) => Number.isFinite(mod.added_at) && mod.added_at! > 0 ? mod.added_at! : 0;
 return [...mods].sort((a, b) => {
  if (mode === 'nameAsc' || mode === 'nameDesc') return names.compare(a.name, b.name) * (mode === 'nameAsc' ? 1 : -1);
  if (mode === 'recommended') {
   const updates = Number(checks[b.id]?.status === 'available') - Number(checks[a.id]?.status === 'available');
   if (updates) return updates;
  }
  const left = timestamp(a), right = timestamp(b);
  // Unknown dates stay last in either direction. Equal dates keep scan order.
  if (!left || !right) return Number(!left) - Number(!right);
  return mode === 'oldest' ? left - right : right - left;
 });
}
