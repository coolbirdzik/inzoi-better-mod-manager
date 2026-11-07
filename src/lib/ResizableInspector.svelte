<script lang="ts">
 import { onMount, type Snippet } from 'svelte';
 import { t } from './i18n.svelte';

 let {children}:{children:Snippet}=$props();
 const minimum=240, maximum=600, defaultWidth=310;
 function savedWidth(){try{const value=Number(localStorage.getItem('inzoimm.inspectorWidth'));return Number.isFinite(value)&&value>=minimum?Math.min(maximum,value):defaultWidth}catch{return defaultWidth}}
 let preferred=$state(savedWidth()), limit=$state(maximum), dragging=$state(false);
 let panel:HTMLDivElement;
 let startX=0, startWidth=0;
 const width=$derived(Math.max(minimum,Math.min(preferred,limit)));
 function remember(){try{localStorage.setItem('inzoimm.inspectorWidth',String(preferred))}catch{/* Keep the width for this session. */}}
 function resize(value:number){preferred=Math.max(minimum,Math.min(limit,value))}
 function start(event:PointerEvent){
  if(event.button!==0)return;
  event.preventDefault();startX=event.clientX;startWidth=width;dragging=true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
 }
 function finish(){if(dragging){dragging=false;remember()}}
 function keyboard(event:KeyboardEvent){
  if(event.altKey||event.ctrlKey||event.metaKey)return;
  const step=event.shiftKey?50:10;
  if(event.key==='ArrowLeft')resize(width+step);
  else if(event.key==='ArrowRight')resize(width-step);
  else if(event.key==='Home')resize(minimum);
  else if(event.key==='End')resize(limit);
  else return;
  event.preventDefault();event.stopPropagation();remember();
 }
 onMount(()=>{
  const container=panel.parentElement!;
  const measure=()=>{limit=Math.max(minimum,Math.min(maximum,container.clientWidth-240))};
  measure();const observer=new ResizeObserver(measure);observer.observe(container);
  window.addEventListener('blur',finish);
  return()=>{observer.disconnect();window.removeEventListener('blur',finish)};
 });
 $effect(()=>{
  document.documentElement.classList.toggle('resizing-inspector',dragging);
  return()=>document.documentElement.classList.remove('resizing-inspector');
 });
</script>

<div class="inspector-panel" bind:this={panel} style:width={`${width}px`}>
 <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (A focusable separator implements the ARIA window splitter control.) -->
 <div class="inspector-resizer" class:dragging role="separator" tabindex="0" aria-label={t('resizeInspector')} title={t('resizeInspectorHint')} aria-controls="mod-inspector" aria-orientation="vertical" aria-valuemin={minimum} aria-valuemax={Math.round(limit)} aria-valuenow={Math.round(width)}
  onpointerdown={start} onpointermove={event=>{if(dragging)resize(startWidth+startX-event.clientX)}} onpointerup={finish} onpointercancel={finish} onlostpointercapture={finish} onkeydown={keyboard} ondblclick={()=>{resize(defaultWidth);remember()}}></div>
 {@render children()}
</div>
