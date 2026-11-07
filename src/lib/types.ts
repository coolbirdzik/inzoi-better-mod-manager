export type Mod = { id:string; name:string; author:string; version:string; description:string; path:string; thumbnail:string|null; managed:boolean; enabled:boolean; source:string; project:string; library_root?:string|null; metadata?:Record<string,unknown>; tags?:string[]; archive?:boolean; previous_version?:string|null; superseded_by?:string|null; added_at?:number };
export type SourceFolder = {path:string;scan_subfolders:boolean};
export type Settings = { mods_dir:string; library_dir:string; scan_subfolders:boolean; source_dirs?:(SourceFolder|string)[]; downloads_dir?:string; };
export type Snapshot = { settings:Settings; mods:Mod[]; warnings:string[] };

export type ModUpdate={id:string;provider:string;mod_id:number;current_version:string;latest_version:string;status:'available'|'upToDate'|'manual'|'unsupported';url:string;token:string|null;file_id:number};
