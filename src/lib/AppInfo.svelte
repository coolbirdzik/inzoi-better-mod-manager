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
 <p class="app-version">{t('version')} <span>{version}</span></p>
 <a class="author" href="https://github.com/coolbirdzik" target="_blank" rel="noopener noreferrer" onclick={event=>void external(event,'https://github.com/coolbirdzik')}><IconBrandGithub size={15}/>{t('byAuthor',{author:'coolbirdzik'})}</a>
 <a class="donate" href="https://paypal.me/coolbirdv" target="_blank" rel="noopener noreferrer" onclick={event=>void external(event,'https://paypal.me/coolbirdv')}><IconBrandPaypal size={16}/>{t('donatePayPal')}</a>
 {#if error}<p class="form-error" role="alert">{t('operationFailed')}: {error}</p>{/if}
</section>

<style>
 .app-info{padding:0 12px 8px;min-width:0}
 .app-version{font-size:10px;color:var(--muted);margin:0 0 6px;line-height:1.5}
 .app-version span{font-variant-numeric:tabular-nums;color:#c5beca;margin-left:4px}
 a{display:flex;align-items:center;gap:7px;font-size:10px;line-height:1.5;color:#aaa7b0;text-decoration:none;border-radius:6px;transition:background .15s,color .15s;overflow-wrap:anywhere}
 a :global(svg){flex-shrink:0}
 a:focus-visible{outline:2px solid var(--accent);outline-offset:3px}
 .author{padding:4px 0;margin-bottom:9px}
 .author:hover{color:#fff}
 .donate{justify-content:center;padding:7px 6px;background:#eb795e16;border:1px solid #eb795e38;color:#efab98}
 .donate:hover{background:#eb795e28;color:#ffd0c3}
 .donate:active{background:#eb795e38}
 .form-error{font-size:11px;line-height:1.6;overflow-wrap:anywhere;margin:12px 0 0}
</style>
