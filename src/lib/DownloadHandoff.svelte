<script lang="ts">
 import {onDestroy} from 'svelte';
 import {IconDownload} from '@tabler/icons-svelte';
 import {t} from './i18n.svelte';

 type Flight={id:number;x:number;y:number;dx:number;dy:number;target:HTMLButtonElement};
 let flights=$state<Flight[]>([]),arrival=$state<{id:number;x:number;y:number}|null>(null);
 let sequence=0,noticeTimer:ReturnType<typeof setTimeout>|undefined;
 const pulses=new Set<Animation>();

 function pulse(node:Element,frames:Keyframe[],duration:number){
  try{const animation=node.animate(frames,{duration,easing:'ease-out'});pulses.add(animation);void animation.finished.catch(()=>{}).finally(()=>pulses.delete(animation))}catch{/* The queue also confirms through its count and status. */}
 }
 function arrive(flight:Flight,motion:boolean){
  if(flight.id!==sequence||!flight.target.isConnected)return;
  for(const animation of pulses)animation.cancel();
  if(motion){const icon=flight.target.querySelector('svg');if(icon)pulse(icon,[{transform:'scale(1)'},{transform:'scale(1.3)',offset:.4},{transform:'scale(1)'}],360)}
  pulse(flight.target,[{backgroundColor:'#eb795e38',boxShadow:'inset 0 0 0 1px #eb795e80'},{backgroundColor:'transparent',boxShadow:'inset 0 0 0 1px #eb795e00'}],1400);
  const rect=flight.target.getBoundingClientRect();
  arrival={id:flight.id,x:rect.left,y:rect.top};
  clearTimeout(noticeTimer);noticeTimer=setTimeout(()=>arrival=null,1800);
 }
 export function animateFrom(origin:Element|null,target:HTMLButtonElement|null){
  if(!origin||!target)return;
  const from=origin.getBoundingClientRect(),to=(target.querySelector('svg')??target).getBoundingClientRect();
  if(!from.width||!to.width)return;
  clearTimeout(noticeTimer);arrival=null;
  const x=from.left+from.width/2,y=from.top+from.height/2;
  const flight={id:++sequence,x,y,dx:to.left+to.width/2-x,dy:to.top+to.height/2-y,target};
  if(window.matchMedia('(prefers-reduced-motion: reduce)').matches){arrive(flight,false);return}
  flights=[...flights,flight];
 }
 function fly(node:HTMLElement,flight:Flight){
  let animation:Animation|undefined;
  const finish=()=>{flights=flights.filter(item=>item.id!==flight.id);arrive(flight,true)};
  try{
   animation=node.animate([
    {transform:'translate3d(0,0,0) scale(1)',opacity:1},
    {transform:`translate3d(${flight.dx*.4}px,${flight.dy*.25-60}px,0) scale(.92)`,opacity:1,offset:.4},
    {transform:`translate3d(${flight.dx}px,${flight.dy}px,0) scale(.4)`,opacity:0}
   ],{duration:620,easing:'cubic-bezier(.3,.05,.3,1)',fill:'forwards'});
   void animation.finished.then(finish,()=>{});
  }catch{queueMicrotask(finish)}
  return{destroy(){animation?.cancel()}};
 }
 onDestroy(()=>{clearTimeout(noticeTimer);for(const animation of pulses)animation.cancel()});
</script>

{#each flights as flight (flight.id)}
 <div class="download-handoff-flight" aria-hidden="true" style:left={`${flight.x-22}px`} style:top={`${flight.y-22}px`} use:fly={flight}><IconDownload size={26} stroke={1.7}/></div>
{/each}
<div class="download-handoff-status" role="status" aria-live="polite" aria-atomic="true" style:left={`${arrival?.x??0}px`} style:top={`${arrival?arrival.y-8:0}px`}>
 {#if arrival}{#key arrival.id}<span class="download-handoff-notice">{t('downloadAddedToQueue')}</span>{/key}{/if}
</div>

<style>
 .download-handoff-flight{position:fixed;z-index:3;width:44px;height:44px;display:flex;align-items:center;justify-content:center;pointer-events:none;color:#211713;background:#eb795e;border:1px solid #ffb7a0;border-radius:12px;box-shadow:0 5px 24px #eb795e45;will-change:transform,opacity}
 .download-handoff-status{position:fixed;z-index:3;transform:translateY(-100%);pointer-events:none}
 .download-handoff-notice{display:block;max-width:min(260px,calc(100vw - 28px));padding:9px 12px;border:1px solid #eb795e50;border-radius:6px;background:#30241f;color:#f2c2b3;box-shadow:0 5px 20px #0005;font-size:11px;line-height:1.5}
</style>
