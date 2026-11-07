<script lang="ts">
 import { IconArrowUpRight, IconPlus, IconX } from '@tabler/icons-svelte';
 import { t } from './i18n.svelte';
 import { metadataFields, webLinks } from './library';
 import type { Mod } from './types';
 let {mod,disabled=false,onopen,ontags}:{mod:Mod;disabled?:boolean;onopen:(url:string)=>void;ontags:(tags:string[])=>Promise<boolean>}=$props();
 let tag=$state('');
 const fields=$derived(metadataFields(mod.metadata));
 const links=$derived(webLinks(mod));
 async function addTag(){const value=tag.trim();if(!value)return;if(await ontags([...(mod.tags??[]),value]))tag='';}
</script>
<section class="mod-metadata" aria-label={t('metadata')}>
 {#if links.length}<h3>{t('links')}</h3><div class="metadata-links">{#each links as url}<button title={url} disabled={disabled} onclick={()=>onopen(url)}><span>{url}</span><IconArrowUpRight size={15}/></button>{/each}</div>{/if}
 <h3>{t('tags')}</h3>
 <div class="tag-chips">{#each mod.tags??[] as value}<span class="tag-chip">{value}<button class="icon" disabled={disabled} aria-label={t('removeTag',{tag:value})} onclick={()=>ontags((mod.tags??[]).filter(item=>item!==value))}><IconX size={12}/></button></span>{/each}</div>
 <form class="tag-form" onsubmit={e=>{e.preventDefault();void addTag()}}><input aria-label={t('addTag')} placeholder={t('addTag')} maxlength="60" bind:value={tag} disabled={disabled}/><button class="icon" aria-label={t('addTag')} disabled={disabled||!tag.trim()}><IconPlus size={16}/></button></form>
 <h3>{t('metadata')}</h3><dl class="metadata-fields"><div><dt>{t('project')}</dt><dd>{mod.project}</dd></div><div><dt>{t('modPath')}</dt><dd>{mod.path}</dd></div>{#each fields as field}<div><dt>{field.key}</dt><dd>{field.value}</dd></div>{/each}</dl>
</section>
