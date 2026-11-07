<script lang="ts">
 import { convertFileSrc } from '@tauri-apps/api/core';
 import { IconBox } from '@tabler/icons-svelte';
 import { t } from './i18n.svelte';
 let { path, name, large = false }: { path: string | null; name: string; large?: boolean } = $props();
 let failedPath = $state<string | null>(null);
</script>

{#if path && failedPath !== path}
 <img class="thumbnail-image" src={path.startsWith('https://') ? path : convertFileSrc(path)} alt={name} loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror={() => failedPath = path}/>
{:else}
 <div class="thumbnail-placeholder" title={t('noThumbnail')}>
  <IconBox size={large ? 44 : 25} stroke={1.1}/>
  {#if large}<span>{t('noThumbnail')}</span>{/if}
 </div>
{/if}
