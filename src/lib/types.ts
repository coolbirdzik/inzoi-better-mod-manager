export type Mod = { id:string; name:string; author:string; version:string; description:string; path:string; thumbnail:string|null; managed:boolean; enabled:boolean; source:string; project:string };
export type Settings = { mods_dir:string; library_dir:string; };
export type Snapshot = { settings:Settings; mods:Mod[]; warnings:string[] };