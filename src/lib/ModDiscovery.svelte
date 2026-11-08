<script lang="ts">
 import {IconSearch,IconFolder,IconX} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';
 import type {Discovery,DiscoveryProgress} from './types';
 let {drives=[],result,busy,scanning,progress,cancelling=false,error,onauto,onquick,onstop,onchoose,onadd,onclose,onopen,onimport}:{result:Discovery|null;busy:boolean;scanning:boolean;progress:DiscoveryProgress|null;cancelling?:boolean;error:string;drives?:string[];onauto:(drives:string[])=>void;onquick:()=>void;onstop:()=>void;onchoose:()=>void;onadd:(paths:string[])=>void;onclose:()=>void;onopen:(path:string)=>void;onimport:(path:string)=>void}=$props();
 let selected=$state<string[]>([]);
 let chosenDrives=$state<string[]>([]);
 $effect(()=>{chosenDrives=[...drives]});
 function toggleDrive(drive:string){chosenDrives=chosenDrives.includes(drive)?chosenDrives.filter(d=>d!==drive):[...chosenDrives,drive]}
 const recommended=$derived(result?.folders.filter(folder=>folder.origin.startsWith('vortex')&&!folder.already_added).map(folder=>folder.path)??[]);
 $effect(()=>{selected=result?.folders.filter(folder=>!folder.already_added).map(folder=>folder.path)??[]});
</script>

