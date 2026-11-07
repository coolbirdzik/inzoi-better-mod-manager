<script lang="ts">
 import {IconArrowUpRight,IconX} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';
 let {value=$bindable(''),disabled=false,onopen}:{value?:string;disabled?:boolean;onopen:()=>void}=$props();
</script>

<section class="nexus-api-key">
 <p id="nexus-api-key-guide">{t('nexusApiKeyGuide')}</p>
 <button class="wide" {disabled} onclick={onopen}>{t('getNexusApiKey')}<IconArrowUpRight size={15}/></button>
 <label for="nexus-api-key">{t('nexusApiKey')}</label>
  <div class="key-input"><input id="nexus-api-key" type="password" bind:value autocomplete="off" spellcheck="false" placeholder={t('apiKeyPlaceholder')} aria-describedby="nexus-api-key-guide nexus-api-key-hint" {disabled}/>
   {#if value}<button class="icon" {disabled} onclick={()=>value=''} aria-label={t('clearNexusApiKey')} title={t('clearNexusApiKey')}><IconX size={16}/></button>{/if}
  </div>
 <p class="muted" id="nexus-api-key-hint">{t('apiKeyHint')}</p>
</section>

<style>
 .nexus-api-key{margin:20px 0;padding:16px;border:1px solid var(--line);border-radius:8px;background:#ffffff03}
 p{font-size:12px;line-height:1.7;margin:0 0 10px;color:var(--muted)}
 p.muted{margin:10px 0 0}
 .key-input{display:flex;align-items:center;gap:5px}.key-input input{flex:1}.key-input button{margin-top:7px}
</style>
