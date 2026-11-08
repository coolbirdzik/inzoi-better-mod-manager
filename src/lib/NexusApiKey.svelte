<script lang="ts">
 import {IconArrowUpRight,IconX} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';
 let {provider='nexus',value=$bindable(''),disabled=false,saving=false,onopen,onchange}:{provider?:'nexus'|'curseforge';value?:string;disabled?:boolean;saving?:boolean;onopen:()=>void;onchange:(value:string)=>void}=$props();
 const fieldId=$derived(`${provider}-api-key`);
</script>

<section class="nexus-api-key">
 <p id={`${fieldId}-guide`}>{t(provider==='curseforge'?'curseforgeApiKeyGuide':'nexusApiKeyGuide')}</p>
 <button class="wide" {disabled} onclick={onopen}>{t(provider==='curseforge'?'getCurseforgeApiKey':'getNexusApiKey')}<IconArrowUpRight size={15}/></button>
 <label for={fieldId}>{t(provider==='curseforge'?'curseforgeApiKey':'nexusApiKey')}</label><span class="key-status" class:ok={!!value.trim()}>{t(value.trim()?'keyConnected':'keyNotConnected')}</span>
  <div class="key-input"><input id={fieldId} type="password" bind:value oninput={event=>onchange(event.currentTarget.value)} autocomplete="off" spellcheck="false" placeholder={t('apiKeyPlaceholder')} aria-describedby={`${fieldId}-guide ${fieldId}-hint`} {disabled}/>
   {#if value}<button class="icon" {disabled} onclick={()=>{value='';onchange('')}} aria-label={t(provider==='curseforge'?'clearCurseforgeApiKey':'clearNexusApiKey')} title={t(provider==='curseforge'?'clearCurseforgeApiKey':'clearNexusApiKey')}><IconX size={16}/></button>{/if}
  </div>
 <p class="muted" id={`${fieldId}-hint`} role="status">{t(saving?'nexusKeySaving':provider==='curseforge'?'curseforgeApiKeyHint':'apiKeyHint')}</p>
</section>

<style>
 .key-status{display:inline-block;margin:0 0 8px;padding:1px 8px;border-radius:99px;font-size:10px;line-height:1.6;color:#d9a441;border:1px solid #d9a44166}.key-status.ok{color:var(--accent);border-color:var(--accent)}
 .nexus-api-key{margin:20px 0;padding:16px;border:1px solid var(--line);border-radius:8px;background:#ffffff03}
 p{font-size:12px;line-height:1.7;margin:0 0 10px;color:var(--muted)}
 p.muted{margin:10px 0 0}
 .key-input{display:flex;align-items:center;gap:5px}.key-input input{flex:1}.key-input button{margin-top:7px}
</style>