<div class="modal-head"><h2>{t('scanComputer')}</h2><button class="icon" aria-label={t('close')} onclick={onclose}><IconX size={20}/></button></div>
<div class="scan-body">
<p class="muted">{t('discoveryHint')}</p>
{#if drives.length}<div class="drive-picker" role="group" aria-label={t('scanDrivesLabel')}><span class="muted">{t('scanDrivesLabel')}</span>{#each drives as drive (drive)}<button class="drive-chip" aria-pressed={chosenDrives.includes(drive)} disabled={busy} onclick={()=>toggleDrive(drive)}>{drive.replace(/[\\/]+$/,'')}</button>{/each}</div>{/if}
<div class="actions scan-actions"><button disabled={busy||!chosenDrives.length} onclick={()=>onauto(chosenDrives)}><IconSearch size={17}/>{t('scanSelectedDrives',{count:chosenDrives.length})}</button><button disabled={busy} onclick={onquick}>{t('scanQuick')}</button><button disabled={busy} onclick={onchoose}><IconFolder size={17}/>{t('scanChooseFolder')}</button></div>
{#if scanning}<p role="status">{t('discoveringMods')}</p>{#if progress}<div class="scan-progress"><strong>{t('scanProgressCount',{entries:progress.entries,count:progress.mods})}</strong><code title={progress.current_path}>{progress.current_path}</code></div>{/if}<button class="wide" disabled={cancelling} onclick={onstop}>{t(cancelling?'scanStopping':'scanStop')}</button>{/if}
{#if error}<p class="form-error" role="alert">{error}</p>{/if}
{#if result}
 {#if result.cancelled}<p class="muted" role="status">{t('scanCancelled')}</p>{/if}
 {#if result.scanned_entries!==undefined}<p class="muted">{t('scanProgressCount',{entries:result.scanned_entries,count:result.folders.reduce((sum,folder)=>sum+folder.mods.length,0)})}</p>{/if}
 {#if recommended.length&&!scanning}<div class="scan-suggest" role="status"><p>{t('discoverySuggest')}</p><button class="primary" disabled={busy} onclick={()=>onadd(recommended)}>{t('discoverySuggestAdd',{count:recommended.length})}</button></div>{/if}
 <div class="scan-results" aria-label={t('discoveryResults')}>
 {#each result.folders as folder (folder.path)}
  <label class="scan-result"><input type="checkbox" value={folder.path} bind:group={selected} disabled={busy||folder.already_added}/><span><strong>{folder.origin.startsWith('vortex')?'Vortex / Nexus Mods':folder.origin==='game'?t('gameModsFolder'):t('localFiles')} · {t('folderModCount',{count:folder.mods.length})}</strong><code>{folder.path}</code><small>{folder.mods.slice(0,5).join(' · ')}{folder.mods.length>5?' …':''}</small>{#if folder.already_added}<small class="added">{t('discoveryAlreadyAdded')}</small>{:else if folder.origin.startsWith('vortex')}<small class="added">{t('discoveryRecommended')}</small>{/if}</span></label>
 {:else}<p class="muted">{t('discoveryEmpty')}</p>{/each}
 </div>
 {#if result.archives?.length}<section class="scan-archives" aria-label={t('scanNexusArchives')}><h3>{t('scanNexusArchives')} ({result.archives.length})</h3><p class="muted">{t('scanArchiveHint')}</p>{#each result.archives as archive (archive.path)}<div class="archive-row"><div><strong>{archive.name}</strong><code>{archive.path}</code></div><button disabled={busy} title={t('openFolder')} aria-label={t('openFolder')} onclick={()=>onopen(archive.path.replace(/[\\/][^\\/]+$/,''))}><IconFolder size={16}/></button>{#if /\.zip$/i.test(archive.path)}<button disabled={busy} onclick={()=>onimport(archive.path)}>{t('importZip')}</button>{/if}</div>{/each}</section>{/if}
 {#if result.warnings.length}<details class="warnings"><summary>{t('scanWarnings',{count:result.warnings.length})}</summary>{#each result.warnings as warning}<p>{warning}</p>{/each}</details>{/if}
 {#if result.skipped_warnings}<p class="muted">{t('scanMoreWarnings',{count:result.skipped_warnings})}</p>{/if}
 <p class="muted">{t('discoveryVortexHint')}</p>
{/if}
</div>
<div class="actions scan-footer"><button onclick={onclose}>{t('cancel')}</button><button class="primary" disabled={busy||!selected.length} onclick={()=>onadd(selected)}>{t('discoveryAddSelected',{count:selected.length})}</button></div>

<style>
 .scan-suggest{display:flex;gap:12px;align-items:center;justify-content:space-between;border:1px solid var(--accent);border-radius:8px;padding:12px 14px;margin:15px 0 0}.scan-suggest p{margin:0;font-size:12px;line-height:1.6}.scan-suggest button{flex-shrink:0}
 .drive-picker{display:flex;flex-wrap:wrap;gap:7px;align-items:center;margin:15px 0 0}.drive-chip{font-size:12px;min-width:44px}.drive-chip[aria-pressed=true]{border-color:var(--accent);color:var(--accent)}
 .scan-progress{font-size:11px;color:var(--muted);line-height:1.8}.scan-progress code{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.scan-progress strong{font-weight:400}.scan-archives h3{font-size:13px;font-weight:500}.archive-row{display:flex;gap:7px;align-items:center;border-bottom:1px solid var(--line);padding:12px 0}.archive-row>div{min-width:0;flex:1}.archive-row strong{font-size:12px;overflow-wrap:anywhere}.archive-row code{display:block;font-size:10px;color:var(--muted);overflow-wrap:anywhere;margin-top:7px}.archive-row button{flex-shrink:0;font-size:11px}
 :global(.discovery-modal){display:flex;flex-direction:column;overflow:hidden}.modal-head,.scan-footer{flex-shrink:0}.scan-body{overflow:auto;min-height:0;margin-top:10px;padding-right:4px}.scan-footer{padding-top:12px;border-top:1px solid var(--line)}
 .scan-actions{justify-content:flex-start;margin:18px 0}.scan-results{margin:15px 0;border-top:1px solid var(--line)}.scan-result{display:flex;gap:13px;align-items:flex-start;padding:16px 4px;margin:0;border-bottom:1px solid var(--line);cursor:pointer}.scan-result input{width:16px;margin:3px 0;flex-shrink:0;accent-color:var(--accent)}.scan-result>span{min-width:0}.scan-result strong{display:block;font-size:12px;font-weight:500}.scan-result code{display:block;overflow-wrap:anywhere;font-size:11px;color:var(--muted);margin:7px 0}.scan-result small{display:block;color:var(--muted);font-size:11px;line-height:1.7}.scan-result .added{color:var(--accent);margin-top:4px}
</style>
