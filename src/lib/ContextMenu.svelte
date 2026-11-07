<script module lang="ts">
 export type MenuItem={id:string;label:string;run:()=>void;disabled?:boolean;checked?:boolean;danger?:boolean;separator?:boolean};
</script>
<script lang="ts">
 import {onMount} from 'svelte';
 import {IconCheck} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';
 let {x,y,items,onclose}:{x:number;y:number;items:MenuItem[];onclose:(restoreFocus?:boolean)=>void}=$props();
 let menu:HTMLDivElement;
 let left=$state(0),top=$state(0);
 function buttons(){return Array.from(menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)'))}
 function keydown(e:KeyboardEvent){
  if(e.key==='Escape'){e.preventDefault();e.stopPropagation();onclose(true);return}
  if(e.key==='Tab'){onclose(true);return}
  if(!['ArrowDown','ArrowUp','Home','End'].includes(e.key))return;
  e.preventDefault();e.stopPropagation();const options=buttons();if(!options.length)return;
  const index=options.indexOf(document.activeElement as HTMLButtonElement);
  const next=e.key==='Home'?0:e.key==='End'?options.length-1:(index+(e.key==='ArrowDown'?1:-1)+options.length)%options.length;
  options[next]?.focus();
 }
 onMount(()=>{
  const rect=menu.getBoundingClientRect();
  left=Math.max(8,Math.min(x,window.innerWidth-rect.width-8));top=Math.max(8,Math.min(y,window.innerHeight-rect.height-8));
  buttons()[0]?.focus({preventScroll:true});
  const outside=(e:PointerEvent)=>{if(!menu.contains(e.target as Node))onclose()};
  const wheel=(e:Event)=>{if(!menu.contains(e.target as Node))onclose()};
  const resize=()=>onclose();
  document.addEventListener('pointerdown',outside,true);document.addEventListener('wheel',wheel,true);window.addEventListener('resize',resize);
  return()=>{document.removeEventListener('pointerdown',outside,true);document.removeEventListener('wheel',wheel,true);window.removeEventListener('resize',resize)};
 });
</script>
<div class="context-menu" role="menu" aria-label={t('contextMenu')} tabindex="-1" bind:this={menu} style:left={`${left}px`} style:top={`${top}px`} onkeydown={keydown} oncontextmenu={e=>e.preventDefault()}>
 {#each items as item (item.id)}
  {#if item.separator}<div role="separator" class="menu-separator"></div>{/if}
  <button role={item.checked===undefined?'menuitem':'menuitemcheckbox'} aria-checked={item.checked} disabled={item.disabled} class:danger={item.danger} onclick={()=>{onclose(true);item.run()}}><span class="menu-check">{#if item.checked}<IconCheck size={14}/>{/if}</span><span>{item.label}</span></button>
 {/each}
</div>
