<script lang="ts">
 import { language, languages, setLanguage, t, type TranslationKey } from './lib/i18n.svelte';
 import { onMount } from 'svelte';
 import { invoke, isTauri } from '@tauri-apps/api/core';
 import { listen } from '@tauri-apps/api/event';
 import { open, confirm } from '@tauri-apps/plugin-dialog';
 import { openPath, openUrl } from '@tauri-apps/plugin-opener';
 import { check, type Update } from '@tauri-apps/plugin-updater';
 import { relaunch } from '@tauri-apps/plugin-process';
 import { getCurrent } from '@tauri-apps/plugin-deep-link';
 import { IconSearch, IconPlus, IconRefresh, IconSettings, IconBox, IconFolder, IconArrowUpRight, IconDownload, IconTrash, IconPencil, IconCommand, IconCheck, IconX, IconChevronRight, IconLayoutGrid, IconList, IconPhoto, IconArrowLeft, IconArrowRight } from '@tabler/icons-svelte';
 import ModThumbnail from './lib/ModThumbnail.svelte';
 import NexusApiKey from './lib/NexusApiKey.svelte';
 import Downloads from './lib/Downloads.svelte';
 import {downloadFolder,pendingDownload,type NexusInfo,type DownloadJob,type DownloadResult} from './lib/downloads';
 import ModMetadata from './lib/ModMetadata.svelte';
 import SourceManager, {type ManagedSource} from './lib/SourceManager.svelte';
 import ContextMenu, {type MenuItem} from './lib/ContextMenu.svelte';
 import { folders, inside, pathKey, normalizePath, folderName, sourceFolders, webLinks, type Folder } from './lib/library';
 import type { Mod, Snapshot, Settings } from './lib/types';
 let data=$state<Snapshot|null>(null), query=$state(''), filter=$state('all'), selected=$state(''), busy=$state(false), error=$state(''), message=$state('');
 let settingsOpen=$state(false), draft=$state<Settings|null>(null), editOpen=$state(false), editName=$state(''), editDescription=$state('');
 let importOpen=$state(false), importPath=$state(''), importRecursive=$state(true), addingSource=$state(false), sourcePaths=$state<string[]>([]);
 let linkOpen=$state(false), nexus=$state<NexusInfo|null>(null), update=$state<Update|null>(null);
 let waitingLinks=$state<NexusInfo[]>([]), jobs=$state<DownloadJob[]>([]), downloadsOpen=$state(false), queuePaused=$state(false), processing=$state(false);
 let nexusKey=$state(''), promptKey=$state(false), updating=$state(false);
 const pendingCount=$derived(jobs.filter(pendingDownload).length);
 function activateLink(info:NexusInfo){nexus=info;promptKey=!nexusKey.trim();downloadsOpen=false;linkOpen=true;}
 function showLink(info:NexusInfo){
  const same=(other:NexusInfo)=>other.mod_id===info.mod_id&&other.file_id===info.file_id;
  if((nexus&&same(nexus))||waitingLinks.some(same)||jobs.some(j=>same(j.info)&&!['failed','cancelled'].includes(j.status)))return;
  if(busy||updating||settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen){waitingLinks.push(info);return}activateLink(info);
 }
 function dismissLink(){linkOpen=false;nexus=null}
 $effect(()=>{if(!busy&&!updating&&!settingsOpen&&!editOpen&&!linkOpen&&!importOpen&&!downloadsOpen&&waitingLinks.length)activateLink(waitingLinks.shift()!)});
 function patchJob(id:string,patch:Partial<DownloadJob>){jobs=jobs.map(job=>job.info.request_id===id?{...job,...patch}:job)}
 $effect(()=>{if(!processing&&!updating&&!busy&&!queuePaused&&jobs.some(j=>j.status==='queued'))void processQueue()});
 function confirmDownload(){if(!nexus||!nexusKey.trim())return;jobs.push({info:nexus,status:'queued',received:0,total:null});dismissLink()}
 async function processQueue(){
  if(processing||updating||busy||queuePaused)return;
  const job=jobs.find(j=>j.status==='queued');if(!job)return;
  const id=job.info.request_id;processing=true;
  try{
   let result=job.result;
   if(!result){
    if(!nexusKey.trim())throw new Error('nexusApiKeyRequired');
    patchJob(id,{status:'downloading',error:undefined});
    result=await invoke<DownloadResult>('download_nxm',{requestId:id,apiKey:nexusKey.trim()});
    patchJob(id,{result,received:result.bytes});
   }
   if(result.can_import){patchJob(id,{status:'importing'});await invoke('import_download',{path:result.path});patchJob(id,{status:'completed',error:undefined});try{await refresh()}catch(e){error=String(e)}}
   else patchJob(id,{status:'needsExtraction',error:undefined});
  }catch(e){const detail=String(e).replace(/^Error: /,'');const cancelled=detail==='downloadCancelled';const saved=jobs.find(j=>j.info.request_id===id)?.result;patchJob(id,{status:cancelled?'cancelled':saved?'importFailed':'failed',error:cancelled?undefined:detail})}
  finally{processing=false}
 }
 async function cancelDownload(job:DownloadJob){
  const id=job.info.request_id;
  if(job.status==='queued'){patchJob(id,{status:'cancelled'});return}
  if(job.status==='downloading'){patchJob(id,{status:'cancelling'});try{await invoke('cancel_nxm',{requestId:id})}catch(e){error=String(e);patchJob(id,{status:'downloading'})}}
 }
 function retryDownload(job:DownloadJob){patchJob(job.info.request_id,{status:'queued',error:undefined,received:job.result?.bytes??0})}
 async function chooseDownloadFolder(){await run(async()=>{if(!data)return;const path=await open({directory:true,title:t('downloadFolder')});if(path){await invoke('save_settings',{settings:{...data.settings,downloads_dir:path}});await refresh()}})}
 function openDownloads(){if(updating)return;settingsOpen=false;editOpen=false;importOpen=false;dismissLink();downloadsOpen=true}
 let updateStatus=$state<TranslationKey>('checkingUpdates');
 let input:HTMLInputElement;
 type ViewMode = 'grid' | 'list';
 function initialView():ViewMode {try{return localStorage.getItem('inzoimm.view')==='list'?'list':'grid'}catch{return 'grid'}}
 let view=$state<ViewMode>(initialView());
 let modItems=$state<HTMLDivElement>(), modList=$state<HTMLElement>();
 function setView(mode:ViewMode){view=mode;try{localStorage.setItem('inzoimm.view',mode)}catch{/* Keep the session preference. */}}
 async function chooseThumbnail(m:Mod){await run(async()=>{const path=await open({multiple:false,title:t('chooseThumbnail'),filters:[{name:t('imageFiles'),extensions:['png','jpg','jpeg','webp']}]});if(path){await invoke('set_mod_thumbnail',{id:m.id,path});await refresh()}})}
 let folder=$state(''), tagFilter=$state(''), manageSources=$state(false), sourceQuery=$state('');
 const sourcesView=$derived(filter==='managed'&&manageSources);
 const attachedSources=$derived(sourceFolders(data?.settings));
 const folderTree=$derived(folders(data?.settings,data?.mods??[]));
 const currentFolder=$derived(folderTree.find(f=>pathKey(f.path)===pathKey(folder)));
 const childFolders=$derived(folder?folderTree.filter(f=>f.parent?pathKey(f.parent)===pathKey(folder):pathKey(normalizePath(f.path).split('/').slice(0,-1).join('/'))===pathKey(folder)):[]);
 const managedRoots=$derived(folderTree.filter(f=>f.parent===null&&f.kind!=='game'));
 function rootLabel(root:(typeof folderTree)[number]){return root.kind==='downloads'?t('downloads'):root.kind==='managed'&&data&&pathKey(root.path)===pathKey(data.settings.library_dir)?t('libraryFolder'):root.name}
 const sourceRows=$derived<ManagedSource[]>(managedRoots.map(root=>({...root,label:rootLabel(root),configurable:root.kind==='source'||(root.kind==='managed'&&!!data&&pathKey(root.path)===pathKey(data.settings.library_dir)),recursive:sourceScan(root.path)})));
 const allTags=$derived([...new Set((data?.mods??[]).flatMap(m=>m.tags??[]))].sort((a,b)=>a.localeCompare(b)));
 $effect(()=>{if(data&&tagFilter&&!allTags.includes(tagFilter))tagFilter=''});
 type Location={filter:string;folder:string;query:string;tagFilter:string;selected:string;scroll:number;manageSources:boolean};
 let locations=$state<Location[]>([]), locationIndex=$state(0);
 const historySession=crypto.randomUUID();
 const canBack=$derived(locationIndex>0), canForward=$derived(locationIndex<locations.length-1);
 function location():Location{return{filter,folder,query,tagFilter,selected,manageSources,scroll:modList?.scrollTop??0}}
 function navigate(next:Partial<Location>){
  closeContext();
  if(locations.length)locations[locationIndex]=location();
  const nextLocation={...location(),...next,scroll:next.scroll??0};
  if(nextLocation.filter===filter&&pathKey(nextLocation.folder)===pathKey(folder)&&nextLocation.query===query&&nextLocation.tagFilter===tagFilter&&nextLocation.selected===selected&&nextLocation.manageSources===manageSources)return;
  locations=[...locations.slice(0,locationIndex+1),nextLocation];locationIndex=locations.length-1;
  window.history.pushState({inzoimm:historySession,index:locationIndex},'');restoreLocation(nextLocation);
 }
 function restoreLocation(point:Location){manageSources=point.manageSources;filter=point.filter;folder=point.folder;query=point.query;tagFilter=point.tagFilter;selected=point.selected;requestAnimationFrame(()=>{if(modList)modList.scrollTop=point.scroll})}
 function historyMove(delta:number){if(settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||updating)return;closeContext();if(delta<0?!canBack:!canForward)return;locations[locationIndex]=location();window.history.go(delta)}
 function historyPop(e:PopStateEvent){if(e.state?.inzoimm!==historySession||!locations[e.state.index])return;locations[locationIndex]=location();locationIndex=e.state.index;closeContext();restoreLocation(locations[locationIndex])}
 function mouseNavigation(e:MouseEvent){if(e.button!==3&&e.button!==4)return;e.preventDefault();e.stopPropagation();if(e.type==='mouseup')historyMove(e.button===3?-1:1)}
 function browse(path:string){navigate({folder:path,filter:'managed',selected:'',manageSources:false})}
 function chooseFilter(value:string){navigate({filter:value,folder:'',selected:'',manageSources:false})}
 function managedTabKeys(e:KeyboardEvent){if(!['ArrowLeft','ArrowRight','Home','End'].includes(e.key))return;e.preventDefault();e.stopPropagation();const next=e.key==='Home'?false:e.key==='End'?true:!manageSources;navigate({manageSources:next,folder:''});requestAnimationFrame(()=>document.getElementById(next?'managed-sources-tab':'managed-mods-tab')?.focus())}
 function addSource(){if(busy||updating||!data)return;closeContext();sourcePaths=[];addingSource=true;importRecursive=true;importOpen=true}
 async function chooseSourceFolders(){await run(async()=>{const chosen=await open({directory:true,multiple:true,title:t('addSource')});if(chosen)sourcePaths=typeof chosen==='string'?[chosen]:chosen})}
 async function saveSourceFolders(){await run(async()=>{
  if(!data)return;const sources=sourceFolders(data.settings);
  for(const path of sourcePaths){if(![data.settings.mods_dir,data.settings.library_dir,downloadFolder(data.settings),...sources.map(s=>s.path)].some(root=>pathKey(root)===pathKey(path)))sources.push({path,scan_subfolders:importRecursive})}
  await invoke('save_settings',{settings:{...data.settings,source_dirs:sources}});await refresh();importOpen=false;navigate({filter:'managed',folder:'',selected:'',manageSources:true});
 })}
 async function removeSource(path:string){await run(async()=>{if(!data)return;await invoke('save_settings',{settings:{...data.settings,source_dirs:sourceFolders(data.settings).filter(source=>pathKey(source.path)!==pathKey(path))}});if(inside(folder,path))navigate({folder:'',selected:''});await refresh()})}
 async function changeSourceScan(path:string,recursive:boolean){return run(async()=>{if(!data)return;await invoke('save_settings',{settings:{...data.settings,scan_subfolders:pathKey(path)===pathKey(data.settings.library_dir)?recursive:data.settings.scan_subfolders,source_dirs:sourceFolders(data.settings).map(source=>pathKey(source.path)===pathKey(path)?{...source,scan_subfolders:recursive}:source)}});await refresh()})}
 async function saveTags(mod:Mod,tags:string[]){return run(async()=>{await invoke('set_mod_tags',{id:mod.id,tags});await refresh()})}
 function sourceScan(path:string){return data&&pathKey(path)===pathKey(data.settings.library_dir)?data.settings.scan_subfolders:attachedSources.find(source=>pathKey(source.path)===pathKey(path))?.scan_subfolders??true}
 type ContextTarget={kind:'library'}|{kind:'mod';id:string}|{kind:'folder';path:string}|{kind:'source';path:string;rootKind:Folder['kind']};
 let context=$state<{target:ContextTarget;x:number;y:number;origin:HTMLElement}|null>(null);
 function closeContext(restoreFocus=false){const origin=context?.origin;context=null;if(restoreFocus&&origin?.isConnected)origin.focus({preventScroll:true})}
 function contextTarget(node:HTMLElement,initial:ContextTarget){
  let target=initial;
  function openMenu(e:MouseEvent|KeyboardEvent){
   if(e.target instanceof HTMLElement&&e.target.closest('input:not([type="checkbox"]),textarea,select,[contenteditable="true"]'))return;
   e.preventDefault();e.stopPropagation();if(settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen)return;
   if(target.kind==='mod')selected=target.id;
   const rect=node.getBoundingClientRect();context={target,x:e instanceof MouseEvent?e.clientX:rect.left+12,y:e instanceof MouseEvent?e.clientY:rect.top+Math.min(rect.height,36),origin:e.target instanceof Element?(e.target.closest<HTMLElement>('button,input,[tabindex]')??(document.activeElement instanceof HTMLElement?document.activeElement:node)):node};
  }
  const key=(e:KeyboardEvent)=>{if(e.key==='ContextMenu'||(e.shiftKey&&e.key==='F10'))openMenu(e)};
  node.addEventListener('contextmenu',openMenu);node.addEventListener('keydown',key);
  return{update(next:ContextTarget){target=next},destroy(){node.removeEventListener('contextmenu',openMenu);node.removeEventListener('keydown',key)}};
 }
 const contextItems=$derived.by(()=>{
  const items:MenuItem[]=[];if(!context)return items;
  const blocked=busy||updating;
  function item(id:string,label:string,run:()=>void,options:Partial<MenuItem>={}){items.push({id,label,run,...options})}
  const target=context.target;
  if(target.kind==='mod'){
   const mod=data?.mods.find(m=>m.id===target.id);if(!mod)return items;
   item('open',t('openFolder'),()=>void run(async()=>{await openPath(mod.path)}),{disabled:blocked});
   item('thumbnail',t('chooseThumbnail'),()=>void chooseThumbnail(mod),{disabled:blocked});
   item('tags',t('editTags'),()=>{selected=mod.id;requestAnimationFrame(()=>{const field=document.querySelector<HTMLInputElement>('.inspector .tag-form input');field?.focus();field?.scrollIntoView({block:'nearest'})})},{disabled:blocked});
   if(mod.managed){
    item('toggle',t(mod.enabled?'disableMod':'enableMod'),()=>void toggle(mod),{disabled:blocked,separator:true});
    item('edit',t('edit'),()=>editMod(mod),{disabled:blocked});
    item('remove',t('remove'),()=>void remove(mod),{disabled:blocked||mod.enabled,danger:true});
   }else{
    if(inGame(mod))item('adopt',t('takeControl'),()=>void adopt(mod),{disabled:blocked,separator:true});
    item('copy',t('copyToLibrary'),()=>void copyToLibrary(mod),{disabled:blocked});
   }
   webLinks(mod).forEach((url,index)=>item(`link-${index}`,url,()=>void run(async()=>{await openUrl(url)}),{disabled:blocked,separator:index===0}));
  }else if(target.kind==='folder'||target.kind==='source'){
   item('browse',t('open'),()=>browse(target.path));
   item('open',t('openFolder'),()=>void run(async()=>{await openPath(target.path)}),{disabled:blocked});
   const configurable=target.kind==='source'&&(target.rootKind==='source'||(target.rootKind==='managed'&&data&&pathKey(target.path)===pathKey(data.settings.library_dir)));
   if(configurable)item('scan',t('scanSubfolders'),()=>void changeSourceScan(target.path,!sourceScan(target.path)),{disabled:blocked,checked:sourceScan(target.path),separator:true});
   item('refresh',t('refresh'),()=>void run(refresh),{disabled:blocked});
   if(target.kind==='source'&&target.rootKind==='source')item('detach',t('removeSource',{folder:folderName(target.path)}),()=>void removeSource(target.path),{disabled:blocked,separator:true});
  }else{
   item('back',t('back'),()=>historyMove(-1),{disabled:!canBack||updating});
   item('forward',t('forward'),()=>historyMove(1),{disabled:!canForward||updating});
   item('refresh',t('refresh'),()=>void run(refresh),{disabled:blocked,separator:true});
   if(filter==='managed')item('source',t('addSource'),addSource,{disabled:blocked||!data});
   else{item('add',t('addMod'),()=>void add(),{disabled:blocked||!data});item('zip',t('importZip'),()=>void addZip(),{disabled:blocked||!data})}
   item('grid',t('gridView'),()=>setView('grid'),{checked:view==='grid',separator:true});
   item('list',t('listView'),()=>setView('list'),{checked:view==='list'});
  }
  return items;
 });
 function inGame(mod:Mod){return !!data&&inside(mod.path,data.settings.mods_dir)}
 const filtered=$derived((data?.mods??[]).filter(m=>(filter==='all'||filter==='managed'&&(m.managed||!inGame(m))||filter==='external'&&!m.managed&&inGame(m)||filter==='enabled'&&m.enabled)&&(!folder||(query.trim()||tagFilter?inside(m.path,folder):pathKey(normalizePath(m.path).split('/').slice(0,-1).join('/'))===pathKey(folder)||pathKey(m.path)===pathKey(folder)))&&(!tagFilter||(m.tags??[]).includes(tagFilter))&&`${m.name} ${m.author} ${m.project} ${m.path} ${(m.tags??[]).join(' ')}`.toLowerCase().includes(query.toLowerCase())));
 const errorMessage=$derived(error==='desktopRequired'?t('desktopRequired'):error==='nexusApiKeyRequired'?t('nexusApiKeyRequired'):error);
 const active=$derived(filtered.find(m=>m.id===selected)??filtered[0]);
 async function run(action:()=>Promise<void>){if(busy||updating)return false;busy=true;error='';message='';try{await action();return true}catch(e){error=String(e);return false}finally{busy=false}}
 async function refresh(){data=await invoke<Snapshot>('snapshot')}
 async function add(){await run(async()=>{const p=await open({directory:true,multiple:false,title:t('chooseImportFolder')});if(p){addingSource=false;importPath=p;importRecursive=data?.settings.scan_subfolders??true;importOpen=true}})}
 async function importFolder(){await run(async()=>{await invoke('import_folder',{path:importPath,scanSubfolders:importRecursive});await refresh();message='folderImported';importOpen=false})}
 async function addZip(){await run(async()=>{const p=await open({multiple:false,filters:[{name:t('zipArchive'),extensions:['zip']}]});if(p){await invoke('import_zip',{path:p});await refresh();message='zipImported'}})}
 async function copyToLibrary(m:Mod){await run(async()=>{await invoke('import_folder',{path:m.path});await refresh();message='copied'})}
 function editMod(m:Mod){selected=m.id;editName=m.name;editDescription=m.description;editOpen=true}
 async function adopt(m:Mod){await run(async()=>{if(await confirm(t('adoptConfirm',{name:m.name}),{title:t('adoptTitle'),okLabel:t('confirm'),cancelLabel:t('cancel')})){await invoke('adopt_mod',{path:m.path});await refresh();message='adopted'}})}
 async function toggle(m:Mod){await run(async()=>{await invoke('toggle_mod',{id:m.id});await refresh()})}
 async function remove(m:Mod){await run(async()=>{if(await confirm(t('removeConfirm',{name:m.name}),{title:t('removeTitle'),kind:'warning',okLabel:t('confirm'),cancelLabel:t('cancel')})){await invoke('remove_mod',{id:m.id});await refresh()}})}
 async function updates(){
  if(updating||busy||processing||pendingCount||settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen)return;
  updating=true;updateStatus='checkingUpdates';
  try{
   if(await invoke<boolean>('is_portable')){updateStatus='applyingUpdate';const applied=await invoke<boolean>('update_portable');if(!applied)updateStatus='upToDate';}
   else{update=await check({timeout:20000});if(update){updateStatus='applyingUpdate';await update.downloadAndInstall(undefined,{timeout:600000,restartAfterInstall:true});await relaunch();}else updateStatus='upToDate';}
  }catch(e){updateStatus='updateUnavailable';error=String(e)}finally{updating=false}
 }
 function openNexusKeys(){void run(async()=>{await openUrl('https://www.nexusmods.com/settings/api-keys')})}
 async function receiveLink(raw:string){try{const info=await invoke<NexusInfo>('parse_nxm',{link:raw});downloadsOpen=false;showLink(info)}catch(e){error=String(e)}}
 onMount(()=>{locations=[location()];locationIndex=0;window.history.replaceState({inzoimm:historySession,index:0},'');if(!isTauri()){error='desktopRequired';return}
  refresh().catch(e=>error=String(e));void updates();const updateTimer=setInterval(()=>void updates(),60_000);invoke<{request_id:string;mod_id:number;file_id:number;url:string}[]>('pending_links').then(links=>{if(links.length){for(const info of links)showLink(info);}}).catch(e=>error=String(e));const progressUnlisten=listen<{request_id:string;received:number;total:number|null}>('nexus-download-progress',e=>{const job=jobs.find(j=>j.info.request_id===e.payload.request_id);if(job&&['downloading','cancelling'].includes(job.status))patchJob(e.payload.request_id,{received:e.payload.received,total:e.payload.total})});const unlisten=listen<{request_id:string;mod_id:number;file_id:number;url:string}>('nexus-link',e=>showLink(e.payload));getCurrent().then(urls=>{for(const u of urls??[])if(u.startsWith('nxm:'))void receiveLink(u)}).catch(e=>error=String(e));return()=>{clearInterval(updateTimer);void unlisten.then(f=>f());void progressUnlisten.then(f=>f())};
 });
 function focusDialog(node:HTMLElement){const previous=document.activeElement as HTMLElement|null;const controls=()=>Array.from(node.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),[tabindex="0"]')).filter(el=>el.offsetParent!==null);queueMicrotask(()=>(node.querySelector<HTMLElement>('[data-autofocus]:not(:disabled)')??controls()[0])?.focus());const trap=(e:KeyboardEvent)=>{if(e.key!=='Tab')return;const all=controls();if(!all.length){e.preventDefault();return}const first=all[0],last=all[all.length-1];if(e.shiftKey&&(document.activeElement===first||document.activeElement===node||!node.contains(document.activeElement))){e.preventDefault();last.focus()}else if(!e.shiftKey&&(document.activeElement===last||!node.contains(document.activeElement))){e.preventDefault();first.focus()}};node.addEventListener('keydown',trap);return{destroy(){node.removeEventListener('keydown',trap);if(previous?.isConnected)previous.focus()}};}
 function openSettings(){if(updating)return;draft=data?{...data.settings,scan_subfolders:data.settings.scan_subfolders??true}:null;editOpen=false;dismissLink();downloadsOpen=false;importOpen=false;settingsOpen=true;queueMicrotask(()=>document.querySelector<HTMLInputElement>('.modal input')?.focus());}
 function keyboard(e:KeyboardEvent){if(e.altKey&&['ArrowLeft','ArrowRight'].includes(e.key)){e.preventDefault();historyMove(e.key==='ArrowLeft'?-1:1);return}if(context)return;if((e.ctrlKey||e.metaKey)&&e.key===','){e.preventDefault();openSettings();return}if((e.ctrlKey||e.metaKey)&&e.key==='k'){e.preventDefault();if(!settingsOpen&&!editOpen&&!linkOpen&&!importOpen&&!downloadsOpen)input?.focus();return}if(e.key==='Escape'){settingsOpen=false;dismissLink();editOpen=false;importOpen=false;downloadsOpen=false}if(settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||sourcesView)return;if(e.target instanceof HTMLInputElement||e.target instanceof HTMLTextAreaElement||e.target instanceof HTMLSelectElement)return;if(['ArrowDown','ArrowUp',...(view==='grid'?['ArrowLeft','ArrowRight']:[])].includes(e.key)){
  e.preventDefault();const i=filtered.findIndex(m=>m.id===active?.id);
  const cards=Array.from(modItems?.querySelectorAll<HTMLButtonElement>('.mod-row')??[]);
  const columns=view==='grid'?(cards.filter(card=>card.offsetTop===cards[0]?.offsetTop).length||1):1;
  const step=e.key==='ArrowDown'?columns:e.key==='ArrowUp'?-columns:e.key==='ArrowRight'?1:-1;
  const next=Math.max(0,Math.min(filtered.length-1,i+step));selected=filtered[next]?.id??'';
  cards[next]?.scrollIntoView({block:'nearest'});
  if(e.target instanceof HTMLElement&&e.target.closest('.mod-row'))cards[next]?.focus();
 }}
</script>
<svelte:window onkeydown={keyboard} onpopstate={historyPop} onmousedown={mouseNavigation} onmouseup={mouseNavigation} onauxclick={mouseNavigation}/>
<div class="shell" use:contextTarget={{kind:'library'}}>
 <aside>
  <div class="aside-content">
  <div class="brand"><img src="/app-icon.png" alt=""/><div>inZOI<span>Better Mod Manager</span></div></div>
  <div class="section-label">{t('library')}</div>
  {#each [{id:'all',name:t('allMods'),icon:IconBox},{id:'enabled',name:t('enabled'),icon:IconCheck},{id:'managed',name:t('managedMods'),icon:IconFolder},{id:'external',name:t('externalMods'),icon:IconArrowUpRight}] as tab}
   <button class:chosen={filter===tab.id} onclick={()=>chooseFilter(tab.id)}><tab.icon size={18}/>{tab.name}<small>{(data?.mods??[]).filter(m=>tab.id==='all'||tab.id==='enabled'&&m.enabled||tab.id==='managed'&&(m.managed||!inGame(m))||tab.id==='external'&&!m.managed&&inGame(m)).length}</small></button>
  {/each}
  <label class="tag-filter">{t('tags')}<select aria-label={t('filterTags')} bind:value={tagFilter}><option value="">{t('allTags')}</option>{#each allTags as tag}<option value={tag}>{tag}</option>{/each}</select></label>
  <div class="section-label space">{t('connections')}</div>
  <button onclick={()=>void updates()} disabled={busy||updating||processing||pendingCount>0}><IconDownload size={18}/>{t('appUpdates')}</button><span class="update-status">{t(updateStatus,{version:update?.version??''})}</span>
  </div>
  <div class="aside-bottom"><button class="downloads-button" class:chosen={downloadsOpen} disabled={updating} onclick={openDownloads}><IconDownload size={18}/>{t('downloads')}{#if pendingCount}<small class="download-count">{pendingCount}</small>{:else if jobs.some(job=>job.status==='failed'||job.status==='importFailed')}<small class="download-count" aria-label={t('downloadFailed')}>!</small>{/if}</button><div class="game"><span class="game-dot"></span><div>inZOI<small>{t('localLibraryHint')}</small></div></div><button disabled={updating} onclick={openSettings}><IconSettings size={18}/>{t('settings')}<kbd>Ctrl ,</kbd></button></div>
 </aside>
 <main>
  <header><div class="navigation-buttons" role="group" aria-label={t('navigation')}><button class="icon" title={t('back')} aria-label={t('back')} disabled={!canBack||updating} onclick={()=>historyMove(-1)}><IconArrowLeft size={18}/></button><button class="icon" title={t('forward')} aria-label={t('forward')} disabled={!canForward||updating} onclick={()=>historyMove(1)}><IconArrowRight size={18}/></button></div><div class="search"><IconSearch size={20}/><input bind:this={input} value={sourcesView?sourceQuery:query} oninput={e=>{if(sourcesView)sourceQuery=e.currentTarget.value;else query=e.currentTarget.value}} placeholder={t(sourcesView?'searchSources':'searchPlaceholder')} aria-label={t(sourcesView?'searchSources':'search')}/>{#if sourcesView&&sourceQuery}<button class="icon" aria-label={t('clearSourceSearch')} onclick={()=>sourceQuery=''}><IconX size={15}/></button>{/if}<kbd>Ctrl K</kbd></div><button class="icon" title={t('refresh')} disabled={busy||updating} onclick={()=>void run(refresh)}><IconRefresh size={18} class={busy?'spin':''}/></button></header>
  <div class="page-title" class:managed-title={filter==='managed'}><div><div class="breadcrumb">{t('personalLibrary')} <IconChevronRight size={12}/> inZOI</div><h1>{folder?currentFolder?.name??folderName(folder):filter==='external'?t('externalMods'):filter==='enabled'?t('enabled'):filter==='managed'?t('yourLibrary'):t('allMods')}</h1><p>{sourcesView?t('sourceCount',{count:managedRoots.length}):t('modCount',{count:filtered.length})}</p></div><div class="actions">{#if filter==='managed'}<button class="primary" disabled={busy||updating||!data} onclick={addSource}><IconPlus size={17}/>{t('addSource')}</button>{:else}<button disabled={busy||updating||!data} onclick={addZip}><IconDownload size={16}/>{t('importZip')}</button><button class="primary" disabled={busy||updating||!data} onclick={add}><IconPlus size={17}/>{t('addMod')}</button>{/if}</div></div>
  {#if error}<div class="notice error" role="alert">{t('operationFailed')}: {errorMessage}<button class="icon" onclick={()=>error=''} aria-label={t('closeError')}><IconX size={16}/></button></div>{/if}
  {#if message}<div class="notice" role="status">{t(message as TranslationKey,{version:update?.version??''})}</div>{/if}
  {#if updating&&update}<div class="notice" role="status">{t('applyingUpdate')}</div>{/if}
  {#if data?.warnings.length}<details class="warnings"><summary>{t('scanWarnings',{count:data.warnings.length})}</summary>{#each data.warnings as warning}<p>{warning}</p>{/each}</details>{/if}
  {#if filter==='managed'}<div class="managed-tabs" role="tablist" tabindex="-1" aria-label={t('managedMods')} onkeydown={managedTabKeys}><button id="managed-mods-tab" role="tab" aria-selected={!manageSources} aria-controls="managed-mods-panel" tabindex={manageSources?-1:0} onclick={()=>navigate({manageSources:false})}>{t('modsTab')}</button><button id="managed-sources-tab" role="tab" aria-selected={manageSources} aria-controls="managed-sources-panel" tabindex={manageSources?0:-1} onclick={()=>navigate({manageSources:true,folder:''})}>{t('sourceFolders')}<span>{managedRoots.length}</span></button></div>{/if}
  {#if sourcesView}<div class="sources-workspace" id="managed-sources-panel" role="tabpanel" aria-labelledby="managed-sources-tab"><SourceManager sources={sourceRows} search={sourceQuery} disabled={busy||updating} contextAction={contextTarget} onbrowse={browse} onopen={path=>void run(async()=>{await openPath(path)})} onremove={path=>void removeSource(path)} onscan={changeSourceScan}/></div>{:else}
  <div class="workspace" id="managed-mods-panel" role={filter==='managed'?'tabpanel':undefined} aria-labelledby={filter==='managed'?'managed-mods-tab':undefined}><section class="mod-list" bind:this={modList} aria-label={t('modList')}>
  <div class="library-toolbar">{#if filter==='managed'}<div class="source-scope"><IconFolder size={15}/><select aria-label={t('browseSource')} value={currentFolder?.root??''} onchange={e=>{if(e.currentTarget.value)browse(e.currentTarget.value);else navigate({folder:'',selected:'',manageSources:false})}}><option value="">{t('allSources')}</option>{#each managedRoots as root}<option value={root.path}>{rootLabel(root)}</option>{/each}</select></div>{:else}<span>{t('modColumn')}</span>{/if}<div class="view-switch" role="group" aria-label={t('viewLayout')}>
   <button class="icon" class:chosen={view==='grid'} aria-label={t('gridView')} title={t('gridView')} aria-pressed={view==='grid'} onclick={()=>setView('grid')}><IconLayoutGrid size={17}/></button>
   <button class="icon" class:chosen={view==='list'} aria-label={t('listView')} title={t('listView')} aria-pressed={view==='list'} onclick={()=>setView('list')}><IconList size={17}/></button>
  </div></div>
  {#if folder}<div class="folder-breadcrumb"><button onclick={()=>navigate({folder:'',selected:''})}>{t('managedMods')}</button><IconChevronRight size={12}/><span title={folder}>{folder}</span>{#if currentFolder?.parent}<button class="icon" title={t('parentFolder')} aria-label={t('parentFolder')} onclick={()=>browse(currentFolder!.parent!)}>↑</button>{/if}</div>{/if}
  {#if !query.trim()&&!tagFilter&&folder&&childFolders.length}<div class="folder-cards">{#each childFolders as entry}<button title={entry.path} use:contextTarget={{kind:'folder',path:entry.path}} onclick={()=>browse(entry.path)}><IconFolder size={23}/><span>{entry.parent===null?rootLabel(entry):entry.name}<small>{t('folderModCount',{count:entry.count})}</small></span><IconChevronRight size={14}/></button>{/each}</div>{/if}
  <div class="mod-items" class:mod-grid={view==='grid'} bind:this={modItems}>
  {#each filtered as mod (mod.id)}
   <button class="mod-row" use:contextTarget={{kind:'mod',id:mod.id}} class:selected={active?.id===mod.id} aria-pressed={active?.id===mod.id} onclick={()=>selected=mod.id}>
    <div class="thumb"><ModThumbnail path={mod.thumbnail} name={mod.name||mod.project} large={view==='grid'}/></div>
    <div class="mod-title"><strong title={mod.name||mod.project}>{mod.name||mod.project}</strong><span>{mod.author||t('unknownAuthor')}</span><span class="mod-origin">{mod.managed?t('privateLibrary'):inGame(mod)?t('inGame'):t('externalSource')}</span>{#if mod.tags?.length}<span class="mod-tags" title={mod.tags.join(' · ')}>{mod.tags.join(' · ')}</span>{/if}</div>
    <span class:enabled={mod.enabled} class="status">{mod.enabled?t('enabled'):t('disabled')}</span>
   </button>
  {/each}
  </div>
  {#if filtered.length===0&&(!childFolders.length||query.trim()||tagFilter)}<div class="empty"><IconBox size={42} stroke={1}/><h2>{query?t('noResults'):t('emptyTitle')}</h2><p>{query?t('noResultsHint'):t('emptyHint')}</p>{#if !query}<button onclick={add} disabled={!data||busy}>{t('chooseModFolder')} <IconArrowUpRight size={16}/></button>{/if}</div>{/if}
  </section><section class="inspector" use:contextTarget={active?{kind:'mod',id:active.id}:{kind:'library'}} aria-label={t('modDetails')}>{#if active}<div class="cover"><ModThumbnail path={active.thumbnail} name={active.name||active.project} large/></div><div class="inspector-body"><button class="wide thumbnail-action" disabled={busy||updating} onclick={()=>chooseThumbnail(active!)}><IconPhoto size={16}/>{t('chooseThumbnail')}</button><span class="eyebrow">{active.managed?t('privateLibrary'):inGame(active)?t('detectedInGame'):t('externalSource')}</span><h2>{active.name||active.project}</h2><p class="author">{t('byAuthor',{author:active.author||t('unknownAuthor')})}</p><p class="description">{active.description||t('noDescription')}</p><dl><div><dt>{t('version')}</dt><dd>{active.version||'—'}</dd></div><div><dt>{t('status')}</dt><dd>{active.enabled?t('enabled'):t('disabled')}</dd></div><div><dt>{t('managedBy')}</dt><dd>{active.managed?'Better Mod Manager':t('outsideApp')}</dd></div></dl>{#key active.id}<ModMetadata mod={active} disabled={busy||updating} onopen={url=>void run(async()=>{await openUrl(url)})} ontags={tags=>saveTags(active!,tags)}/>{/key}{#if active.managed}<button class="wide primary" disabled={busy||updating} onclick={()=>toggle(active!)}>{active.enabled?t('disableMod'):t('enableMod')}</button><div class="actions"><button disabled={updating} onclick={()=>editMod(active!)}><IconPencil size={16}/>{t('edit')}</button><button class="danger" disabled={busy||updating||active.enabled} onclick={()=>remove(active!)}><IconTrash size={16}/>{t('remove')}</button></div>{:else}{#if inGame(active)}<div class="external-note">{t('externalNote')}</div><button class="primary wide" disabled={busy||updating} onclick={()=>adopt(active!)}>{t('takeControl')}</button>{:else}<p class="external-note">{t('sourceNote')}</p>{/if}<button class="wide" disabled={busy||updating} onclick={()=>copyToLibrary(active!)}>{t('copyToLibrary')}</button>{/if}<button class="wide ghost" onclick={()=>void run(async()=>{await openPath(active!.path)})}><IconFolder size={16}/>{t('openFolder')}</button></div>{:else}<div class="inspector-empty"><IconCommand size={28} stroke={1}/><p>{t('selectMod')}</p></div>{/if}</section></div>
  {/if}
  <footer><span><span class="game-dot"></span>{updating?t('applyingUpdate'):busy?t('working'):t('localLibrary')}</span><span><kbd>↑</kbd><kbd>↓</kbd> {t('navigate')} <kbd>Ctrl K</kbd> {t('search')}</span></footer>
 </main>
</div>
{#if settingsOpen||editOpen||linkOpen||importOpen}<div class="overlay"><div class="modal" use:focusDialog role="dialog" aria-modal="true" aria-label={settingsOpen?t('settings'):editOpen?t('editMod'):importOpen?t(addingSource?'addSource':'importFolder'):t('confirmNexusDownload')} tabindex="-1"><div class="modal-head"><h2>{settingsOpen?t('librarySettings'):editOpen?t('editMod'):importOpen?t(addingSource?'addSource':'importFolder'):t('confirmNexusDownload')}</h2><button class="icon" onclick={()=>{settingsOpen=false;editOpen=false;dismissLink();importOpen=false}} aria-label={t('close')}><IconX size={20}/></button></div>
 {#if settingsOpen}<label>{t('language')}<select value={language.code} onchange={e=>setLanguage(e.currentTarget.value)}>{#each languages as option}<option value={option.code}>{option.name}</option>{/each}</select></label><p class="muted">{t('languageHint')}</p><NexusApiKey bind:value={nexusKey} disabled={busy||updating} onopen={openNexusKeys}/>{/if}
 {#if settingsOpen&&draft}<p class="muted">{t('settingsHint')}</p>{#each [{key:'mods_dir' as const,name:t('gameModsFolder')},{key:'library_dir' as const,name:t('libraryFolder')}] as f}<label>{f.name}<div class="path-input"><input id={f.key} bind:value={draft[f.key]}/><button title={t('chooseFolder')} onclick={()=>void run(async()=>{const p=await open({directory:true});if(p&&draft)draft[f.key]=p})}><IconFolder size={18}/></button></div></label>{#if f.key==='library_dir'}<label class="checkbox-option"><input type="checkbox" bind:checked={draft.scan_subfolders}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('managedFolderHint')}</p>{/if}{/each}<label>{t('downloadFolder')}<div class="path-input"><input id="downloads_dir" bind:value={draft.downloads_dir} placeholder={`${draft.library_dir}/Downloads`}/><button title={t('chooseDownloadFolder')} onclick={()=>void run(async()=>{const p=await open({directory:true,title:t('downloadFolder')});if(p&&draft)draft.downloads_dir=p})}><IconFolder size={18}/></button></div></label><p class="muted">{t('downloadFolderHint')}</p><p class="muted">{t('updateHint')}</p><button class="wide" onclick={()=>void run(async()=>{if(await confirm(t('nxmConfirm'),{title:t('nxmTitle'),okLabel:t('confirm'),cancelLabel:t('cancel')})){await invoke('register_nxm');message='nxmRegistered'}})}>{t('setNxmHandler')}</button><button class="primary wide" disabled={busy||updating} onclick={()=>void run(async()=>{await invoke('save_settings',{settings:draft});await refresh();settingsOpen=false})}>{t('saveSettings')}</button>
 {:else if importOpen}{#if addingSource}<div class="scan-choice"><label class="checkbox-option"><input type="checkbox" bind:checked={importRecursive}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('scanSubfoldersHint')}</p></div><button class="wide" disabled={busy||updating} onclick={chooseSourceFolders}><IconFolder size={17}/>{t('chooseFolder')}</button>{#each sourcePaths as path}<p class="import-path">{path}</p>{/each}{:else}<p class="import-path">{importPath}</p><label class="checkbox-option"><input type="checkbox" bind:checked={importRecursive}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('scanSubfoldersHint')}</p>{/if}<div class="actions"><button disabled={busy||updating} onclick={()=>importOpen=false}>{t('cancel')}</button><button class="primary" disabled={busy||updating||(addingSource&&!sourcePaths.length)} onclick={addingSource?saveSourceFolders:importFolder}>{t(addingSource?'addSource':'importFolder')}</button></div>
 {:else if editOpen}<label>{t('modName')}<input bind:value={editName}/></label><label>{t('description')}<textarea bind:value={editDescription} rows="5"></textarea></label><button class="primary wide" disabled={busy||updating} onclick={()=>void run(async()=>{await invoke('edit_mod',{id:active!.id,name:editName,description:editDescription});await refresh();editOpen=false})}>{t('saveChanges')}</button>
 {:else if linkOpen&&nexus}<div class="download-confirm"><IconDownload size={28} stroke={1.5}/><p class="link-info">{t('linkInfo',{mod:nexus.mod_id,file:nexus.file_id})}</p><p>{t('downloadConfirmHint')}</p><p class="download-confirm-path" title={downloadFolder(data?.settings)}>{downloadFolder(data?.settings)}</p></div>{#if promptKey}<NexusApiKey bind:value={nexusKey} disabled={updating} onopen={openNexusKeys}/>{/if}<div class="actions"><button onclick={dismissLink}>{t('cancel')}</button><button class="primary" data-autofocus disabled={updating||!nexusKey.trim()} onclick={confirmDownload}>{t('ok')}</button></div>{/if}

 {#if error}<p class="form-error" role="alert">{t('operationFailed')}: {errorMessage}</p>{/if}
 </div></div>{/if}

{#if downloadsOpen}<div class="overlay downloads-overlay"><div class="downloads-panel" use:focusDialog role="dialog" aria-modal="true" aria-label={t('downloads')} tabindex="-1"><Downloads {jobs} paused={queuePaused} path={downloadFolder(data?.settings)} onclose={()=>downloadsOpen=false} onpause={()=>queuePaused=!queuePaused} oncancel={job=>void cancelDownload(job)} onretry={retryDownload} onclear={()=>jobs=jobs.filter(pendingDownload)} onopen={path=>void run(async()=>{if(/^https?:\/\//.test(path))await openUrl(path);else await openPath(path)})} onlink={raw=>void receiveLink(raw)} onfolder={()=>void chooseDownloadFolder()} onsettings={openSettings}/>{#if error}<p class="form-error" role="alert">{t('operationFailed')}: {errorMessage}</p>{/if}</div></div>{/if}

{#if context}{#key context}<ContextMenu x={context.x} y={context.y} items={contextItems} onclose={closeContext}/>{/key}{/if}
