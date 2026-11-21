<script lang="ts">
 import { onMount } from 'svelte';
 import { getVersion } from '@tauri-apps/api/app';
 import { isTauri } from '@tauri-apps/api/core';
 import { openUrl } from '@tauri-apps/plugin-opener';
 import { IconBrandGithub, IconBrandPaypal } from '@tabler/icons-svelte';
 import { version as buildVersion } from '../../package.json';
 import { t } from './i18n.svelte';

 let version=$state(buildVersion), error=$state('');
 onMount(()=>{if(isTauri())void getVersion().then(value=>{if(value)version=value}).catch(()=>{/* Use the build version if native app information is unavailable. */})});
 async function external(event:MouseEvent,url:string){
  if(!isTauri())return;
  event.preventDefault();error='';
  try{await openUrl(url)}catch(reason){error=String(reason)}
 }
</script>

<section class="app-info" aria-label={t('aboutApp')}>
 <span class="app-version" title={t('version')}>v{version}</span>
 <a class="author" href="https://github.com/coolbirdzik" target="_blank" rel="noopener noreferrer" aria-label={t('byAuthor',{author:'coolbirdzik'})} title={t('byAuthor',{author:'coolbirdzik'})} onclick={event=>void external(event,'https://github.com/coolbirdzik')}><IconBrandGithub size={15}/></a>
 <a class="donate" href="https://paypal.me/coolbirdv" target="_blank" rel="noopener noreferrer" aria-label={t('donatePayPal')} title={t('donatePayPal')} onclick={event=>void external(event,'https://paypal.me/coolbirdv')}><IconBrandPaypal size={15}/></a>
 {#if error}<p class="form-error" role="alert">{t('operationFailed')}: {error}</p>{/if}
</section>

<style>
 .app-info{display:flex;flex-wrap:wrap;align-items:center;gap:2px;min-width:0;flex-shrink:0}
 .app-version{font-size:10px;color:#77717c;font-variant-numeric:tabular-nums;margin-right:4px}
 a{display:flex;align-items:center;justify-content:center;width:24px;height:24px;color:#8b868f;text-decoration:none;border-radius:5px;transition:background .15s,color .15s}
 a :global(svg){flex-shrink:0}
 a:hover{background:#ffffff0a;color:#fff}
 a:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
 .donate:hover{color:#efab98}
 .form-error{flex-basis:100%;font-size:11px;line-height:1.6;overflow-wrap:anywhere;margin:8px 0 0}
</style>
