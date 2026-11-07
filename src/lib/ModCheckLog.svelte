<script lang="ts">
 import {t,type TranslationKey} from './i18n.svelte';
 import {IconX,IconCopy,IconRefresh,IconSettings} from '@tabler/icons-svelte';
 import type {CheckLog} from './check-log';
 let {entries,path,loading,error,copied,copyFallback,onclose,onrefresh,oncopy,onsettings,oninspect,caninspect}:{entries:CheckLog[];path:string;loading:boolean;error:string;copied:boolean;copyFallback:string;onclose:()=>void;onrefresh:()=>void;oncopy:(entries:CheckLog[])=>void;onsettings:()=>void;oninspect:(entry:CheckLog)=>void;caninspect:(entry:CheckLog)=>boolean}=$props();
 let errorsOnly=$state(true),latestOnly=$state(true),query=$state('');
 const latestRun=$derived([...entries].sort((a,b)=>b.timestamp-a.timestamp)[0]?.run_id);
 const scoped=$derived(entries.filter(entry=>(!latestOnly||entry.run_id===latestRun)&&(!query.trim()||`${entry.mod_name} ${entry.project} ${entry.provider} ${entry.detail} ${entry.code}`.toLowerCase().includes(query.trim().toLowerCase()))));
 const filtered=$derived(scoped.filter(entry=>!errorsOnly||entry.status==='failed').sort((a,b)=>b.timestamp-a.timestamp));
 const groups=$derived.by(()=>{
  const result=new Map<string,{provider:string;code:string;count:number}>();
  for(const entry of scoped.filter(entry=>entry.status==='failed')){const key=`${entry.provider}:${entry.code}`;const group=result.get(key)??{provider:entry.provider,code:entry.code,count:0};group.count++;result.set(key,group)}
  return [...result.values()];
 });
 const reasonKeys:Record<string,TranslationKey>={missing_key:'checkLogMissingKey',http_401:'checkLogUnauthorized',http_403:'checkLogForbidden',http_404:'checkLogNotFound',http_429:'checkLogRateLimit',timeout:'checkLogTimeout',network:'checkLogNetwork',backend_mismatch:'checkLogBackendMismatch',missing_mod:'checkLogMissingMod'};
 function reason(code:string){return t(reasonKeys[code]??'checkLogOther')}
 function status(entry:CheckLog){return t(({failed:'checkLogFailed',available:'modUpdateAvailable',upToDate:'modUpToDate',manual:'reviewModFiles',unsupported:'modUpdateUnsupported',checking:'checkingModUpdates'} as Record<string,TranslationKey>)[entry.status]??'checkLogOther')}
 const provider=(value:string)=>value==='nexus'?'Nexus Mods':value==='curseforge'?'CurseForge':t('localFiles');
 function selectText(node:HTMLTextAreaElement){queueMicrotask(()=>{node.focus();node.select()})}
</script>
<div class="check-log-head"><div><h2>{t('modCheckLog')}</h2><p>{t('checkLogHint')}</p></div><button class="icon" aria-label={t('close')} onclick={onclose}><IconX size={20}/></button></div>
<div class="check-log-toolbar"><button disabled={loading} onclick={onrefresh}><IconRefresh size={15} class={loading?'spin':''}/>{t('refreshCheckLog')}</button><button disabled={!filtered.length} onclick={()=>oncopy(filtered)}><IconCopy size={15}/>{t(copied?'checkLogCopied':'copyCheckLog')}</button><button onclick={onsettings}><IconSettings size={15}/>{t('settings')}</button></div>
<div class="check-log-filters"><input type="search" bind:value={query} aria-label={t('searchCheckLog')} placeholder={t('searchCheckLog')}/><label><input type="checkbox" bind:checked={errorsOnly}/>{t('checkLogErrorsOnly')}</label><label><input type="checkbox" bind:checked={latestOnly}/>{t('checkLogLatestOnly')}</label></div>
{#if error}<p class="form-error" role="alert">{error}</p>{/if}
{#if groups.length}<div class="check-log-causes" aria-label={t('checkLogCauses')}>{#each groups as group}<p><strong>{provider(group.provider)} · {group.count}</strong><span>{reason(group.code)}</span></p>{/each}</div>{/if}
{#if copyFallback}<div class="check-log-fallback"><p class="muted">{t('checkLogCopyFallback')}</p><textarea readonly value={copyFallback} aria-label={t('copyCheckLog')} use:selectText></textarea></div>{/if}
<div class="check-log-entries" role="list" aria-label={t('modCheckLog')}>
 {#each filtered as entry (entry.id)}<article role="listitem" class:failed={entry.status==='failed'}><div class="check-log-entry-title"><strong>{entry.mod_name||entry.project}</strong><span>{provider(entry.provider)}</span><time datetime={new Date(entry.timestamp).toISOString()}>{new Date(entry.timestamp).toLocaleString()}</time></div><div class="check-log-entry-status"><span>{entry.status==='failed'?reason(entry.code):status(entry)}</span><small>{entry.duration_ms} ms</small>{#if caninspect(entry)}<button onclick={()=>oninspect(entry)}>{t('checkLogInspectMod')}</button>{/if}</div><pre>{entry.detail}</pre></article>{:else}<p class="check-log-empty">{t(entries.length?'checkLogNoMatches':'checkLogEmpty')}</p>{/each}
</div>
{#if path}<p class="check-log-path" title={path}>{path}</p>{/if}
