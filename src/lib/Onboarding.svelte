<script lang="ts">
 import {IconKey} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';
 import NexusApiKey from './NexusApiKey.svelte';
 let {nexusKey=$bindable(''),curseforgeKey=$bindable(''),nexusSaving,curseforgeSaving,disabled=false,onnexus,oncurseforge,onchangenexus,onchangecurseforge,ondone}:{nexusKey?:string;curseforgeKey?:string;nexusSaving:boolean;curseforgeSaving:boolean;disabled?:boolean;onnexus:()=>void;oncurseforge:()=>void;onchangenexus:(value:string)=>void;onchangecurseforge:(value:string)=>void;ondone:()=>void}=$props();
 const both=$derived(!!nexusKey.trim()&&!!curseforgeKey.trim());
 const saving=$derived(nexusSaving||curseforgeSaving);
</script>

<div class="modal-head"><h2><IconKey size={20}/>{t('onboardingTitle')}</h2></div>
<div class="onboarding-body">
 <p class="muted">{t('onboardingIntro')}</p>
 <NexusApiKey bind:value={nexusKey} {disabled} saving={nexusSaving} onopen={onnexus} onchange={onchangenexus}/>
 <NexusApiKey provider="curseforge" bind:value={curseforgeKey} {disabled} saving={curseforgeSaving} onopen={oncurseforge} onchange={onchangecurseforge}/>
</div>
<div class="actions onboarding-footer"><span class="muted" role="status">{both?'':t('onboardingNeedBoth')}</span><button class:primary={both} data-autofocus disabled={saving||disabled} onclick={ondone}>{t(both?'onboardingContinue':'onboardingSkip')}</button></div>

<style>
 h2{display:flex;align-items:center;gap:9px}.onboarding-body{overflow:auto;min-height:0;max-height:62vh;padding-right:4px}.onboarding-body>p{font-size:12px;line-height:1.7;margin:12px 0 0}
 .onboarding-footer{align-items:center;justify-content:space-between;gap:12px;padding-top:12px;border-top:1px solid var(--line)}.onboarding-footer span{font-size:11px}
</style>
