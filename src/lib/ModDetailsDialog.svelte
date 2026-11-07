<script lang="ts">
 import { onMount } from 'svelte';
 import { IconX } from '@tabler/icons-svelte';
 import { t } from './i18n.svelte';
 import { modDescription } from './description';
 import { modOrigin } from './library';
 import ModThumbnail from './ModThumbnail.svelte';
 import ModMetadata from './ModMetadata.svelte';
 import RichDescription from './RichDescription.svelte';
 import type { Mod } from './types';
 let {mod,disabled=false,error='',onopen,ontags,onclose}:{mod:Mod;disabled?:boolean;error?:string;onopen:(url:string)=>void;ontags:(tags:string[])=>Promise<boolean>;onclose:()=>void}=$props();
 let dialog:HTMLDialogElement;
 let tab=$state<'description'|'metadata'>('description');
 const description=$derived(modDescription(mod));
 onMount(()=>{
  const previous=document.activeElement instanceof HTMLElement?document.activeElement:null;
  const opened=dialog;opened.showModal();
  return()=>{opened.close();queueMicrotask(()=>{if(previous?.isConnected)previous.focus({preventScroll:true})})};
 });
 function tabs(event:KeyboardEvent) {
  if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
  event.preventDefault();tab=event.key==='Home'?'description':event.key==='End'?'metadata':tab==='description'?'metadata':'description';
  dialog.querySelector<HTMLButtonElement>(`[data-tab="${tab}"]`)?.focus();
 }
</script>

<dialog class="mod-reader" bind:this={dialog} aria-label={t('modDetails')} oncancel={event=>{event.preventDefault();onclose()}} onkeydown={event=>event.stopPropagation()}>
 <div class="mod-reader-header">
  <div class="mod-reader-thumbnail"><ModThumbnail path={mod.thumbnail} name={mod.name||mod.project}/></div>
  <div class="mod-reader-title"><span class="eyebrow">{modOrigin(mod).label||t('localFiles')}</span><h2>{mod.name||mod.project}</h2><p>{t('byAuthor',{author:mod.author||t('unknownAuthor')})}{#if mod.version}<span> · {mod.version}</span>{/if}</p></div>
  <button class="icon" aria-label={t('close')} title={t('close')} onclick={onclose}><IconX size={21}/></button>
 </div>
 <div class="mod-reader-tabs" role="tablist" tabindex="-1" aria-label={t('modDetails')} onkeydown={tabs}>
  <button role="tab" data-tab="description" id="mod-reader-description-tab" aria-controls="mod-reader-description" aria-selected={tab==='description'} tabindex={tab==='description'?0:-1} onclick={()=>tab='description'}>{t('description')}</button>
  <button role="tab" data-tab="metadata" id="mod-reader-metadata-tab" aria-controls="mod-reader-metadata" aria-selected={tab==='metadata'} tabindex={tab==='metadata'?0:-1} onclick={()=>tab='metadata'}>{t('metadata')}</button>
 </div>
 {#if error}<p class="mod-reader-error" role="alert">{t('operationFailed')}: {error}</p>{/if}
 <div class="mod-reader-scroll">
  {#if tab==='description'}
   <div class="mod-reader-content mod-reader-description" id="mod-reader-description" role="tabpanel" aria-labelledby="mod-reader-description-tab" tabindex="0"><RichDescription text={description||t('noDescription')} {onopen}/></div>
  {:else}
   <div class="mod-reader-content" id="mod-reader-metadata" role="tabpanel" aria-labelledby="mod-reader-metadata-tab" tabindex="0">
    <dl class="mod-reader-facts"><div><dt>{t('version')}</dt><dd>{mod.version||'—'}</dd></div><div><dt>{t('status')}</dt><dd>{mod.archive?t('notInstalled'):mod.enabled?t('enabled'):t('disabled')}</dd></div><div><dt>{t('managedBy')}</dt><dd>{mod.managed?'Better Mod Manager':t('outsideApp')}</dd></div></dl>
    <ModMetadata {mod} {disabled} {onopen} {ontags} expanded/>
   </div>
  {/if}
 </div>
</dialog>
