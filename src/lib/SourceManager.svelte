<script module lang="ts">
 import type {Folder} from './library';
 export type ManagedSource=Folder&{label:string;configurable:boolean;recursive:boolean};
 export type SourceContext={kind:'source';path:string;rootKind:Folder['kind']};
</script>
<script lang="ts">
 import type {Action} from 'svelte/action';
 import {IconFolder,IconDownload,IconSearch,IconArrowUpRight,IconUnlink,IconChevronRight,IconCheck} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';
 let {sources,search='',disabled=false,contextAction,onbrowse,onopen,onremove,onscan}:{sources:ManagedSource[];search?:string;disabled?:boolean;contextAction:Action<HTMLElement,SourceContext>;onbrowse:(path:string)=>void;onopen:(path:string)=>void;onremove:(path:string)=>void;onscan:(path:string,recursive:boolean)=>Promise<boolean>}=$props();
 const filtered=$derived(sources.filter(source=>`${source.label} ${source.path}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())));
</script>

<section class="source-manager" aria-label={t('sourceFolders')}>
 <div class="sources-intro"><div><h2>{t('sourceFolders')}</h2><p>{t('managedCollectionHint')}</p></div></div>
 {#each ['source','app'] as group}
  {@const rows=filtered.filter(source=>group==='source'?source.kind==='source':source.kind!=='source')}
  {#if rows.length||(!search.trim()&&group==='source')}
   <section class="source-group" aria-label={t(group==='source'?'yourSourceFolders':'appSourceFolders')}>
    <div class="source-group-heading"><h3>{t(group==='source'?'yourSourceFolders':'appSourceFolders')}</h3><span>{rows.length}</span></div>
    <div class="source-entries">
    {#each rows as source (source.path)}
     <div class="managed-source-row" data-path={source.path} use:contextAction={{kind:'source',path:source.path,rootKind:source.kind}}>
      <button class="source-folder" title={source.path} onclick={()=>onbrowse(source.path)}><span class="source-symbol" class:downloads={source.kind==='downloads'}>{#if source.kind==='downloads'}<IconDownload size={21} stroke={1.5}/>{:else}<IconFolder size={21} stroke={1.5}/>{/if}</span><span class="source-identity"><strong>{source.label}</strong><small>{source.path}</small></span><span class="source-browse-arrow"><IconChevronRight size={15}/></span></button>
      <span class="source-mod-count">{t('folderModCount',{count:source.count})}</span>
      {#if source.configurable}<label class="source-scan"><input type="checkbox" checked={source.recursive} {disabled} onchange={e=>{const input=e.currentTarget;void onscan(source.path,input.checked).then(saved=>{if(!saved)input.checked=source.recursive})}}/><span class="source-switch" aria-hidden="true"></span><span>{t('scanSubfolders')}</span></label>{:else}<span class="source-automatic"><IconCheck size={14}/>{t('automaticSource')}</span>{/if}
      <div class="source-actions"><button class="icon" {disabled} title={t('openFolder')} aria-label={t('openFolder')} onclick={()=>onopen(source.path)}><IconArrowUpRight size={17}/></button>{#if source.kind==='source'}<button class="icon detach-source" {disabled} title={t('removeSource',{folder:source.name})} aria-label={t('removeSource',{folder:source.name})} onclick={()=>onremove(source.path)}><IconUnlink size={16}/></button>{:else}<span class="source-action-spacer" aria-hidden="true"></span>{/if}</div>
     </div>
    {:else}<div class="source-collection-empty"><IconFolder size={25} stroke={1}/><p>{t('noSourceFoldersHint')}</p></div>{/each}
    </div>
   </section>
  {/if}
 {/each}
 {#if !filtered.length&&search.trim()}<div class="source-search-empty"><IconSearch size={30} stroke={1}/><h3>{t('noSourceMatches')}</h3><p>{t('searchSourcesHint')}</p></div>{/if}
 <p class="sources-footnote">{t('sourceDetachHint')}</p>
</section>

<style>
 .source-manager{width:100%;max-width:1160px;margin:0 auto;padding:25px 0 28px}
 .sources-intro{display:flex;align-items:flex-start;justify-content:space-between;gap:28px;margin-bottom:29px}
 h2{font-size:19px;letter-spacing:-.35px;font-weight:550;margin:0 0 9px}.sources-intro p{font-size:12px;color:var(--muted);line-height:1.8;max-width:430px;margin:0}
 
 .source-group{margin-bottom:28px}.source-group-heading{display:flex;gap:9px;align-items:center;margin-bottom:11px}.source-group-heading h3{font-size:11px;font-weight:500;color:#c6bbc9;margin:0}.source-group-heading>span{font-size:10px;color:#8e8592;font-variant-numeric:tabular-nums}
 .source-entries{background:#ffffff02;border:1px solid var(--line);border-radius:10px;overflow:hidden}.managed-source-row{display:grid;grid-template-columns:minmax(130px,1fr) 74px 155px 68px;align-items:center;gap:14px;padding:16px 17px;border-bottom:1px solid var(--line)}.managed-source-row:last-child{border-bottom:0}.managed-source-row:hover{background:#ffffff03}
 .source-folder{padding:0;gap:13px;min-width:0;text-align:left;justify-content:flex-start;border-radius:5px}.source-folder:hover{background:transparent}.source-symbol{width:40px;height:40px;border-radius:9px;background:#ffffff04;border:1px solid #ffffff08;display:flex;align-items:center;justify-content:center;color:#b9a99f;flex-shrink:0}.source-symbol.downloads{background:#eb795e0a;color:#e6a08b;border-color:#eb795e19}.source-identity{min-width:0;flex:1}.source-identity strong{display:block;font-size:12px;font-weight:500;color:#e0d7e3;margin-bottom:6px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.source-identity small{display:block;font-size:10px;color:#89818f;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.source-browse-arrow{flex-shrink:0;color:#746a7d;opacity:0;transition:opacity .15s}.source-folder:hover .source-browse-arrow,.source-folder:focus-visible .source-browse-arrow{opacity:1}
 .source-mod-count{font-size:11px;color:#a89bae;font-variant-numeric:tabular-nums;text-align:right;white-space:nowrap}.source-scan{display:flex;align-items:center;gap:8px;font-size:10px;color:#b6aaba;cursor:pointer;line-height:1.5;position:relative;min-width:0}.source-scan input{position:absolute;left:0;width:29px;height:17px;margin:0;opacity:0;cursor:pointer}.source-switch{width:29px;height:17px;background:#ffffff10;border:1px solid #ffffff17;border-radius:12px;position:relative;flex-shrink:0;pointer-events:none;transition:background .15s}.source-switch:after{content:'';position:absolute;width:9px;height:9px;background:#b9b1c0;border-radius:50%;left:3px;top:3px;transition:transform .15s}.source-scan input:checked+.source-switch{background:#eb795e;border-color:#eb795e}.source-scan input:checked+.source-switch:after{transform:translateX(12px);background:#241b19}.source-scan input:focus-visible+.source-switch{outline:2px solid var(--accent);outline-offset:3px}.source-scan:has(input:disabled){opacity:.45;cursor:default}.source-automatic{display:flex;align-items:center;gap:7px;font-size:10px;color:#8d8494}.source-automatic :global(svg){color:#a5b5a9}
 .source-actions{display:flex;gap:2px;align-items:center}.source-actions button{color:#8b8094;padding:7px}.source-actions button:hover{color:#ece1ef;background:#ffffff08}.source-actions .detach-source:hover{color:#e6a08b}.source-action-spacer{width:32px;flex-shrink:0}.source-collection-empty{display:flex;align-items:center;gap:15px;padding:25px;color:#7e7287}.source-collection-empty p{margin:0;font-size:12px;line-height:1.8;color:#9e92a6}.sources-footnote{font-size:10px;line-height:1.8;color:#797180;margin:0}.source-search-empty{text-align:center;padding:55px 20px;color:#87768d}.source-search-empty h3{font-size:15px;font-weight:500;color:#d0c2d7}.source-search-empty p{font-size:12px;color:var(--muted)}
 @media(max-width:1100px){.managed-source-row{grid-template-columns:minmax(130px,1fr) 61px 140px 65px;gap:10px;padding:16px 13px}.source-symbol{width:35px;height:35px}.source-folder{gap:10px}.sources-intro{gap:18px}.source-identity small{font-size:9px}}
 @media(prefers-reduced-motion:reduce){.source-switch,.source-switch:after,.source-browse-arrow{transition:none}}
</style>
