<script lang="ts">
 import {IconDownload,IconFolder,IconX,IconPlayerPause,IconPlayerPlay,IconRefresh,IconTrash,IconArrowUpRight} from '@tabler/icons-svelte';
 import {t,type TranslationKey} from './i18n.svelte';
 import {pendingDownload,type DownloadJob} from './downloads';
 let {jobs,paused,path,onclose,onpause,oncancel,onretry,onclear,onopen,onlink,onfolder,onsettings}:{jobs:DownloadJob[];paused:boolean;path:string;onclose:()=>void;onpause:()=>void;oncancel:(job:DownloadJob)=>void;onretry:(job:DownloadJob)=>void;onclear:()=>void;onopen:(path:string)=>void;onlink:(url:string)=>void;onfolder:()=>void;onsettings:()=>void}=$props();
 let link=$state('');
 const statusKey:Record<DownloadJob['status'],TranslationKey>={queued:'downloadQueued',downloading:'downloadRunning',cancelling:'downloadCancelling',importing:'downloadImporting',completed:'downloadReady',failed:'downloadFailed',importFailed:'downloadImportFailed',cancelled:'downloadCancelled',needsExtraction:'downloadNeedsExtraction'};
 const mb=(bytes:number)=>(bytes/1048576).toFixed(1);
</script>
<div class="downloads-head"><div><h2>{t('downloads')}</h2><p>{t('downloadQueueHint')}</p></div><button class="icon" aria-label={t('close')} onclick={onclose}><IconX size={20}/></button></div>
<div class="download-destination"><IconFolder size={18}/><div><span>{t('downloadFolder')}</span><p title={path}>{path}</p></div><button onclick={onfolder}>{t('change')}</button></div>
<form class="download-link-form" onsubmit={e=>{e.preventDefault();if(link.trim()){onlink(link.trim());link=''}}}><input aria-label={t('link')} placeholder="nxm://inzoi/…" bind:value={link}/><button disabled={!link.trim()}>{t('addDownload')}</button></form>
<div class="queue-toolbar"><button onclick={onpause}>{#if paused}<IconPlayerPlay size={15}/>{t('resumeQueue')}{:else}<IconPlayerPause size={15}/>{t('pauseQueue')}{/if}</button><button disabled={!jobs.some(j=>!pendingDownload(j))} onclick={onclear}><IconTrash size={15}/>{t('clearFinished')}</button></div>
{#if paused}<p class="queue-note" role="status">{t('queuePausedHint')}</p>{/if}
<div class="download-jobs" aria-label={t('downloadQueue')}>
 {#each jobs as job (job.info.request_id)}
  <article class="download-job" aria-label={t('linkInfo',{mod:job.info.mod_id,file:job.info.file_id})}>
   <div class="download-job-title"><IconDownload size={18}/><strong>{job.result?job.result.path.split(/[\\/]/).at(-1):t('linkInfo',{mod:job.info.mod_id,file:job.info.file_id})}</strong><button class="icon" aria-label={t('openOnNexus')} title={t('openOnNexus')} onclick={()=>onopen(job.info.url)}><IconArrowUpRight size={15}/></button></div>
   <div class="download-job-status"><span>{t(statusKey[job.status])}</span>{#if job.received>0}<span>{mb(job.received)} MB{#if job.total} / {mb(job.total)} MB{/if}</span>{/if}</div>
   {#if job.status==='downloading'}<progress max={job.total??undefined} value={job.total?job.received:undefined} aria-label={t('downloadProgress')}></progress>{/if}
   {#if job.error}<p class="form-error">{job.error==='nexusApiKeyRequired'?t('nexusApiKeyRequired'):job.error}</p>{/if}
   {#if job.status==='needsExtraction'}<p class="muted">{t('archiveExtractionHint')}</p>{/if}
   {#if job.result}<p class="download-job-path" title={job.result.path}>{job.result.path}</p>{/if}
   <div class="download-job-actions">
    {#if job.status==='queued'||job.status==='downloading'}<button onclick={()=>oncancel(job)}>{t('cancel')}</button>{/if}
    {#if ['failed','cancelled','importFailed'].includes(job.status)}<button onclick={()=>onretry(job)}><IconRefresh size={14}/>{t(job.result?'retryImport':'retryDownload')}</button>{/if}
    {#if job.error==='nexusApiKeyRequired'}<button onclick={onsettings}>{t('settings')}</button>{/if}
    {#if job.result}<button onclick={()=>onopen(job.result!.path.replace(/[\\/][^\\/]+$/,''))}><IconFolder size={14}/>{t('openFolder')}</button>{/if}
   </div>
  </article>
 {:else}<div class="downloads-empty"><IconDownload size={34} stroke={1}/><h3>{t('noDownloads')}</h3><p>{t('noDownloadsHint')}</p></div>{/each}
</div>
