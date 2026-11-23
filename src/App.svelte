<script lang="ts">
 import { language, languages, setLanguage, t, errorText, enablerPrefix, nothingInstallablePrefix, type TranslationKey } from './lib/i18n.svelte';
 import { onMount } from 'svelte';
 import { invoke, isTauri, Channel } from '@tauri-apps/api/core';
 import { listen } from '@tauri-apps/api/event';
 import { open, confirm } from '@tauri-apps/plugin-dialog';
 import { openUrl } from '@tauri-apps/plugin-opener';
 import { check, Update } from '@tauri-apps/plugin-updater';
 import { relaunch } from '@tauri-apps/plugin-process';
 import { getCurrent } from '@tauri-apps/plugin-deep-link';
 import { IconSearch, IconPlus, IconRefresh, IconSettings, IconBox, IconFolder, IconArrowUpRight, IconDownload, IconTrash, IconPencil, IconCommand, IconX, IconChevronRight, IconLayoutGrid, IconList, IconPhoto, IconArrowLeft, IconArrowRight, IconInfoCircle, IconArrowsSort, IconMaximize, IconStack2, IconHistory, IconZoomScan, IconTag, IconFileZip, IconArchive } from '@tabler/icons-svelte';
 import { modCategory, groupMods, type ModCategory, type CategoryKey } from './lib/categories';
 import ModThumbnail from './lib/ModThumbnail.svelte';
 import AppInfo from './lib/AppInfo.svelte';
 import NexusApiKey from './lib/NexusApiKey.svelte';
 import Onboarding from './lib/Onboarding.svelte';
 import Downloads from './lib/Downloads.svelte';
 import DownloadHandoff from './lib/DownloadHandoff.svelte';
 import {downloadFolder,pendingDownload,downloadLabel,type NexusInfo,type DownloadJob,type DownloadResult} from './lib/downloads';
 import ModMetadata from './lib/ModMetadata.svelte';
 import RichDescription from './lib/RichDescription.svelte';
 import ModDetailsDialog from './lib/ModDetailsDialog.svelte';
 import { modDescription } from './lib/description';
 import ModCheckLog from './lib/ModCheckLog.svelte';
 import {checkFailureCode,redactCheckLog,formatCheckLog,validCheckLogs,savedCheckLogs,type CheckLog} from './lib/check-log';
 import ResizableInspector from './lib/ResizableInspector.svelte';
 import SourceManager, {type ManagedSource} from './lib/SourceManager.svelte';
 import ContextMenu, {type MenuItem} from './lib/ContextMenu.svelte';
 import { folders, inside, pathKey, normalizePath, folderName, modFolder, sourceFolders, webLinks, modOrigin, modEnablerUrl, type Folder } from './lib/library';
 import ModDiscovery from './lib/ModDiscovery.svelte';
 import type {Discovery,DiscoveryProgress} from './lib/types';
 import type { Mod, Snapshot, Settings, ModUpdate, InstallReport } from './lib/types';
 import { initialLibrarySort, saveLibrarySort, sortLibrary, type LibrarySort } from './lib/library-sort';
 const openFolder=(path:string)=>invoke<void>('open_folder',{path});
 let data=$state<Snapshot|null>(null), query=$state(''), filter=$state('all'), selected=$state(''), busy=$state(false), error=$state(''), message=$state(''), messageParams=$state<Record<string,string|number>>({}), enablerNoticeHidden=$state(false);
 let settingsOpen=$state(false), draft=$state<Settings|null>(null), editOpen=$state(false), editName=$state(''), editDescription=$state('');
 let modToggle=$state<{name:string;enabled:boolean}|null>(null);
 let stagingSaving=$state(false);
 const operationLabel=$derived(modToggle
  ?t(modToggle.enabled?'enablingMod':'disablingMod',{name:modToggle.name})
  :stagingSaving?t('movingStaging'):'');
 let operationVisible=$state(false);
 $effect(()=>{
  if(!operationLabel){operationVisible=false;return}
  const timer=setTimeout(()=>{operationVisible=true},160);
  return()=>clearTimeout(timer);
 });
 let discoveryOpen=$state(false), discovery=$state<Discovery|null>(null), scanProgress=$state<DiscoveryProgress|null>(null), scanCancelling=$state(false), scanDrives=$state<string[]>([]);
 let scanRequestId=$state('');
 let importOpen=$state(false), importPath=$state(''), importRecursive=$state(true), addingSource=$state(false), sourcePaths=$state<string[]>([]);
 let linkOpen=$state(false), nexus=$state<NexusInfo|null>(null), update=$state<Update|null>(null);
 let waitingLinks=$state<NexusInfo[]>([]), jobs=$state<DownloadJob[]>([]), downloadsOpen=$state(false), queuePaused=$state(false), processing=$state(false);
 const CURSEFORGE_KEYS_URL='https://console.curseforge.com/#/api-keys';
 let nexusKey=$state(''), promptKey=$state(false), updating=$state(false);
 let nexusKeyReady=$state(false), nexusKeySaving=$state(false);
 let keyRevision=0, keyWrites:Promise<void>=Promise.resolve();
 async function loadNexusKey(){
  try{const saved=await invoke<string|null>('load_nexus_api_key');if(!keyRevision)nexusKey=saved??''}
  catch{error='nexusKeyLoadFailed'}finally{nexusKeyReady=true}
 }
 function changeNexusKey(value:string){
  nexusKey=value;const revision=++keyRevision;nexusKeySaving=true;
  keyWrites=keyWrites.catch(()=>{}).then(async()=>{
   if(revision!==keyRevision)return;
   await invoke('save_nexus_api_key',{apiKey:value.trim()});
   if(revision===keyRevision&&['nexusKeySaveFailed','nexusKeyLoadFailed'].includes(error))error='';
  });
  void keyWrites.catch(()=>{if(revision===keyRevision)error='nexusKeySaveFailed'}).finally(()=>{if(revision===keyRevision)nexusKeySaving=false});
 }
 let curseforgeKey=$state(''), curseforgeKeyReady=$state(false), curseforgeKeySaving=$state(false);
 let curseforgeRevision=0, curseforgeWrites:Promise<void>=Promise.resolve();
 async function loadCurseforgeKey(){try{const saved=await invoke<string|null>('load_curseforge_api_key');if(!curseforgeRevision)curseforgeKey=saved??''}catch{error='curseforgeKeyLoadFailed'}finally{curseforgeKeyReady=true}}
 function changeCurseforgeKey(value:string){
  curseforgeKey=value;const revision=++curseforgeRevision;curseforgeKeySaving=true;
  curseforgeWrites=curseforgeWrites.catch(()=>{}).then(async()=>{if(revision!==curseforgeRevision)return;await invoke('save_curseforge_api_key',{apiKey:value.trim()});if(revision===curseforgeRevision&&['curseforgeKeyLoadFailed','curseforgeKeySaveFailed'].includes(error))error=''});
  void curseforgeWrites.catch(()=>{if(revision===curseforgeRevision)error='curseforgeKeySaveFailed'}).finally(()=>{if(revision===curseforgeRevision)curseforgeKeySaving=false});
 }
 const keyFor=(info:NexusInfo)=>info.provider==='curseforge'?curseforgeKey:nexusKey;
 const keyReady=(info:NexusInfo)=>info.provider==='curseforge'?curseforgeKeyReady:nexusKeyReady;
 const missingKeys=$derived((nexusKey.trim()?0:1)+(curseforgeKey.trim()?0:1));
 let nxmRegistered=$state<boolean|null>(null), nxmStatusRequest=0;
 const readyMissingKeys=$derived(nexusKeyReady&&curseforgeKeyReady?missingKeys:0);
 const settingsCount=$derived(readyMissingKeys+(nxmRegistered===false?1:0));
 const settingsAttention=$derived([
  readyMissingKeys?t('keysMissingBadge',{count:readyMissingKeys}):'',
  nxmRegistered===false?t('nxmHandlerMissing'):'',
 ].filter(Boolean).join(' · '));
 async function refreshNxmRegistration(){
  if(!isTauri())return;
  const request=++nxmStatusRequest;
  try{
   const registered=await invoke<boolean|null>('nxm_handler_registered');
   if(request===nxmStatusRequest)nxmRegistered=registered;
   return registered;
  }catch{/* Keep the last known registration state if the OS query fails. */}
 }
 async function registerNxm(){
  await run(async()=>{
   if(!await confirm(t('nxmConfirm'),{title:t('nxmTitle'),okLabel:t('confirm'),cancelLabel:t('cancel')}))return;
   await invoke('register_nxm');
   if(await refreshNxmRegistration()===false){error='nxmHandlerMissing';return}
   message='nxmRegistered';
  });
 }
 let onboardingOpen=$state(false), onboardingChecked=false;
 $effect(()=>{if(onboardingChecked||!nexusKeyReady||!curseforgeKeyReady||updating)return;onboardingChecked=true;if(missingKeys>0){closeContext();onboardingOpen=true}});
 const pendingCount=$derived(jobs.filter(pendingDownload).length);
 function activateLink(info:NexusInfo){nexus=info;promptKey=!keyFor(info).trim();downloadsOpen=false;linkOpen=true;}
 function showLink(info:NexusInfo){
  const same=(other:NexusInfo)=>(other.provider??'nexus')===(info.provider??'nexus')&&other.slug===info.slug&&other.mod_id===info.mod_id&&other.file_id===info.file_id;
  if((nexus&&same(nexus))||waitingLinks.some(same)||jobs.some(j=>same(j.info)&&!['failed','cancelled'].includes(j.status)))return;
  if(!keyReady(info)||busy||updating||onboardingOpen||settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||checkLogsOpen||metadataOpen||discoveryOpen){waitingLinks.push(info);return}activateLink(info);
 }
 function dismissLink(){linkOpen=false;nexus=null}
 $effect(()=>{if(waitingLinks[0]&&keyReady(waitingLinks[0])&&!busy&&!updating&&!onboardingOpen&&!settingsOpen&&!editOpen&&!linkOpen&&!importOpen&&!downloadsOpen&&!checkLogsOpen&&!metadataOpen&&!discoveryOpen&&waitingLinks.length)activateLink(waitingLinks.shift()!)});
 function patchJob(id:string,patch:Partial<DownloadJob>){jobs=jobs.map(job=>job.info.request_id===id?{...job,...patch}:job)}
 $effect(()=>{if(!processing&&!updating&&!busy&&!queuePaused&&jobs.some(j=>j.status==='queued'))void processQueue()});
 let downloadHandoff=$state<DownloadHandoff>(),downloadsButton=$state<HTMLButtonElement>(),downloadConfirmation=$state<HTMLDivElement>();
 function confirmDownload(){if(!nexus||!keyFor(nexus).trim())return;const checked=Object.values(modChecks).find((check):check is ModUpdate=>!!check&&check.status==='available'&&!!check.token&&check.provider===(nexus!.provider??'nexus')&&check.file_id===nexus!.file_id&&updateProjectId(check)===nexus!.mod_id);const m=data?.mods.find(m=>m.id===checked?.id);jobs.push({info:nexus,status:'queued',received:0,total:null,update:checked?.token&&m?{id:m.id,token:checked.token,name:m.name}:undefined});downloadHandoff?.animateFrom(downloadConfirmation?.querySelector('svg')??null,downloadsButton??null);dismissLink()}
 async function processQueue(){
  if(processing||updating||busy||queuePaused)return;
  const job=jobs.find(j=>j.status==='queued');if(!job)return;
  const id=job.info.request_id;processing=true;
  try{
   let result=job.result;
   if(!result){
    if(!keyFor(job.info).trim())throw new Error(job.info.provider==='curseforge'?'curseforgeApiKeyRequired':'nexusApiKeyRequired');
    patchJob(id,{status:'downloading',error:undefined});
    result=await invoke<DownloadResult>('download_nxm',{requestId:id,apiKey:keyFor(job.info).trim()});
    patchJob(id,{result,received:result.bytes});
   }
   if(result.can_import){patchJob(id,{status:'importing'});if(job.update){selected=await invoke<string>('install_mod_update',{token:job.update.token,path:result.path});modChecks={...modChecks,[job.update.id]:undefined}}else{const report=await installArchive('import_download',result.path);if(!report){patchJob(id,{status:'importFailed',error:'modEnablerDeclined'});return}showInstallReport(report)}patchJob(id,{status:'completed',error:undefined})}
   else patchJob(id,{status:'needsExtraction',error:undefined});
  }catch(e){const detail=String(e).replace(/^Error: /,'');const cancelled=detail==='downloadCancelled';const saved=jobs.find(j=>j.info.request_id===id)?.result;if(saved&&detail.startsWith(nothingInstallablePrefix))patchJob(id,{status:'needsExtraction',error:undefined});else patchJob(id,{status:cancelled?'cancelled':saved?'importFailed':'failed',error:cancelled?undefined:detail})}
  finally{if(jobs.find(job=>job.info.request_id===id)?.result){try{await refresh()}catch(e){error=String(e)}}processing=false}
 }
 async function cancelDownload(job:DownloadJob){
  const id=job.info.request_id;
  if(job.status==='queued'){patchJob(id,{status:'cancelled'});return}
  if(job.status==='downloading'){patchJob(id,{status:'cancelling'});try{await invoke('cancel_nxm',{requestId:id})}catch(e){error=String(e);patchJob(id,{status:'downloading'})}}
 }
 function retryDownload(job:DownloadJob){patchJob(job.info.request_id,{status:'queued',error:undefined,received:job.result?.bytes??0})}
 async function chooseDownloadFolder(){await run(async()=>{if(!data)return;const path=await open({directory:true,title:t('downloadFolder')});if(path){await invoke('save_settings',{settings:{...data.settings,downloads_dir:path}});await refresh()}})}
 function openDownloads(){if(updating)return;metadataOpen=false;checkLogsOpen=false;settingsOpen=false;editOpen=false;importOpen=false;dismissLink();downloadsOpen=true}
 let modChecks=$state<Record<string,ModUpdate|undefined>>({}),modCheckErrors=$state<Record<string,string>>({}),checkingMods=$state(false),checkingId=$state(''),modChecksDone=$state(false);
 let checkLogsOpen=$state(false),checkLogLoading=$state(false),checkLogError=$state(''),checkLogPath=$state(''),checkLogCopied=$state(false),checkLogFallback=$state('');
 let checkLogs=$state<CheckLog[]>(savedCheckLogs());
 function saveCheckLog(entry:CheckLog){
  const clean=(value:string,limit:number)=>redactCheckLog(value,[nexusKey,curseforgeKey]).slice(0,limit);
  entry={...entry,mod_id:clean(entry.mod_id,1024),mod_name:clean(entry.mod_name,256),project:clean(entry.project,256),detail:clean(entry.detail,2000)};
  checkLogs=[...checkLogs.filter(old=>old.id!==entry.id),entry].sort((a,b)=>a.timestamp-b.timestamp).slice(-500);
  try{localStorage.setItem('inzoimm.modCheckLog',JSON.stringify(checkLogs))}catch{/* Native history is also saved by the backend. */}
 }
 async function loadCheckLog(){
  checkLogLoading=true;checkLogError='';
  try{const snapshot=await invoke<{entries:CheckLog[];path:string}|null>('read_mod_check_log');if(snapshot){checkLogPath=snapshot.path;const records=new Map(checkLogs.map(entry=>[entry.id,entry]));for(const entry of validCheckLogs(snapshot.entries))records.set(entry.id,entry);checkLogs=[...records.values()].sort((a,b)=>a.timestamp-b.timestamp).slice(-500)}}
  catch(e){checkLogError=redactCheckLog(String(e),[nexusKey,curseforgeKey])}finally{checkLogLoading=false}
 }
 function openCheckLog(){if(updating)return;metadataOpen=false;settingsOpen=false;editOpen=false;importOpen=false;downloadsOpen=false;dismissLink();checkLogCopied=false;checkLogFallback='';checkLogsOpen=true;void loadCheckLog()}
 async function copyCheckLog(entries:CheckLog[]){const text=formatCheckLog(entries,[nexusKey,curseforgeKey]);checkLogCopied=false;checkLogFallback='';try{await navigator.clipboard.writeText(text);checkLogCopied=true}catch{checkLogFallback=text}}
 function inspectCheckLog(entry:CheckLog){const m=libraryMods.find(m=>m.id===entry.mod_id||m.project===entry.project);if(!m)return;checkLogsOpen=false;navigate({filter:'all',folder:'',query:'',tagFilter:'',selected:m.id,manageSources:false,legacy:!!m.legacy});modSections[m.id]=true}
 let metadataOpen=$state(false);
 let modSections=$state<Record<string,boolean>>({});
 let checkGeneration=0;
 const libraryMods=$derived((data?.mods??[]).filter(m=>!m.superseded_by));
 const categoryLabels:Record<CategoryKey,TranslationKey>={outfits:'categoryOutfits',hairstyle:'categoryHairstyle',faceMakeup:'categoryFaceMakeup',accessories:'categoryAccessories',gameplay:'categoryGameplay',other:'categoryOther'};
 function categoryLabel(category:ModCategory){return category.key?t(categoryLabels[category.key]):category.name??''}
 const categories=$derived(new Map(libraryMods.map(m=>[m.id,modCategory(m)])));
 const categoryOf=(mod:Mod)=>categories.get(mod.id)??modCategory(mod);
 function initialGrouping(){try{return localStorage.getItem('inzoimm.groupByCategory')!=='false'}catch{return true}}
 let groupByCategory=$state(initialGrouping());
 function setGrouping(on:boolean){groupByCategory=on;try{localStorage.setItem('inzoimm.groupByCategory',String(on))}catch{/* Keep the session preference. */}}
 const modUpdateCount=$derived(libraryMods.filter(m=>modChecks[m.id]?.status==='available').length);
 const updateProjectId=(check:ModUpdate)=>check.mod_id;
 async function checkMods(mod?:Mod){
  if(checkingMods||updating)return;const generation=++checkGeneration;const runId=crypto.randomUUID();checkingMods=true;modChecksDone=false;
  const candidates=mod?[mod]:libraryMods.filter(m=>!m.archive&&['nexus','curseforge'].includes(modOrigin(m).kind)&&!!(modOrigin(m).kind==='curseforge'?curseforgeKey:nexusKey).trim());
  try{for(const m of candidates){if(generation!==checkGeneration)break;checkingId=m.id;modChecks={...modChecks,[m.id]:undefined};modCheckErrors={...modCheckErrors,[m.id]:''};
   const provider=modOrigin(m).kind;const key=provider==='curseforge'?curseforgeKey:nexusKey;const logId=crypto.randomUUID(),started=Date.now();
   const logBase={id:logId,run_id:runId,timestamp:started,mod_id:m.id,mod_name:redactCheckLog(m.name,[nexusKey,curseforgeKey]),project:m.project,provider,duration_ms:0};
   try{const result=await invoke<ModUpdate>('check_mod_update',{id:m.id,apiKey:key.trim(),logId,runId});saveCheckLog({...logBase,status:result.status,code:result.status,duration_ms:Date.now()-started,detail:`${result.current_version} -> ${result.latest_version} | file #${result.file_id}`});if(generation===checkGeneration&&data?.mods.some(current=>current.id===m.id)){modChecks={...modChecks,[m.id]:result};if(result.status==='available')modSections[m.id]=true}}
   catch(e){const detail=redactCheckLog(String(e).replace(/^Error: /,''),[nexusKey,curseforgeKey]);saveCheckLog({...logBase,status:'failed',code:checkFailureCode(detail),duration_ms:Date.now()-started,detail});if(generation===checkGeneration){modCheckErrors={...modCheckErrors,[m.id]:detail};modSections[m.id]=true}}
  }}finally{if(generation===checkGeneration){checkingMods=false;checkingId='';modChecksDone=true;void loadCheckLog()}}
 }
 async function updateMod(m:Mod){await run(async()=>{const checked=modChecks[m.id];if(!checked?.token)return;
  if(jobs.some(job=>job.update?.id===m.id&&pendingDownload(job)))return;
  const info=await invoke<NexusInfo>('prepare_mod_update',{token:checked.token});jobs.push({info,status:'queued',received:0,total:null,update:{id:m.id,token:checked.token,name:m.name}});
 })}
 async function restoreMod(m:Mod){await run(async()=>{if(await confirm(t('restoreModConfirm',{name:m.name}),{title:t('restorePreviousMod'),okLabel:t('confirm'),cancelLabel:t('cancel')})){try{selected=await invoke<string>('restore_previous_mod',{id:m.id});modChecks={...modChecks,[m.id]:undefined}}finally{await refresh()}}})}
 function updateError(value:string){return errorText(value,['nexusApiKeyRequired','curseforgeApiKeyRequired'])}
 let updateStatus=$state<TranslationKey>('checkingUpdates'), appUpdateError=$state('');
 let input=$state<HTMLInputElement>();
 type FolderKey='mods_dir'|'library_dir'|'paks_dir';
 type ViewMode = 'grid' | 'list';
 function initialView():ViewMode {try{return localStorage.getItem('inzoimm.view')==='list'?'list':'grid'}catch{return 'grid'}}
 let view=$state<ViewMode>(initialView());
 let librarySort=$state<LibrarySort>(initialLibrarySort());
 function setLibrarySort(mode:LibrarySort){librarySort=mode;saveLibrarySort(mode)}
 let modItems=$state<HTMLDivElement>(), modList=$state<HTMLElement>();
 function setView(mode:ViewMode){view=mode;try{localStorage.setItem('inzoimm.view',mode)}catch{/* Keep the session preference. */}}
 async function chooseThumbnail(m:Mod){await run(async()=>{const path=await open({multiple:false,title:t('chooseThumbnail'),filters:[{name:t('imageFiles'),extensions:['png','jpg','jpeg','webp']}]});if(path){await invoke('set_mod_thumbnail',{id:m.id,path});await refresh()}})}
 let folder=$state(''), tagFilter=$state(''), manageSources=$state(false), sourceQuery=$state(''), legacyView=$state(false);
 const sourcesView=$derived(manageSources);
 const attachedSources=$derived(sourceFolders(data?.settings));
 const folderTree=$derived(folders(data?.settings,libraryMods));
 const currentFolder=$derived(folderTree.find(f=>pathKey(f.path)===pathKey(folder)));
 const childFolders=$derived(folder?folderTree.filter(f=>f.parent?pathKey(f.parent)===pathKey(folder):pathKey(normalizePath(f.path).split('/').slice(0,-1).join('/'))===pathKey(folder)):[]);
 const managedRoots=$derived(folderTree.filter(f=>f.parent===null&&f.kind!=='game'));
 function rootLabel(root:(typeof folderTree)[number]){return root.kind==='game'?t('gameModsFolder'):root.kind==='downloads'?t('downloads'):root.kind==='managed'&&data&&pathKey(root.path)===pathKey(data.settings.library_dir)?t('libraryFolder'):root.name}
 const sourceRows=$derived<ManagedSource[]>(managedRoots.map(root=>({...root,label:rootLabel(root),configurable:root.kind==='source'||(root.kind==='managed'&&!!data&&pathKey(root.path)===pathKey(data.settings.library_dir)),recursive:sourceScan(root.path)})));
 const allTags=$derived([...new Set(libraryMods.flatMap(m=>m.tags??[]))].sort((a,b)=>a.localeCompare(b)));
 $effect(()=>{if(data&&tagFilter&&!allTags.includes(tagFilter))tagFilter=''});
 type Location={filter:string;folder:string;query:string;tagFilter:string;selected:string;scroll:number;manageSources:boolean;legacy:boolean};
 let locations=$state<Location[]>([]), locationIndex=$state(0);
 const historySession=crypto.randomUUID();
 const canBack=$derived(locationIndex>0), canForward=$derived(locationIndex<locations.length-1);
 function location():Location{return{filter,folder,query,tagFilter,selected,manageSources,legacy:legacyView,scroll:modList?.scrollTop??0}}
 function navigate(next:Partial<Location>){
  closeContext();settingsOpen=false;
  if(locations.length)locations[locationIndex]=location();
  const nextLocation={...location(),...next,scroll:next.scroll??0};
  if(nextLocation.filter===filter&&pathKey(nextLocation.folder)===pathKey(folder)&&nextLocation.query===query&&nextLocation.tagFilter===tagFilter&&nextLocation.selected===selected&&nextLocation.manageSources===manageSources&&nextLocation.legacy===legacyView)return;
  locations=[...locations.slice(0,locationIndex+1),nextLocation];locationIndex=locations.length-1;
  window.history.pushState({inzoimm:historySession,index:locationIndex},'');restoreLocation(nextLocation);
 }
 function restoreLocation(point:Location){manageSources=point.manageSources;legacyView=point.legacy??false;filter=point.filter;folder=point.folder;query=point.query;tagFilter=point.tagFilter;selected=point.selected;requestAnimationFrame(()=>{if(modList)modList.scrollTop=point.scroll})}
 function historyMove(delta:number){if(settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||checkLogsOpen||metadataOpen||discoveryOpen||updating)return;closeContext();if(delta<0?!canBack:!canForward)return;locations[locationIndex]=location();window.history.go(delta)}
 function historyPop(e:PopStateEvent){if(e.state?.inzoimm!==historySession||!locations[e.state.index])return;locations[locationIndex]=location();locationIndex=e.state.index;closeContext();restoreLocation(locations[locationIndex])}
 function mouseNavigation(e:MouseEvent){if(e.button!==3&&e.button!==4)return;e.preventDefault();e.stopPropagation();if(e.type==='mouseup')historyMove(e.button===3?-1:1)}
 function browse(path:string){navigate({folder:path,filter:'all',selected:'',manageSources:false})}
 function chooseFilter(value:string){navigate({filter:value,folder:'',selected:'',manageSources:false,legacy:false})}
 function openLegacy(){navigate({filter:'all',folder:'',selected:'',manageSources:false,legacy:true})}
 const legacyMods=$derived(libraryMods.filter(m=>m.legacy));
 const showLegacyEntry=$derived(legacyMods.length>0||!!data?.settings.paks_dir);
 function managedTabKeys(e:KeyboardEvent){if(!['ArrowLeft','ArrowRight','Home','End'].includes(e.key))return;e.preventDefault();e.stopPropagation();const next=e.key==='Home'?false:e.key==='End'?true:!manageSources;navigate({manageSources:next,folder:''});requestAnimationFrame(()=>document.getElementById(next?'managed-sources-tab':'managed-mods-tab')?.focus())}
 async function scanComputer(paths?:string[],allDrives=true,drives?:string[]){await run(async()=>{
  discovery=null;scanProgress=null;scanCancelling=false;const id=crypto.randomUUID();scanRequestId=id;
  const onProgress=new Channel<DiscoveryProgress>();onProgress.onmessage=progress=>{if(scanRequestId===id)scanProgress=progress};
  try{discovery=await invoke<Discovery>('discover_mods',{paths:paths??null,allDrives,drives:allDrives&&!paths?drives??null:null,requestId:id,onProgress})}finally{if(scanRequestId===id)scanRequestId='';scanCancelling=false}
 })}
 async function stopDiscovery(){if(!scanRequestId||scanCancelling)return;scanCancelling=true;try{await invoke('cancel_discovery',{requestId:scanRequestId})}catch(e){scanCancelling=false;error=String(e)}}
 function closeDiscovery(){if(scanRequestId)void stopDiscovery();discoveryOpen=false}
 function openDiscovery(){if(busy||updating||!data)return;closeContext();discoveryOpen=true;discovery=null;scanProgress=null;void loadScanDrives()}
 async function loadScanDrives(){try{scanDrives=(await invoke<string[]>('list_scan_drives'))??[]}catch(e){scanDrives=[];error=String(e)}}
 async function chooseDiscoveryFolder(){if(busy||updating)return;try{const paths=await open({directory:true,multiple:true,title:t('scanChooseFolder')});if(paths)await scanComputer(typeof paths==='string'?[paths]:paths,false)}catch(e){error=String(e)}}
 async function importDiscoveredArchive(path:string){await run(async()=>{const report=await installArchive('import_zip',path);if(!report)return;await refresh();discoveryOpen=false;revealInstall(report)})}
 /** Installs an archive; asks before writing the inZOI Mod Enabler into the game. Returns null when declined. */
 async function installArchive(command:'import_zip'|'import_download',path:string,legacy=false):Promise<InstallReport|null>{
  const args=legacy?{path,legacy:true}:{path};
  const empty:InstallReport={imported:0,legacy:0,enabler:false,needs_enabler:false};
  try{return await invoke<InstallReport|null>(command,args)??empty}
  catch(e){if(!String(e).includes(enablerPrefix))throw e;if(!await confirm(t('modEnablerConfirm'),{title:t('modEnablerTitle'),okLabel:t('confirm'),cancelLabel:t('cancel')}))return null;return await invoke<InstallReport|null>(command,{...args,enabler:true})??empty}
 }
 async function openEnablerPage(){await run(async()=>{await openUrl(modEnablerUrl)})}
 /** For installs the user started: show the result where the new mods are listed. */
 function revealInstall(report:InstallReport){showInstallReport(report);if(report.legacy)openLegacy()}
 /** Installs a saved archive from the library; `legacy` puts its PAK files into ~mods instead. */
 async function installSavedArchive(mod:Mod,legacy=false){await run(async()=>{const report=await installArchive('import_download',mod.path,legacy);if(!report)return;await refresh();revealInstall(report)})}
 function showInstallReport(report:InstallReport){messageParams={count:report.legacy};message=report.enabler?'modEnablerInstalled':report.legacy?(report.needs_enabler?'legacyInstalledNoEnabler':'legacyInstalled'):'zipImported'}
 async function chooseModEnabler(){await run(async()=>{const p=await open({multiple:false,title:t('installModEnabler'),filters:[{name:t('zipArchive'),extensions:['zip','7z','rar']}]});if(p){await invoke('install_mod_enabler',{path:p});await refresh();messageParams={};message='modEnablerInstalled'}})}
 async function addDiscovered(paths:string[]){await run(async()=>{if(!data)return;const sources=sourceFolders(data.settings).map(source=>paths.some(path=>pathKey(path)===pathKey(source.path))?{...source,scan_subfolders:true}:source);for(const path of paths){if(![data.settings.mods_dir,data.settings.library_dir,...sources.map(s=>s.path)].some(root=>pathKey(root)===pathKey(path)))sources.push({path,scan_subfolders:true})}await invoke('save_settings',{settings:{...data.settings,scan_subfolders:data.settings.scan_subfolders||paths.some(path=>pathKey(path)===pathKey(data!.settings.library_dir)),source_dirs:sources}});await refresh();discoveryOpen=false;navigate({filter:'all',folder:'',selected:'',manageSources:false,legacy:false})})}
 function addSource(){if(busy||updating||!data)return;closeContext();sourcePaths=[];addingSource=true;importRecursive=true;importOpen=true}
 async function chooseSourceFolders(){await run(async()=>{const chosen=await open({directory:true,multiple:true,title:t('addSource')});if(chosen)sourcePaths=typeof chosen==='string'?[chosen]:chosen})}
 async function saveSourceFolders(){await run(async()=>{
  if(!data)return;const sources=sourceFolders(data.settings);
  for(const path of sourcePaths){if(![data.settings.mods_dir,data.settings.library_dir,downloadFolder(data.settings),...sources.map(s=>s.path)].some(root=>pathKey(root)===pathKey(path)))sources.push({path,scan_subfolders:importRecursive})}
  await invoke('save_settings',{settings:{...data.settings,source_dirs:sources}});await refresh();importOpen=false;navigate({filter:'all',folder:'',selected:'',manageSources:true});
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
   e.preventDefault();e.stopPropagation();if(settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||checkLogsOpen||metadataOpen||discoveryOpen)return;
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
   item('open',t('openFolder'),()=>void run(async()=>{await openFolder(modFolder(mod))}),{disabled:blocked});
   item('thumbnail',t('chooseThumbnail'),()=>void chooseThumbnail(mod),{disabled:blocked});
   item('tags',t('editTags'),()=>{selected=mod.id;requestAnimationFrame(()=>{const section=document.querySelector<HTMLDetailsElement>('.inspector-tags');if(section)section.open=true;const field=document.querySelector<HTMLInputElement>('.inspector .tag-form input');field?.focus();field?.scrollIntoView({block:'nearest'})})},{disabled:blocked});
   if(mod.archive&&/\.(zip|7z|rar)$/i.test(mod.path)){
    item('install',t('installArchive'),()=>void installSavedArchive(mod),{disabled:blocked,separator:true});
    if(data?.settings.paks_dir)item('install-legacy',t('installAsLegacy'),()=>void installSavedArchive(mod,true),{disabled:blocked});
   }
   if(!mod.archive){
    item('toggle',t(mod.enabled?'disableMod':'enableMod'),()=>void toggle(mod),{disabled:blocked,separator:true});
    if(['nexus','curseforge'].includes(modOrigin(mod).kind))item('check-update',t('checkModUpdate'),()=>void checkMods(mod),{disabled:blocked||checkingMods});
    if(modChecks[mod.id]?.status==='available')item('update',t('updateMod'),()=>void updateMod(mod),{disabled:blocked});
    if(mod.managed)item('edit',t('edit'),()=>editMod(mod),{disabled:blocked});
   }
   item('remove',t('remove'),()=>void remove(mod),{disabled:blocked,danger:true});
   webLinks(mod).forEach((url,index)=>item(`link-${index}`,url,()=>void run(async()=>{await openUrl(url)}),{disabled:blocked,separator:index===0}));
  }else if(target.kind==='folder'||target.kind==='source'){
   item('browse',t('open'),()=>browse(target.path));
   item('open',t('openFolder'),()=>void run(async()=>{await openFolder(target.path)}),{disabled:blocked});
   const configurable=target.kind==='source'&&(target.rootKind==='source'||(target.rootKind==='managed'&&data&&pathKey(target.path)===pathKey(data.settings.library_dir)));
   if(configurable)item('scan',t('scanSubfolders'),()=>void changeSourceScan(target.path,!sourceScan(target.path)),{disabled:blocked,checked:sourceScan(target.path),separator:true});
   item('refresh',t('refresh'),()=>void run(refresh),{disabled:blocked});
   if(target.kind==='source'&&target.rootKind==='source')item('detach',t('removeSource',{folder:folderName(target.path)}),()=>void removeSource(target.path),{disabled:blocked,separator:true});
  }else{
   item('back',t('back'),()=>historyMove(-1),{disabled:!canBack||updating});
   item('forward',t('forward'),()=>historyMove(1),{disabled:!canForward||updating});
   item('refresh',t('refresh'),()=>void run(refresh),{disabled:blocked,separator:true});
   item('source',t('addSource'),addSource,{disabled:blocked||!data});
   item('add',t('addMod'),()=>void add(),{disabled:blocked||!data});item('zip',t('importZip'),()=>void addZip(),{disabled:blocked||!data});
   item('grid',t('gridView'),()=>setView('grid'),{checked:view==='grid',separator:true});
   item('list',t('listView'),()=>setView('list'),{checked:view==='list'});
  }
  return items;
 });
 function inGame(mod:Mod){return !!data&&(inside(mod.path,data.settings.mods_dir)||!!data.settings.paks_dir&&inside(mod.path,data.settings.paks_dir))}
 const filtered=$derived(sortLibrary(libraryMods.filter(m=>(filter==='all'||filter==='enabled'&&m.enabled||filter==='disabled'&&!m.enabled||filter==='updates'&&modChecks[m.id]?.status==='available')&&(folder||legacyView===!!m.legacy)&&(!folder||(query.trim()||tagFilter?inside(m.path,folder):pathKey(normalizePath(m.path).split('/').slice(0,-1).join('/'))===pathKey(folder)||pathKey(m.path)===pathKey(folder)))&&(!tagFilter||(m.tags??[]).includes(tagFilter))&&`${m.name} ${m.author} ${m.project} ${m.path} ${(m.tags??[]).join(' ')} ${categoryLabel(categoryOf(m))}`.toLowerCase().includes(query.toLowerCase())),librarySort,modChecks,language.code));
 // Rendering, selection fallback and keyboard navigation all follow this one order.
 const groups=$derived(groupByCategory?groupMods(filtered,categoryOf,categoryLabel,language.code):[{category:{id:''} as ModCategory,mods:filtered}]);
 const ordered=$derived(groups.flatMap(group=>group.mods));
 const showGroups=$derived(groups.length>1);
 const errorMessage=$derived(errorText(error,['desktopRequired','nexusApiKeyRequired','nexusKeyLoadFailed','nexusKeySaveFailed','curseforgeKeyLoadFailed','curseforgeKeySaveFailed','curseforgeApiKeyRequired','nxmHandlerMissing']));
 const active=$derived(ordered.find(m=>m.id===selected)??ordered[0]);
 async function run(action:()=>Promise<void>){if(busy||updating)return false;busy=true;error='';message='';messageParams={};try{await action();return true}catch(e){error=String(e);return false}finally{busy=false}}
 async function refresh(){data=await invoke<Snapshot>('snapshot')}
 async function add(){await run(async()=>{const p=await open({directory:true,multiple:false,title:t('chooseImportFolder')});if(p){addingSource=false;importPath=p;importRecursive=data?.settings.scan_subfolders??true;importOpen=true}})}
 async function importFolder(){await run(async()=>{const report=await invoke<InstallReport|null>('import_folder',{path:importPath,scanSubfolders:importRecursive});await refresh();importOpen=false;if(report?.legacy)revealInstall(report);else message='folderImported'})}
 async function addZip(){await run(async()=>{const p=await open({multiple:false,filters:[{name:t('zipArchive'),extensions:['zip','7z','rar']}]});if(p){const report=await installArchive('import_zip',p);if(!report)return;await refresh();revealInstall(report)}})}
 function editMod(m:Mod){selected=m.id;editName=m.name;editDescription=m.description;editOpen=true}
 async function toggle(m:Mod){
  await run(async()=>{
   const enabled=!m.enabled;
   modToggle={name:m.name||m.project,enabled};
   try{
    try{selected=await invoke<string>('set_mod_enabled',{id:m.id,enabled})}
    finally{await refresh()}
   }finally{modToggle=null}
  });
 }
 async function remove(m:Mod){await run(async()=>{if(await confirm(t('removeConfirm',{name:m.name}),{title:t('removeTitle'),kind:'warning',okLabel:t('confirm'),cancelLabel:t('cancel')})){try{await invoke('remove_mod',{id:m.id})}finally{await refresh()}}})}
 async function checkAppUpdate(){
  try{return await check({timeout:20000})}
  catch(primary){try{const metadata=await invoke<ConstructorParameters<typeof Update>[0]|null>('check_app_update_fallback');return metadata?new Update(metadata):null}catch(fallback){throw new Error(`${String(primary)}; GitHub API: ${String(fallback)}`)}}
 }
 let checkingApp=$state(false), availableVersion=$state(''), updatePrompt=$state(false), installFailed=$state(false);
 const appIdle=()=>!(checkingMods||updating||busy||nexusKeySaving||curseforgeKeySaving||processing||pendingCount||settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||checkLogsOpen||metadataOpen||discoveryOpen);
 // Only checks. Installing closes and restarts the app, so that always waits for the user's choice.
 async function updates(){
  if(checkingApp||availableVersion||!appIdle())return;
  checkingApp=true;appUpdateError='';installFailed=false;updateStatus='checkingUpdates';
  try{
   const version=await invoke<boolean>('is_portable')?await invoke<string|null>('check_portable_update'):(update=await checkAppUpdate())?.version;
   if(version){availableVersion=version;updateStatus='newVersion';updatePrompt=true}else updateStatus='upToDate';
  }catch(e){updateStatus='updateUnavailable';appUpdateError=String(e).replace(/^Error: /,'')}finally{checkingApp=false}
 }
 function appUpdateClick(){if(availableVersion){appUpdateError='';updatePrompt=true}else void updates()}
 async function installAppUpdate(){
  if(updating||!availableVersion)return;
  updating=true;updatePrompt=false;appUpdateError='';installFailed=false;updateStatus='applyingUpdate';
  try{
   if(await invoke<boolean>('is_portable')){if(!await invoke<boolean>('update_portable')){availableVersion='';updateStatus='upToDate'}}
   else if(update){await update.downloadAndInstall(undefined,{timeout:600000,restartAfterInstall:true});await relaunch()}
  }catch(e){updateStatus='appUpdateFailed';installFailed=true;appUpdateError=String(e).replace(/^Error: /,'')}finally{updating=false}
 }
 function openNexusKeys(){void run(async()=>{await openUrl('https://www.nexusmods.com/settings/api-keys')})}
 async function receiveLink(raw:string){try{const info=await invoke<NexusInfo>('parse_nxm',{link:raw});downloadsOpen=false;showLink(info)}catch(e){error=String(e)}}
 onMount(()=>{locations=[location()];locationIndex=0;window.history.replaceState({inzoimm:historySession,index:0},'');if(!isTauri()){error='desktopRequired';return}
  void loadNexusKey();void loadCurseforgeKey();void loadCheckLog();void refreshNxmRegistration();
  refresh().then(()=>invoke<boolean>('init_paks_dir').then(changed=>changed?refresh():undefined).catch(()=>{})).catch(e=>error=String(e));void updates();const updateTimer=setInterval(()=>void updates(),60_000);invoke<{request_id:string;mod_id:number;file_id:number;url:string}[]>('pending_links').then(links=>{if(links.length){for(const info of links)showLink(info);}}).catch(e=>error=String(e));const progressUnlisten=listen<{request_id:string;received:number;total:number|null}>('nexus-download-progress',e=>{const job=jobs.find(j=>j.info.request_id===e.payload.request_id);if(job&&['downloading','cancelling'].includes(job.status))patchJob(e.payload.request_id,{received:e.payload.received,total:e.payload.total})});const unlisten=listen<{request_id:string;mod_id:number;file_id:number;url:string}>('nexus-link',e=>showLink(e.payload));getCurrent().then(urls=>{for(const u of urls??[])if(u.startsWith('nxm:'))void receiveLink(u)}).catch(e=>error=String(e));return()=>{clearInterval(updateTimer);void unlisten.then(f=>f());void progressUnlisten.then(f=>f())};
 });
 function focusDialog(node:HTMLElement){const previous=document.activeElement as HTMLElement|null;const controls=()=>Array.from(node.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),[tabindex="0"]')).filter(el=>el.offsetParent!==null);queueMicrotask(()=>(node.querySelector<HTMLElement>('[data-autofocus]:not(:disabled)')??controls()[0])?.focus());const trap=(e:KeyboardEvent)=>{if(e.key!=='Tab')return;const all=controls();if(!all.length){e.preventDefault();return}const first=all[0],last=all[all.length-1];if(e.shiftKey&&(document.activeElement===first||document.activeElement===node||!node.contains(document.activeElement))){e.preventDefault();last.focus()}else if(!e.shiftKey&&(document.activeElement===last||!node.contains(document.activeElement))){e.preventDefault();first.focus()}};node.addEventListener('keydown',trap);return{destroy(){node.removeEventListener('keydown',trap);if(previous?.isConnected)previous.focus()}};}
 async function saveSettings(){
  if(!draft)return;
  const settings={...draft,staging_dir:draft.staging_dir?.trim()??'',paks_dir:draft.paks_dir??''};
  await run(async()=>{
   const previous=data?.settings;
   stagingSaving=pathKey(settings.staging_dir)!==pathKey(previous?.staging_dir??'')
    || (!settings.staging_dir&&pathKey(settings.mods_dir)!==pathKey(previous?.mods_dir??''));
   try{await invoke('save_settings',{settings});await refresh();settingsOpen=false}
   finally{stagingSaving=false}
  });
 }
 async function chooseStagingFolder(){
  await run(async()=>{
   const folder=await open({directory:true,title:t('stagingLocation')});
   if(folder&&draft)draft.staging_dir=folder;
  });
 }
 function openSettings(){if(updating||onboardingOpen)return;closeDiscovery();metadataOpen=false;checkLogsOpen=false;draft=data?{...data.settings,scan_subfolders:data.settings.scan_subfolders??true}:null;editOpen=false;dismissLink();downloadsOpen=false;importOpen=false;settingsOpen=true;void refreshNxmRegistration();queueMicrotask(()=>document.querySelector<HTMLElement>('.settings-page select')?.focus());}
 function keyboard(e:KeyboardEvent){if(e.altKey&&['ArrowLeft','ArrowRight'].includes(e.key)){e.preventDefault();historyMove(e.key==='ArrowLeft'?-1:1);return}if(context)return;if((e.ctrlKey||e.metaKey)&&e.key===','){e.preventDefault();openSettings();return}if((e.ctrlKey||e.metaKey)&&e.key==='k'){e.preventDefault();if(!settingsOpen&&!editOpen&&!linkOpen&&!importOpen&&!downloadsOpen&&!checkLogsOpen&&!metadataOpen&&!discoveryOpen)input?.focus();return}if(e.key==='Escape'){closeDiscovery();settingsOpen=false;dismissLink();editOpen=false;importOpen=false;downloadsOpen=false;checkLogsOpen=false;metadataOpen=false}if(settingsOpen||editOpen||linkOpen||importOpen||downloadsOpen||checkLogsOpen||metadataOpen||discoveryOpen||sourcesView)return;if(e.target instanceof HTMLInputElement||e.target instanceof HTMLTextAreaElement||e.target instanceof HTMLSelectElement)return;if(['ArrowDown','ArrowUp',...(view==='grid'?['ArrowLeft','ArrowRight']:[])].includes(e.key)){
  e.preventDefault();const i=ordered.findIndex(m=>m.id===active?.id);
  const cards=Array.from(modItems?.querySelectorAll<HTMLButtonElement>('.mod-row')??[]);
  let next=i<0?0:i+(e.key==='ArrowDown'||e.key==='ArrowRight'?1:-1);
  // Category groups start new rows, so move to the nearest card on the next visual row.
  if(view==='grid'&&i>=0&&cards[i]&&(e.key==='ArrowDown'||e.key==='ArrowUp')){
   const from=cards[i].getBoundingClientRect(),down=e.key==='ArrowDown',centre=from.left+from.width/2;
   const candidates=cards.map((card,index)=>({index,rect:card.getBoundingClientRect()})).filter(({rect})=>down?rect.top>from.top+1:rect.top<from.top-1);
   const row=candidates.length?(down?Math.min:Math.max)(...candidates.map(c=>c.rect.top)):from.top;
   next=candidates.filter(c=>c.rect.top===row).sort((a,b)=>Math.abs(a.rect.left+a.rect.width/2-centre)-Math.abs(b.rect.left+b.rect.width/2-centre))[0]?.index??i;
  }
  next=Math.max(0,Math.min(ordered.length-1,next));selected=ordered[next]?.id??'';
  cards[next]?.scrollIntoView({block:'nearest'});
  if(e.target instanceof HTMLElement&&e.target.closest('.mod-row'))cards[next]?.focus();
 }}
</script>
<svelte:window onkeydown={keyboard} onpopstate={historyPop} onmousedown={mouseNavigation} onmouseup={mouseNavigation} onauxclick={mouseNavigation} onfocus={refreshNxmRegistration}/>
<div class="shell" use:contextTarget={{kind:'library'}}>
 <aside>
  <div class="aside-content">
  <div class="brand"><img src="/app-icon.png" alt=""/><div>inZOI<span>Better Mod Manager</span></div></div>
  <button class:chosen={!sourcesView&&!legacyView&&!settingsOpen} onclick={()=>chooseFilter('all')}><IconBox size={18}/>{t('allMods')}<span class="all-mods-counts"><small>{libraryMods.length-legacyMods.length}</small>{#if modUpdateCount}<small class="mod-update-count" aria-label={t('modUpdatesFilter',{count:modUpdateCount})} title={t('modUpdatesFilter',{count:modUpdateCount})}>{modUpdateCount}</small>{/if}</span></button>
  {#if showLegacyEntry}<button class="legacy-entry" class:chosen={!sourcesView&&legacyView&&!settingsOpen} onclick={openLegacy}><IconArchive size={18}/>{t('legacyPakMods')}{#if data?.mod_enabler===false&&legacyMods.length}<small class="legacy-warning" aria-label={t('modEnablerStatusMissing')} title={t('modEnablerStatusMissing')}>!</small>{/if}<small>{legacyMods.length}</small></button>{/if}
  <div class="aside-row">
   <button class="aside-row-main" title={modChecksDone?t('modCheckSummary',{count:modUpdateCount,failed:libraryMods.filter(m=>!!modCheckErrors[m.id]).length}):undefined} disabled={checkingMods||updating||!data||!nexusKeyReady||!curseforgeKeyReady} onclick={()=>checkMods()}><IconRefresh size={18} class={checkingMods?'spin':''}/><span class="aside-row-label">{t(checkingMods?'checkingModUpdates':'checkModUpdates')}</span>{#if modUpdateCount}<small>{modUpdateCount}</small>{/if}</button>
   <button class="icon check-log-link" aria-label={t('viewCheckLog')} title={t('viewCheckLog')} disabled={updating} onclick={openCheckLog}><IconHistory size={16}/></button>
  </div>
  </div>
  <div class="aside-bottom"><button class="downloads-button" bind:this={downloadsButton} class:chosen={downloadsOpen} disabled={updating} onclick={openDownloads}><IconDownload size={18}/>{t('downloads')}{#if pendingCount}<small class="download-count">{pendingCount}</small>{:else if jobs.some(job=>job.status==='failed'||job.status==='importFailed')}<small class="download-count" aria-label={t('downloadFailed')}>!</small>{/if}</button>
   <button disabled={updating} title="Ctrl ," class:chosen={settingsOpen} aria-label={t('settings')} aria-describedby={settingsCount?'settings-count':undefined} aria-current={settingsOpen?'page':undefined} onclick={openSettings}>
    <IconSettings size={18}/>{t('settings')}{#if settingsCount}<small id="settings-count" class="settings-count" role="status" aria-label={settingsAttention} title={settingsAttention}>{settingsCount}</small>{/if}
   </button>
   <div class="aside-meta"><button class="app-update-button" aria-label={t('appUpdates')} title={`${t('appUpdates')}: ${t(updateStatus,{version:availableVersion})}`} onclick={appUpdateClick} disabled={busy||updating||checkingApp||processing||pendingCount>0}><IconRefresh size={13} class={checkingApp?'spin':''}/><span class="update-status" class:sr-only={updateStatus==='upToDate'||updateStatus==='checkingUpdates'}>{t(updateStatus,{version:availableVersion})}</span></button><AppInfo/></div></div>
 </aside>
 <main class:settings-main={settingsOpen}>
  {#if settingsOpen}<section class="settings-page" aria-label={t('settings')}>
  <div class="settings-head"><button class="icon" aria-label={t('backToLibrary')} title={t('backToLibrary')} onclick={()=>settingsOpen=false}><IconArrowLeft size={18}/></button><div class="settings-title"><h1>{t('librarySettings')}</h1><p>{t('settingsHint')}</p></div><button class="primary" disabled={busy||updating||!draft} onclick={()=>void saveSettings()}>{t('saveSettings')}</button></div>
  {#if error}<div class="notice error" role="alert">{t('operationFailed')}: {errorMessage}<button class="icon" onclick={()=>error=''} aria-label={t('closeError')}><IconX size={16}/></button></div>{/if}
  {#if message}<div class="notice" role="status">{t(message as TranslationKey,{version:update?.version??'',...messageParams})}</div>{/if}
  <div class="settings-body">
   <div class="settings-column">{#if draft}<section class="settings-card" aria-labelledby="settings-folders"><h2 id="settings-folders">{t('settingsFolders')}</h2>{#each [{key:'mods_dir' as FolderKey,name:t('gameModsFolder')},{key:'library_dir' as FolderKey,name:t('libraryFolder')}] as f}<label>{f.name}<div class="path-input"><input id={f.key} bind:value={draft[f.key]}/><button title={t('chooseFolder')} onclick={()=>void run(async()=>{const p=await open({directory:true});if(p&&draft)draft[f.key]=f.key==='paks_dir'?(await invoke<string|null>('detect_paks_dir',{picked:p}))??p:p})}><IconFolder size={18}/></button>{#if f.key==='paks_dir'}<button title={t('detectPaks')} aria-label={t('detectPaks')} onclick={()=>void run(async()=>{const p=await invoke<string|null>('detect_paks_dir',{});if(p&&draft)draft.paks_dir=p;else message='pakNotFound'})}><IconSearch size={18}/></button>{/if}</div></label>{#if f.key==='paks_dir'}<p class="muted">{t('pakModsHint')}</p>{#if data?.mod_enabler!=null&&draft.paks_dir===data.settings.paks_dir}<div class="mod-enabler-status"><span class:ok={data.mod_enabler}>{t(data.mod_enabler?'modEnablerStatusOk':'modEnablerStatusMissing')}</span>{#if data.game_dir}<button onclick={()=>void run(async()=>{await openFolder(data!.game_dir!)})}>{t('openGameFolder')}</button>{/if}<button onclick={()=>void openEnablerPage()}>{t('downloadModEnabler')}</button><button disabled={busy||updating} onclick={()=>void chooseModEnabler()}>{t('installModEnabler')}</button></div>{/if}{/if}{#if f.key==='library_dir'}<label class="checkbox-option"><input type="checkbox" bind:checked={draft.scan_subfolders}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('managedFolderHint')}</p>{/if}{/each}<label>{t('downloadFolder')}<div class="path-input"><input id="downloads_dir" bind:value={draft.downloads_dir} placeholder={downloadFolder({...draft,downloads_dir:''})}/><button title={t('chooseDownloadFolder')} onclick={()=>void run(async()=>{const p=await open({directory:true,title:t('downloadFolder')});if(p&&draft)draft.downloads_dir=p})}><IconFolder size={18}/></button></div></label><p class="muted">{t('downloadFolderHint')}</p>  <label for="staging_dir">{t('stagingLocation')}</label>
    <div class="path-input">
     <input id="staging_dir" aria-describedby="staging-location-hint" bind:value={draft.staging_dir} placeholder={draft.mods_dir.replace(/[\\/]+$/,'').replace(/[^\\/]+$/,'')} disabled={busy||updating}/>
     <button title={t('chooseFolder')} aria-label={t('stagingLocation')} disabled={busy||updating} onclick={()=>void chooseStagingFolder()}><IconFolder size={18}/></button>
    </div>
    <p id="staging-location-hint" class="muted">{t('stagingLocationHint')}</p>
   </section><section class="settings-card" aria-labelledby="settings-legacy"><h2 id="settings-legacy">{t('legacyPakMods')}</h2>{#each [{key:'paks_dir' as FolderKey,name:t('pakModsFolder')}] as f}<label>{f.name}<div class="path-input"><input id={f.key} bind:value={draft[f.key]}/><button title={t('chooseFolder')} onclick={()=>void run(async()=>{const p=await open({directory:true});if(p&&draft)draft[f.key]=f.key==='paks_dir'?(await invoke<string|null>('detect_paks_dir',{picked:p}))??p:p})}><IconFolder size={18}/></button>{#if f.key==='paks_dir'}<button title={t('detectPaks')} aria-label={t('detectPaks')} onclick={()=>void run(async()=>{const p=await invoke<string|null>('detect_paks_dir',{});if(p&&draft)draft.paks_dir=p;else message='pakNotFound'})}><IconSearch size={18}/></button>{/if}</div></label>{#if f.key==='paks_dir'}<p class="muted">{t('pakModsHint')}</p>{#if data?.mod_enabler!=null&&draft.paks_dir===data.settings.paks_dir}<div class="mod-enabler-status"><span class:ok={data.mod_enabler}>{t(data.mod_enabler?'modEnablerStatusOk':'modEnablerStatusMissing')}</span>{#if data.game_dir}<button onclick={()=>void run(async()=>{await openFolder(data!.game_dir!)})}>{t('openGameFolder')}</button>{/if}<button onclick={()=>void openEnablerPage()}>{t('downloadModEnabler')}</button><button disabled={busy||updating} onclick={()=>void chooseModEnabler()}>{t('installModEnabler')}</button></div>{/if}{/if}{#if f.key==='library_dir'}<label class="checkbox-option"><input type="checkbox" bind:checked={draft.scan_subfolders}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('managedFolderHint')}</p>{/if}{/each}</section>{/if}</div>
   <div class="settings-column">
    <section class="settings-card" aria-labelledby="settings-general">
     <h2 id="settings-general">{t('settingsGeneral')}</h2>
     <label>{t('language')}<select value={language.code} onchange={e=>setLanguage(e.currentTarget.value)}>{#each languages as option}<option value={option.code}>{option.name}</option>{/each}</select></label>
     <p class="muted">{t('languageHint')}</p>
     <div class="nxm-setting">
      <p class="nxm-status" class:active={nxmRegistered===true} class:missing={nxmRegistered===false} id="nxm-status" role="status">
       {t(nxmRegistered===true?'nxmHandlerActive':nxmRegistered===false?'nxmHandlerMissing':'nxmHandlerUnknown')}
      </p>
      <button class="wide" aria-describedby="nxm-status" disabled={busy||updating} onclick={registerNxm}>{t('setNxmHandler')}</button>
     </div>
     <p class="muted">{t('updateHint')}</p>
    </section>
    <section class="settings-card" aria-labelledby="settings-accounts"><h2 id="settings-accounts">{t('settingsAccounts')}</h2><NexusApiKey bind:value={nexusKey} disabled={busy||updating||!nexusKeyReady} saving={nexusKeySaving} onopen={openNexusKeys} onchange={changeNexusKey}/><NexusApiKey provider="curseforge" bind:value={curseforgeKey} disabled={busy||updating||!curseforgeKeyReady} saving={curseforgeKeySaving} onopen={()=>void run(async()=>{await openUrl(CURSEFORGE_KEYS_URL)})} onchange={changeCurseforgeKey}/></section>
   </div>
  </div>
 </section>{:else}
  <header><div class="navigation-buttons" role="group" aria-label={t('navigation')}><button class="icon" title={t('back')} aria-label={t('back')} disabled={!canBack||updating} onclick={()=>historyMove(-1)}><IconArrowLeft size={18}/></button><button class="icon" title={t('forward')} aria-label={t('forward')} disabled={!canForward||updating} onclick={()=>historyMove(1)}><IconArrowRight size={18}/></button></div><div class="search"><IconSearch size={20}/><input bind:this={input} title="Ctrl K" value={sourcesView?sourceQuery:query} oninput={e=>{if(sourcesView)sourceQuery=e.currentTarget.value;else query=e.currentTarget.value}} placeholder={t(sourcesView?'searchSources':'searchPlaceholder')} aria-label={t(sourcesView?'searchSources':'search')}/>{#if sourcesView&&sourceQuery}<button class="icon" aria-label={t('clearSourceSearch')} onclick={()=>sourceQuery=''}><IconX size={15}/></button>{/if}</div><button class="icon" title={t('refresh')} disabled={busy||updating} onclick={()=>void run(refresh)}><IconRefresh size={18} class={busy&&!operationLabel?'spin':''}/></button></header>
  <div class="page-title managed-title"><div class="library-heading"><h1 title={sourcesView?t('sourceFolders'):folder?currentFolder?.name??folderName(folder):legacyView?t('legacyPakMods'):t('yourLibrary')}>{sourcesView?t('sourceFolders'):folder?currentFolder?.name??folderName(folder):legacyView?t('legacyPakMods'):t('yourLibrary')}</h1><p>{sourcesView?t('sourceCount',{count:managedRoots.length}):t('folderModCount',{count:filtered.length})}</p>{#if !sourcesView}<button class="icon library-help" aria-label={t('managedCollectionHint')}><IconInfoCircle size={16}/><span class="library-help-tooltip" aria-hidden="true">{t('managedCollectionHint')}</span></button>{/if}</div><div class="actions"><button class="icon" aria-label={t('scanComputer')} title={t('scanComputer')} disabled={busy||updating||!data} onclick={openDiscovery}><IconZoomScan size={17}/></button>{#if sourcesView}<button class="primary" disabled={busy||updating||!data} onclick={addSource}><IconPlus size={17}/>{t('addSource')}</button>{:else}<button class="icon" aria-label={t('importZip')} title={t('importZip')} disabled={busy||updating||!data} onclick={addZip}><IconFileZip size={17}/></button><button class="primary" disabled={busy||updating||!data} onclick={add}><IconPlus size={17}/>{t('addMod')}</button>{/if}</div></div>
  {#if modChecksDone&&libraryMods.some(m=>!!modCheckErrors[m.id])}<div class="notice error check-log-notice"><span>{t('checkLogFailedCount',{count:libraryMods.filter(m=>!!modCheckErrors[m.id]).length})}</span><button onclick={openCheckLog}>{t('viewCheckLog')}</button></div>{/if}
  {#if updatePrompt&&availableVersion&&!appUpdateError}<div class="notice app-update-notice" role="status" aria-label={t('appUpdates')}><span><strong>{t('newVersion',{version:availableVersion})}</strong><small>{t('appUpdateRestartHint')}</small></span><button disabled={busy||updating||checkingApp||processing||pendingCount>0} onclick={()=>void installAppUpdate()}>{t('installUpdate')}</button><button onclick={()=>updatePrompt=false}>{t('updateLater')}</button></div>{/if}
  {#if appUpdateError}<div class="notice app-update-notice" role="status" aria-label={t('appUpdates')}><span><strong>{t(updateStatus)}</strong><small>{t('appUpdateLibraryAvailable')}</small><small class="app-update-detail">{appUpdateError}</small></span><button disabled={busy||updating||checkingApp||processing||pendingCount>0} onclick={()=>void (installFailed?installAppUpdate():updates())}>{t('retryAppUpdate')}</button><button class="icon" aria-label={t('close')} onclick={()=>appUpdateError=''}><IconX size={16}/></button></div>{/if}
  {#if error}<div class="notice error" role="alert">{t('operationFailed')}: {errorMessage}<button class="icon" onclick={()=>error=''} aria-label={t('closeError')}><IconX size={16}/></button></div>{/if}
  {#if message}<div class="notice" role="status">{t(message as TranslationKey,{version:update?.version??'',...messageParams})}</div>{/if}
  {#if data?.mod_enabler===false&&!enablerNoticeHidden&&!legacyView&&legacyMods.length}<div class="notice mod-enabler-notice" role="status"><span>{t('modEnablerMissingNotice')}</span><button onclick={()=>void openEnablerPage()}>{t('downloadModEnabler')}</button><button disabled={busy||updating} onclick={()=>void chooseModEnabler()}>{t('installModEnabler')}</button><button class="icon" aria-label={t('close')} onclick={()=>enablerNoticeHidden=true}><IconX size={16}/></button></div>{/if}
  {#if updating&&update}<div class="notice" role="status">{t('applyingUpdate')}</div>{/if}
  {#if data?.warnings.length}<details class="warnings"><summary>{t('scanWarnings',{count:data.warnings.length})}</summary>{#each data.warnings as warning}<p>{warning}</p>{/each}</details>{/if}
  {#if legacyView&&!sourcesView&&!folder}<section class="legacy-panel" aria-label={t('modEnablerTitle')}><div><strong class:ok={data?.mod_enabler===true} class:missing={data?.mod_enabler===false}>{t(data?.mod_enabler===true?'modEnablerStatusOk':data?.mod_enabler===false?'modEnablerStatusMissing':'modEnablerNoFolder')}</strong><p>{t('legacyPanelHint')}</p></div><div class="legacy-panel-actions">{#if data?.game_dir}<button disabled={busy||updating} onclick={()=>void run(async()=>{await openFolder(data!.game_dir!)})}><IconFolder size={15}/>{t('openGameFolder')}</button>{/if}<button onclick={()=>void openEnablerPage()}><IconArrowUpRight size={15}/>{t('downloadModEnabler')}</button>{#if data?.mod_enabler==null}<button onclick={openSettings}><IconSettings size={15}/>{t('settings')}</button>{:else}<button disabled={busy||updating} onclick={()=>void chooseModEnabler()}>{t('installModEnabler')}</button>{/if}</div></section>{/if}
  <div class="managed-tabs" role="tablist" tabindex="-1" aria-label={t('library')} onkeydown={managedTabKeys}><button id="managed-mods-tab" role="tab" aria-selected={!manageSources} aria-controls="managed-mods-panel" tabindex={manageSources?-1:0} onclick={()=>navigate({manageSources:false})}>{t('modsTab')}</button><button id="managed-sources-tab" role="tab" aria-selected={manageSources} aria-controls="managed-sources-panel" tabindex={manageSources?0:-1} onclick={()=>navigate({manageSources:true,folder:''})}>{t('sourceFolders')}<span>{managedRoots.length}</span></button></div>
  {#if sourcesView}<div class="sources-workspace" id="managed-sources-panel" role="tabpanel" aria-labelledby="managed-sources-tab"><SourceManager sources={sourceRows} search={sourceQuery} disabled={busy||updating} contextAction={contextTarget} onbrowse={browse} onopen={path=>void run(async()=>{await openFolder(path)})} onremove={path=>void removeSource(path)} onscan={changeSourceScan}/></div>{:else}
  <div class="workspace" id="managed-mods-panel" role="tabpanel" aria-labelledby="managed-mods-tab">
  <div class="library-toolbar">
   <div class="library-scopes" class:has-tags={allTags.length>0}>
    <div class="source-scope"><IconFolder size={14}/><select aria-label={t('browseSource')} value={currentFolder?.root??''} onchange={e=>{if(e.currentTarget.value)browse(e.currentTarget.value);else navigate({folder:'',selected:'',manageSources:false})}}><option value="">{t('allSources')}</option>{#each folderTree.filter(f=>f.parent===null) as root}<option value={root.path}>{rootLabel(root)}</option>{/each}</select></div>
    <select class="status-filter" aria-label={t('status')} title={t('status')} value={filter} onchange={e=>navigate({filter:e.currentTarget.value})}><option value="all">{t('allMods')}</option><option value="enabled">{t('enabled')}</option><option value="disabled">{t('disabled')}</option><option value="updates">{t('modUpdatesFilter',{count:modUpdateCount})}</option></select>
    <div class="library-sort" title={t('sortBy')}><IconArrowsSort size={14}/><select aria-label={t('sortBy')} value={librarySort} onchange={e=>setLibrarySort(e.currentTarget.value as LibrarySort)}><option value="recommended">{t('sortRecommended')}</option><option value="newest">{t('sortNewest')}</option><option value="oldest">{t('sortOldest')}</option><option value="nameAsc">{t('sortNameAsc')}</option><option value="nameDesc">{t('sortNameDesc')}</option></select></div>
    {#if allTags.length}<div class="tag-scope" class:active={!!tagFilter} title={t('filterTags')}><IconTag size={14}/><select aria-label={t('filterTags')} bind:value={tagFilter}><option value="">{t('allTags')}</option>{#each allTags as tag}<option value={tag}>{tag}</option>{/each}</select></div>{/if}
   </div>
   <div class="view-switch" role="group" aria-label={t('viewLayout')}>
    <button class="icon group-toggle" class:chosen={groupByCategory} aria-label={t('groupByCategory')} title={t('groupByCategory')} aria-pressed={groupByCategory} onclick={()=>setGrouping(!groupByCategory)}><IconStack2 size={16}/></button>
    <span class="view-switch-divider" aria-hidden="true"></span>
    <button class="icon" class:chosen={view==='grid'} aria-label={t('gridView')} title={t('gridView')} aria-pressed={view==='grid'} onclick={()=>setView('grid')}><IconLayoutGrid size={16}/></button>
    <button class="icon" class:chosen={view==='list'} aria-label={t('listView')} title={t('listView')} aria-pressed={view==='list'} onclick={()=>setView('list')}><IconList size={16}/></button>
   </div>
  </div>
  <section class="mod-list" bind:this={modList} aria-label={t('modList')}>
  {#if folder}<div class="folder-breadcrumb"><button onclick={()=>navigate({folder:'',selected:''})}>{t('allMods')}</button><IconChevronRight size={12}/><span title={folder}>{folder}</span>{#if currentFolder?.parent}<button class="icon" title={t('parentFolder')} aria-label={t('parentFolder')} onclick={()=>browse(currentFolder!.parent!)}>↑</button>{/if}</div>{/if}
  {#if !query.trim()&&!tagFilter&&folder&&childFolders.length}<div class="folder-cards">{#each childFolders as entry}<button title={entry.path} use:contextTarget={{kind:'folder',path:entry.path}} onclick={()=>browse(entry.path)}><IconFolder size={23}/><span>{entry.parent===null?rootLabel(entry):entry.name}<small>{t('folderModCount',{count:entry.count})}</small></span><IconChevronRight size={14}/></button>{/each}</div>{/if}
  <div class="mod-items" class:mod-grid={view==='grid'} bind:this={modItems}>
  {#each groups as group (group.category.id)}
  {#if showGroups}<h3 class="mod-group-head"><span>{categoryLabel(group.category)}</span><small>{group.mods.length}</small></h3>{/if}
  {#each group.mods as mod (mod.id)}
   {@const origin=modOrigin(mod)}
   <div class="mod-card"><button class="mod-row" use:contextTarget={{kind:'mod',id:mod.id}} class:selected={active?.id===mod.id} aria-pressed={active?.id===mod.id} onclick={()=>selected=mod.id}>
    <div class="thumb"><ModThumbnail path={mod.thumbnail} name={mod.name||mod.project} large={view==='grid'}/></div>
    <div class="mod-title"><strong title={mod.name||mod.project}>{mod.name||mod.project}</strong><span>{mod.author||t('unknownAuthor')}</span><span class="mod-origin" class:sr-only={origin.kind==='local'&&!mod.legacy} data-provider={origin.kind} title={`${t('modSource')}: ${origin.label||t('localFiles')}`}>{mod.legacy?t('legacyPak'):origin.label||t('localFiles')}</span>{#if modChecks[mod.id]?.status==='available'}<span class="mod-update-badge" title={`${t('modUpdateAvailable')} · ${modChecks[mod.id]?.latest_version||t('latestRelease')}`}>{t('modUpdateAvailable')}</span>{/if}{#if mod.tags?.length}<span class="mod-tags" title={mod.tags.join(' · ')}>{mod.tags.join(' · ')}</span>{/if}</div>
    <span class:enabled={mod.enabled} class="status" class:sr-only={!mod.archive}>{mod.archive?t('downloadedArchive'):mod.enabled?t('enabled'):t('disabled')}</span>
   </button>
   {#if !mod.archive}<button class="mod-toggle" role="switch" aria-checked={mod.enabled} aria-label={`${t(mod.enabled?'disableMod':'enableMod')}: ${mod.name||mod.project}`} title={t(mod.enabled?'disableMod':'enableMod')} disabled={busy||updating} onclick={()=>toggle(mod)}><span></span></button>{/if}
   </div>
  {/each}
  {/each}
  </div>
  {#if !data&&!error}<div class="empty" role="status"><IconRefresh size={30} stroke={1.5} class="spin"/><h2>{t('loadingLibrary')}</h2><p>{t('loadingLibraryHint')}</p></div>{:else if filtered.length===0&&(!childFolders.length||query.trim()||tagFilter)}<div class="empty"><IconBox size={42} stroke={1}/><h2>{query?t('noResults'):legacyView&&!folder?t('legacyEmptyTitle'):t('emptyTitle')}</h2><p>{query?t('noResultsHint'):legacyView&&!folder?t('legacyEmptyHint'):t('emptyHint')}</p>{#if !query&&legacyView&&!folder}<button onclick={addZip} disabled={!data||busy}>{t('importZip')} <IconArrowUpRight size={16}/></button>{:else if !query}<button onclick={add} disabled={!data||busy}>{t('chooseModFolder')} <IconArrowUpRight size={16}/></button>{/if}</div>{/if}
  </section>
  <ResizableInspector>
   <section class="inspector" id="mod-inspector" use:contextTarget={active?{kind:'mod',id:active.id}:{kind:'library'}} aria-label={t('modDetails')}>
    {#if active}
     {#key active.id}
      <div class="inspector-scroll">
       <div class="inspector-identity">
        <div class="cover"><ModThumbnail path={active.thumbnail} name={active.name||active.project} large/></div>
        <div class="identity-copy">
        <div class="inspector-heading"><span class="eyebrow">{modOrigin(active).label||t('localFiles')}</span><button class="icon" aria-label={t('viewModMetadata')} title={t('viewModMetadata')} disabled={updating} onclick={()=>{selected=active!.id;metadataOpen=true}}><IconMaximize size={16}/></button></div>
        <h2>{active.name||active.project}</h2>
        <p class="author">{t('byAuthor',{author:active.author||t('unknownAuthor')})}</p>
        <span class="inspector-status">{active.archive?t('notInstalled'):active.enabled?t('enabled'):t('disabled')}</span>
        </div>
       </div>
       <div class="inspector-body">
        <details class="inspector-section"><summary><IconChevronRight size={14}/><span>{t('description')}</span></summary><div class="section-content"><RichDescription text={modDescription(active)||t('noDescription')} onopen={url=>void run(async()=>{await openUrl(url)})}/></div></details>
        <details class="inspector-section"><summary><IconChevronRight size={14}/><span>{t('modDetails')}</span></summary><div class="section-content">
         <dl class="mod-facts"><div><dt>{t('version')}</dt><dd>{active.version||'—'}</dd></div><div><dt>{t('status')}</dt><dd>{active.archive?t('notInstalled'):active.enabled?t('enabled'):t('disabled')}</dd></div>{#if !active.archive}<div><dt>{t('managedBy')}</dt><dd>{active.managed?'Better Mod Manager':t('outsideApp')}</dd></div>{/if}</dl>
         {#if active.archive}<p class="external-note">{t('archiveLibraryHint')}</p>{:else if active.legacy}<p class="external-note">{t('legacyNote')}</p>{:else if !active.managed}<p class="external-note">{inGame(active)?t('externalNote'):t('sourceNote')}</p>{/if}
        </div></details>
        <ModMetadata mod={active} disabled={busy||updating} onopen={url=>void run(async()=>{await openUrl(url)})} ontags={tags=>saveTags(active!,tags)}/>
        {#if !active.archive&&(['nexus','curseforge'].includes(modOrigin(active).kind)||active.previous_version)}
         {@const checked=modChecks[active.id]}
         {@const updateModId=active.id}
         <details class="inspector-section mod-update-section" open={modSections[updateModId]??(checked?.status==='available'||!!modCheckErrors[updateModId])} ontoggle={e=>modSections[updateModId]=e.currentTarget.open}>
          <summary>{t('modUpdates')}{#if checked?.status==='available'}<span>{t('modUpdateAvailable')}</span>{/if}</summary>
          <p class="muted">{t('currentModVersion')}: {checked?.current_version||active.version||'—'}{#if checked?.latest_version}<br/>{t('latestModVersion')}: {checked.latest_version}{/if}</p>
          {#if checked?.status==='upToDate'}<p class="muted">{t('modUpToDate')}</p>{:else if checked?.status==='manual'}<p class="muted">{t('modUpdateManualHint')}</p>{:else if checked?.status==='unsupported'}<p class="muted">{t('modUpdateUnsupported')}</p>{/if}
          {#if modCheckErrors[active.id]}<p class="form-error" role="alert">{updateError(modCheckErrors[active.id])}</p>{/if}
          <button class="wide" disabled={busy||updating||checkingMods} onclick={()=>checkMods(active!)}><IconRefresh size={15} class={checkingId===active.id?'spin':''}/>{t(checkingId===active.id?'checkingModUpdates':'checkModUpdate')}</button>
          {#if checked?.status==='available'}<button class="wide primary" disabled={busy||updating||jobs.some(job=>job.update?.id===active!.id&&pendingDownload(job))} onclick={()=>updateMod(active!)}><IconDownload size={15}/>{t('updateMod')}</button><p class="muted">{t('modUpdateKeepsPrevious')}</p>{#if checked.provider==='nexus'}<p class="muted">{t('nexusUpdateDownloadHint')}</p>{/if}{/if}
          {#if checked?.url}<button class="wide ghost" onclick={()=>void run(async()=>{await openUrl(checked.url)})}>{t('reviewModFiles')}<IconArrowUpRight size={15}/></button>{/if}
          {#if active.previous_version}<button class="wide" disabled={busy||updating} onclick={()=>restoreMod(active!)}>{t('restorePreviousMod')}</button>{/if}
         </details>
        {/if}
       </div>
      </div>
     {/key}
     <div class="inspector-actions">
      {#if active.archive}
       {#if /\.(zip|7z|rar)$/i.test(active.path)}<button class="wide primary" disabled={busy||updating} onclick={()=>void installSavedArchive(active!)}><IconDownload size={16}/>{t('installArchive')}</button>{#if data?.settings.paks_dir}<button class="wide" title={t('installAsLegacyHint')} disabled={busy||updating} onclick={()=>void installSavedArchive(active!,true)}><IconArchive size={16}/>{t('installAsLegacy')}</button>{/if}{/if}
       <button class="wide danger" disabled={busy||updating} onclick={()=>remove(active!)}><IconTrash size={16}/>{t('remove')}</button>
      {:else}
       <button class="wide primary" disabled={busy||updating} onclick={()=>toggle(active!)}>{active.enabled?t('disableMod'):t('enableMod')}</button>
       <div class="actions">{#if active.managed}<button disabled={busy||updating} onclick={()=>editMod(active!)}><IconPencil size={16}/>{t('edit')}</button>{/if}<button class="danger" disabled={busy||updating} onclick={()=>remove(active!)}><IconTrash size={16}/>{t('remove')}</button></div>
      {/if}
      <div class="inspector-utilities"><button class="wide ghost" onclick={()=>void run(async()=>{await openFolder(modFolder(active!))})}><IconFolder size={16}/>{t('openFolder')}</button><button class="thumbnail-action icon" aria-label={t('chooseThumbnail')} title={t('chooseThumbnail')} disabled={busy||updating} onclick={()=>chooseThumbnail(active!)}><IconPhoto size={17}/></button></div>
     </div>
    {:else}<div class="inspector-empty"><IconCommand size={28} stroke={1}/><p>{t('selectMod')}</p></div>{/if}
   </section>
  </ResizableInspector></div>
  {/if}
  {/if}
  <footer class="library-status">
   <span class="library-status-text">
    <span class="game-dot" class:busy={updating||busy&&(!operationLabel||operationVisible)} aria-hidden="true"></span>
    <span class="library-status-label" title={operationVisible?operationLabel:undefined}>{updating?t('applyingUpdate'):operationVisible&&operationLabel?operationLabel:busy&&!operationLabel?t('working'):t('localLibrary')}</span>
   </span>
   <span class="sr-only" role="status">{operationVisible?operationLabel:''}</span>
   <div class="mod-operation" class:visible={operationVisible} role="progressbar" aria-label={operationLabel||undefined} aria-hidden={!operationVisible}></div>
  </footer>
 </main>
</div>
{#if editOpen||linkOpen||importOpen}<div class="overlay"><div class="modal" use:focusDialog role="dialog" aria-modal="true" aria-label={editOpen?t('editMod'):importOpen?t(addingSource?'addSource':'importFolder'):t(nexus?.provider==='curseforge'?'confirmCurseforgeDownload':'confirmNexusDownload')} tabindex="-1"><div class="modal-head"><h2>{editOpen?t('editMod'):importOpen?t(addingSource?'addSource':'importFolder'):t(nexus?.provider==='curseforge'?'confirmCurseforgeDownload':'confirmNexusDownload')}</h2><button class="icon" onclick={()=>{editOpen=false;dismissLink();importOpen=false}} aria-label={t('close')}><IconX size={20}/></button></div>
 {#if importOpen}{#if addingSource}<div class="scan-choice"><label class="checkbox-option"><input type="checkbox" bind:checked={importRecursive}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('scanSubfoldersHint')}</p></div><button class="wide" disabled={busy||updating} onclick={chooseSourceFolders}><IconFolder size={17}/>{t('chooseFolder')}</button>{#each sourcePaths as path}<p class="import-path">{path}</p>{/each}{:else}<p class="import-path">{importPath}</p><label class="checkbox-option"><input type="checkbox" bind:checked={importRecursive}/><span>{t('scanSubfolders')}</span></label><p class="muted scan-hint">{t('scanSubfoldersHint')}</p>{/if}<div class="actions"><button disabled={busy||updating} onclick={()=>importOpen=false}>{t('cancel')}</button><button class="primary" disabled={busy||updating||(addingSource&&!sourcePaths.length)} onclick={addingSource?saveSourceFolders:importFolder}>{t(addingSource?'addSource':'importFolder')}</button></div>
 {:else if editOpen}<label>{t('modName')}<input bind:value={editName}/></label><label>{t('description')}<textarea bind:value={editDescription} rows="5"></textarea></label><button class="primary wide" disabled={busy||updating} onclick={()=>void run(async()=>{await invoke('edit_mod',{id:active!.id,name:editName,description:editDescription});await refresh();editOpen=false})}>{t('saveChanges')}</button>
 {:else if linkOpen&&nexus}<div class="download-confirm" bind:this={downloadConfirmation}><IconDownload size={28} stroke={1.5}/><p class="link-info">{downloadLabel(nexus)}</p><p>{t(nexus.provider==='curseforge'?'curseforgeDownloadHint':'downloadConfirmHint')}</p><p class="download-confirm-path" title={downloadFolder(data?.settings)}>{downloadFolder(data?.settings)}</p></div>{#if promptKey&&nexus.provider==='curseforge'}<NexusApiKey provider="curseforge" bind:value={curseforgeKey} disabled={updating||!curseforgeKeyReady} saving={curseforgeKeySaving} onopen={()=>void run(async()=>{await openUrl(CURSEFORGE_KEYS_URL)})} onchange={changeCurseforgeKey}/>{:else if promptKey}<NexusApiKey bind:value={nexusKey} disabled={updating||!nexusKeyReady} saving={nexusKeySaving} onopen={openNexusKeys} onchange={changeNexusKey}/>{/if}<div class="actions"><button onclick={dismissLink}>{t('cancel')}</button><button class="primary" data-autofocus disabled={updating||!keyReady(nexus)||(nexus.provider==='curseforge'?curseforgeKeySaving:nexusKeySaving)||!keyFor(nexus).trim()} onclick={confirmDownload}>{t('ok')}</button></div>{/if}

 {#if error}<p class="form-error" role="alert">{t('operationFailed')}: {errorMessage}</p>{/if}
 </div></div>{/if}

{#if onboardingOpen}<div class="overlay"><div class="modal onboarding-modal" use:focusDialog role="dialog" aria-modal="true" aria-label={t('onboardingTitle')} tabindex="-1"><Onboarding bind:nexusKey bind:curseforgeKey nexusSaving={nexusKeySaving} curseforgeSaving={curseforgeKeySaving} disabled={updating} onnexus={openNexusKeys} oncurseforge={()=>void run(async()=>{await openUrl(CURSEFORGE_KEYS_URL)})} onchangenexus={changeNexusKey} onchangecurseforge={changeCurseforgeKey} ondone={()=>onboardingOpen=false}/>{#if error}<p class="form-error" role="alert">{t('operationFailed')}: {errorMessage}</p>{/if}</div></div>{/if}
{#if downloadsOpen}<div class="overlay downloads-overlay"><div class="downloads-panel" use:focusDialog role="dialog" aria-modal="true" aria-label={t('downloads')} tabindex="-1"><Downloads {jobs} paused={queuePaused} path={downloadFolder(data?.settings)} onclose={()=>downloadsOpen=false} onpause={()=>queuePaused=!queuePaused} oncancel={job=>void cancelDownload(job)} onretry={retryDownload} onclear={()=>jobs=jobs.filter(pendingDownload)} onopen={path=>void run(async()=>{if(/^https?:\/\//.test(path))await openUrl(path);else await openFolder(path)})} onlink={raw=>void receiveLink(raw)} onfolder={()=>void chooseDownloadFolder()} onsettings={openSettings}/>{#if error}<p class="form-error" role="alert">{t('operationFailed')}: {errorMessage}</p>{/if}</div></div>{/if}

{#if context}{#key context}<ContextMenu x={context.x} y={context.y} items={contextItems} onclose={closeContext}/>{/key}{/if}

{#if checkLogsOpen}<div class="overlay check-log-overlay"><div class="check-log-panel" role="dialog" aria-modal="true" aria-label={t('modCheckLog')} tabindex="-1" use:focusDialog><ModCheckLog entries={checkLogs} path={checkLogPath} loading={checkLogLoading} error={checkLogError} copied={checkLogCopied} copyFallback={checkLogFallback} onclose={()=>checkLogsOpen=false} onrefresh={()=>void loadCheckLog()} oncopy={entries=>void copyCheckLog(entries)} onsettings={openSettings} oninspect={inspectCheckLog} caninspect={entry=>libraryMods.some(m=>m.id===entry.mod_id||m.project===entry.project)}/></div></div>{/if}

<DownloadHandoff bind:this={downloadHandoff}/>

{#if metadataOpen&&active}<ModDetailsDialog mod={active} disabled={busy||updating} error={error?errorMessage:''} onopen={url=>void run(async()=>{await openUrl(url)})} ontags={tags=>saveTags(active!,tags)} onclose={()=>metadataOpen=false}/>{/if}

{#if discoveryOpen}<div class="overlay"><div class="modal discovery-modal" use:focusDialog role="dialog" aria-modal="true" aria-label={t('scanComputer')} tabindex="-1"><ModDiscovery result={discovery} {busy} scanning={!!scanRequestId} progress={scanProgress} cancelling={scanCancelling} error={errorMessage} drives={scanDrives} onauto={drives=>void scanComputer(undefined,true,drives)} onquick={()=>void scanComputer(undefined,false)} onstop={()=>void stopDiscovery()} onchoose={()=>void chooseDiscoveryFolder()} onadd={paths=>void addDiscovered(paths)} onclose={closeDiscovery} onopen={path=>void run(async()=>{await openFolder(path)})} onimport={path=>void importDiscoveredArchive(path)}/></div></div>{/if}
