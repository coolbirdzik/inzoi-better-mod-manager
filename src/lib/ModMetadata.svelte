<script lang="ts">
 import { IconArrowUpRight, IconChevronRight, IconPlus, IconX } from '@tabler/icons-svelte';
 import { t } from './i18n.svelte';
 import { metadataFields, webLinks } from './library';
 import type { Mod } from './types';
 import RichDescription from './RichDescription.svelte';
 let {mod,disabled=false,expanded=false,onopen,ontags}:{mod:Mod;disabled?:boolean;expanded?:boolean;onopen:(url:string)=>void;ontags:(tags:string[])=>Promise<boolean>}=$props();
 let tag=$state('');
 let sections=$state({links:false,tags:false,metadata:false});
 $effect(()=>{sections={links:expanded,tags:expanded,metadata:expanded}});
 const fields=$derived(metadataFields(mod.metadata));
 const links=$derived(webLinks(mod));
 async function addTag(){const value=tag.trim();if(!value)return;if(await ontags([...(mod.tags??[]),value]))tag='';}
</script>
<section class="mod-metadata" aria-label={t('metadata')}>
 {#if links.length}<details bind:open={sections.links} class="inspector-section"><summary><IconChevronRight size={14}/><span>{t('links')}</span><small>{links.length}</small></summary><div class="section-content metadata-links">{#each links as url}<button title={url} disabled={disabled} onclick={()=>onopen(url)}><span>{url}</span><IconArrowUpRight size={15}/></button>{/each}</div></details>{/if}
 <details bind:open={sections.tags} class="inspector-section inspector-tags"><summary><IconChevronRight size={14}/><span>{t('tags')}</span><small>{mod.tags?.length??0}</small></summary><div class="section-content">
 <div class="tag-chips">{#each mod.tags??[] as value}<span class="tag-chip">{value}<button class="icon" disabled={disabled} aria-label={t('removeTag',{tag:value})} onclick={()=>ontags((mod.tags??[]).filter(item=>item!==value))}><IconX size={12}/></button></span>{/each}</div>
 <form class="tag-form" onsubmit={e=>{e.preventDefault();void addTag()}}><input aria-label={t('addTag')} placeholder={t('addTag')} maxlength="60" bind:value={tag} disabled={disabled}/><button class="icon" aria-label={t('addTag')} disabled={disabled||!tag.trim()}><IconPlus size={16}/></button></form>
 </div></details>
 <details bind:open={sections.metadata} class="inspector-section"><summary><IconChevronRight size={14}/><span>{t('metadata')}</span></summary><dl class="section-content metadata-fields">{#if !mod.archive}<div><dt>{t('project')}</dt><dd>{mod.project}</dd></div>{/if}<div><dt>{t(mod.archive?'archivePath':'modPath')}</dt><dd>{mod.path}</dd></div>{#each fields as field}<div class:metadata-long={field.value.length>180}><dt>{field.key}</dt><dd>{#if /(?:description|summary)$/i.test(field.key)}<RichDescription text={field.value} {onopen}/>{:else}{field.value}{/if}</dd></div>{/each}</dl></details>
</section>
