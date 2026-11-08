import type { Mod, Settings, SourceFolder } from './types';
import {downloadFolder} from './downloads';

export const normalizePath=(path:string)=>path.replace(/^\\\\\?\\/,'').replace(/\\/g,'/').replace(/\/+$/,'');
export const pathKey=(path:string)=>{const p=normalizePath(path);return /^(?:[a-z]:|\/\/)/i.test(p)?p.toLowerCase():p};
export const inside=(path:string,root:string)=>pathKey(path)===pathKey(root)||pathKey(path).startsWith(`${pathKey(root)}/`);
export const folderName=(path:string)=>normalizePath(path).split('/').at(-1)||path;
export const modFolder=(mod:Mod)=>mod.archive||mod.legacy?normalizePath(mod.path).split('/').slice(0,-1).join('/'):mod.path;
export const sourceFolders=(settings?:Settings|null):SourceFolder[]=>(settings?.source_dirs??[]).map(source=>typeof source==='string'?{path:source,scan_subfolders:true}:source);
export type Folder={path:string;name:string;parent:string|null;root:string;count:number;kind:'game'|'managed'|'source'|'downloads'};

export function folders(settings:Settings|null|undefined,mods:Mod[]):Folder[]{
 if(!settings)return [];
 const roots:{path:string;kind:Folder['kind']}[]=[{path:settings.mods_dir,kind:'game'},...(settings.paks_dir?[{path:settings.paks_dir,kind:'game' as const}]:[]),{path:settings.library_dir,kind:'managed'},{path:downloadFolder(settings),kind:'downloads'},...sourceFolders(settings).map(source=>({path:source.path,kind:'source' as const}))];
 for(const mod of mods){if(mod.library_root&&!roots.some(root=>inside(mod.library_root!,root.path)))roots.push({path:mod.library_root,kind:'managed'})}
 const result=new Map<string,Folder>();
 for(const root of roots){const path=normalizePath(root.path);if(!result.has(pathKey(path)))result.set(pathKey(path),{...root,path,name:folderName(path),parent:null,root:path,count:0})}
 for(const mod of mods){
  const root=[...result.values()].filter(f=>f.parent===null&&inside(mod.path,f.path)).sort((a,b)=>b.path.length-a.path.length)[0];
  if(!root)continue;
  const parent=normalizePath(mod.path).split('/').slice(0,-1).join('/');
  if(!inside(parent,root.path))continue;
  let current=root.path;
  for(const part of parent.slice(root.path.length).split('/').filter(Boolean)){
   const path=`${current}/${part}`;
   if(!result.has(pathKey(path)))result.set(pathKey(path),{path,name:part,parent:current,root:root.path,count:0,kind:root.kind});
   current=path;
  }
 }
 for(const folder of result.values())folder.count=mods.filter(m=>inside(m.path,folder.path)).length;
 return [...result.values()];
}

export type MetadataField={key:string;value:string};
export function metadataFields(metadata:Record<string,unknown>={}):MetadataField[]{
 const result:MetadataField[]=[];
 function visit(value:unknown,key:string){
  if(value&&typeof value==='object'&&Object.keys(value).length){for(const [child,v] of Object.entries(value))visit(v,`${key}.${child}`)}
  else result.push({key,value:typeof value==='string'?value:JSON.stringify(value)??''});
 }
 for(const [key,value] of Object.entries(metadata))visit(value,key);
 return result;
}
export function webLinks(mod:Mod):string[]{
 const links=new Set<string>();
 for(const field of [{key:'source',value:mod.source},{key:'description',value:mod.description},...metadataFields(mod.metadata)]){
  for(const raw of field.value.match(/https?:\/\/[^\s<>"']+/gi)??[]){try{const url=new URL(raw.replace(/[),.;\]}]+$/,''));if(['http:','https:'].includes(url.protocol)&&!url.username&&!url.password)links.add(url.href)}catch{/* Not a complete URL. */}}
  for(const match of field.value.matchAll(/nxm:\/\/inzoi\/mods\/(\d+)\/files\/(\d+)/gi))links.add(`https://www.nexusmods.com/inzoi/mods/${match[1]}?tab=files&file_id=${match[2]}`);
 }
 // Common Nexus/MO2 sidecars carry an ID rather than a website link.
 const fields=metadataFields(mod.metadata);
 const id=fields.find(f=>(/^meta\.ini\./i.test(f.key)||/^nexus\.json\./i.test(f.key)||/nexusmodid$/i.test(f.key))&&/(?:^|\.)(?:nexusmodid|modid)$/i.test(f.key)&&/^\d+$/.test(f.value));
 const game=fields.find(f=>/(?:^|\.)(?:gamename|gamedomain)$/i.test(f.key));
 if(id&&(!game||/^inzoi$/i.test(game.value)))links.add(`https://www.nexusmods.com/inzoi/mods/${id.value}`);
 return [...links];
}


export function modOrigin(mod:Mod):{kind:'nexus'|'curseforge'|'web'|'local';label:string;url?:string}{
 const fields=metadataFields(mod.metadata);
 const urlOrigin=(raw:string)=>{try{const url=new URL(raw);if(!['https:','http:'].includes(url.protocol)||url.username||url.password)return;const host=url.hostname.toLowerCase();if(host==='nexusmods.com'||host.endsWith('.nexusmods.com'))return{kind:'nexus' as const,label:'Nexus Mods',url:url.href};if(host==='curseforge.com'||host.endsWith('.curseforge.com'))return{kind:'curseforge' as const,label:'CurseForge',url:url.href};return{kind:'web' as const,label:host.replace(/^www\./,''),url:url.href}}catch{return}};
 // Saved download provenance wins over links merely mentioned by a mod author.
 for(const provider of ['nexus','curseforge'] as const){
  if(fields.some(f=>f.key.startsWith(`${provider}.json.Download.`))){const link=fields.filter(f=>f.key.startsWith(`${provider}.json.Download.`)).map(f=>urlOrigin(f.value)).find(origin=>origin?.kind===provider);return{kind:provider,label:provider==='nexus'?'Nexus Mods':'CurseForge',url:link?.url}}
 }
 const source=urlOrigin(mod.source);if(source)return source;
 const links=fields.filter(f=>/url$|website|homepage/i.test(f.key)).map(f=>urlOrigin(f.value)).filter(origin=>!!origin);
 const known=links.find(origin=>origin.kind==='nexus'||origin.kind==='curseforge');if(known)return known;
 if(fields.some(f=>f.key.startsWith('curseforge.json.')||f.key==='Game library.gameId'&&f.value==='88849'))return{kind:'curseforge',label:'CurseForge'};
 if(fields.some(f=>/^meta\.ini\..*(?:modid|nexusmodid)$/i.test(f.key)&&/^\d+$/.test(f.value)))return{kind:'nexus',label:'Nexus Mods'};
 return links[0]??{kind:'local',label:''};
}
