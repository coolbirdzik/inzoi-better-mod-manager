export type Mod = { id:string; name:string; author:string; version:string; description:string; path:string; thumbnail:string|null; managed:boolean; enabled:boolean; source:string; project:string; library_root?:string|null; metadata?:Record<string,unknown>; tags?:string[]; archive?:boolean; legacy?:boolean; previous_version?:string|null; superseded_by?:string|null; added_at?:number };
export type SourceFolder = {path:string;scan_subfolders:boolean};
export type Settings = { mods_dir:string; library_dir:string; scan_subfolders:boolean; source_dirs?:(SourceFolder|string)[]; downloads_dir?:string; paks_dir?:string|null; };
export type Snapshot = { settings:Settings; mods:Mod[]; warnings:string[] };
export type Discovery = {folders:{path:string;origin:'vortex'|'vortex-downloads'|'game'|'local';mods:string[];already_added:boolean}[];warnings:string[];archives?:{path:string;name:string}[];cancelled?:boolean;scanned_entries?:number;skipped_warnings?:number};
export type DiscoveryProgress = {current_path:string;entries:number;mods:number;archives:number};

export type ModUpdate={id:string;provider:string;mod_id:number;current_version:string;latest_version:string;status:'available'|'upToDate'|'manual'|'unsupported';url:string;token:string|null;file_id:number};
