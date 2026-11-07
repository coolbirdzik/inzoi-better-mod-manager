import type { Settings } from './types';
import {t} from './i18n.svelte';
export type NexusInfo={provider?:'nexus'|'curseforge';slug?:string|null;request_id:string;mod_id:number;file_id:number;url:string};
export type DownloadResult={path:string;bytes:number;can_import:boolean};
export type DownloadStatus='queued'|'downloading'|'cancelling'|'importing'|'completed'|'failed'|'importFailed'|'cancelled'|'needsExtraction';
export type DownloadJob={info:NexusInfo;status:DownloadStatus;received:number;total:number|null;result?:DownloadResult;error?:string;update?:{id:string;token:string;name:string}};
export const downloadFolder=(settings?:Settings|null)=>{
 const path=settings?.downloads_dir||`${(settings?.library_dir??'').replace(/[\\/]+$/,'')}/Downloads`;
 return /^(?:[a-z]:[\\/]|[\\/]{2})/i.test(path)?path.replace(/\//g,'\\'):path;
};
export const pendingDownload=(job:DownloadJob)=>['queued','downloading','cancelling','importing'].includes(job.status);

export const downloadProvider=(info:NexusInfo)=>info.provider==='curseforge'?'CurseForge':'Nexus Mods';
export const downloadLabel=(info:NexusInfo)=>info.provider==='curseforge'?`CurseForge · ${info.slug||`#${info.mod_id}`} · ${info.file_id?`#${info.file_id}`:t('latestRelease')}`:t('linkInfo',{mod:info.mod_id,file:info.file_id});
