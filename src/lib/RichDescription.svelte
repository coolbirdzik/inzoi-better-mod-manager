<script lang="ts">
 import { descriptionFragment, descriptionUrl } from './description';
 let {text,onopen}:{text:string;onopen:(url:string)=>void}=$props();
 function render(node:HTMLElement,value:string) {
  const update=(text:string)=>node.replaceChildren(descriptionFragment(text));
  // Native anchors provide keyboard interaction; delegate their clicks after
  // inserting sanitized nodes so navigation always uses the desktop opener.
  node.addEventListener('click',click);node.addEventListener('auxclick',click);
  update(value);return {update,destroy(){node.removeEventListener('click',click);node.removeEventListener('auxclick',click)}};
 }
 function click(event:MouseEvent) {
  if(!(event.target instanceof Element))return;
  const link=event.target.closest('a');if(!link)return;
  event.preventDefault();const url=descriptionUrl(link.getAttribute('href'));if(url)onopen(url);
 }
</script>

<div class="description rich-description" use:render={text}></div>
