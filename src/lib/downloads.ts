import type { Settings } from './types';
export type NexusInfo={request_id:string;mod_id:number;file_id:number;url:string};
export type DownloadResult={path:string;bytes:number;can_import:boolean};
export type DownloadStatus='queued'|'downloading'|'cancelling'|'importing'|'completed'|'failed'|'importFailed'|'cancelled'|'needsExtraction';
export type DownloadJob={info:NexusInfo;status:DownloadStatus;received:number;total:number|null;result?:DownloadResult;error?:string};
export const downloadFolder=(settings?:Settings|null)=>settings?.downloads_dir||`${settings?.library_dir??''}/Downloads`;
export const pendingDownload=(job:DownloadJob)=>['queued','downloading','cancelling','importing'].includes(job.status);
