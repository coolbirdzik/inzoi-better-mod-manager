import DOMPurify from 'dompurify';
import { marked } from 'marked';
import { metadataFields } from './library';
import type { Mod } from './types';

export function modDescription(mod:Mod):string {
 if(mod.description.trim())return mod.description;
 const fields=metadataFields(mod.metadata);
 return fields.find(field=>/(?:^|\.)description$/i.test(field.key)&&field.value.trim())?.value
  ??fields.find(field=>/(?:^|\.)summary$/i.test(field.key)&&field.value.trim())?.value??'';
}

const escape=(text:string)=>text.replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]!));
export function descriptionUrl(value:string|null):string|undefined {
 try {const url=new URL(value??'');if(['https:','http:'].includes(url.protocol)&&!url.username&&!url.password)return url.href;}catch{/* Incomplete or local links stay as text. */}
}

// Nexus descriptions commonly contain BBCode alongside ordinary HTML.
function bbcode(text:string):string {
 const lists:string[]=[],codes:string[]=[];
 const html=text
  .replace(/\[code\]([\s\S]*?)\[\/code\]/gi,(_,code:string)=>{codes.push(`<pre><code>${escape(code)}</code></pre>`);return `\u0000CODE${codes.length-1}\u0000`;})
  .replace(/\[img(?:=[^\]]*)?\]([\s\S]*?)\[\/img\]/gi,(_,url:string)=>`<img src="${escape(url.trim())}" alt="">`)
  .replace(/\[url=([^\]]*)\]/gi,(_,url:string)=>`<a href="${escape(url.replace(/^['"]|['"]$/g,''))}">`)
  .replace(/\[url\]([^\[]*)\[\/url\]/gi,(_,url:string)=>`<a href="${escape(url.trim())}">${escape(url)}</a>`)
  .replace(/\[\/url\]/gi,'</a>')
  .replace(/\[spoiler(?:=([^\]]*))?\]/gi,(_,title?:string)=>`<details><summary>${escape(title||'…')}</summary>`)
  .replace(/\[\/spoiler\]/gi,'</details>')
  .replace(/\[(\/?)list(?:=([^\]]*))?\]/gi,(_,close:string,ordered?:string)=>{if(close)return `</${lists.pop()??'ul'}>`;const tag=ordered?'ol':'ul';lists.push(tag);return `<${tag}>`;})
  .replace(/\[\*\]/g,'<li>')
  .replace(/\[(\/?)(b|i|u|s|strike|quote|heading|h[1-6]|size|color|font|center|left|right|justify)(?:=[^\]]*)?\]/gi,(_,close:string,name:string)=>{
   const tag:Record<string,string>={b:'strong',i:'em',u:'u',s:'s',strike:'s',quote:'blockquote',heading:'h3',size:'span',color:'span',font:'span',center:'div',left:'div',right:'div',justify:'div'};
   return `<${close}${tag[name.toLowerCase()]??name.toLowerCase()}>`;
  })
  .replace(/\[br\]/gi,'<br>')
  .replace(/\r?\n/g,'<br>')
  .replace(/(<\/?(?:ul|ol|li|h[1-6]|blockquote|details|summary|div)>)(?:<br>)+/gi,'$1')
  .replace(/(?:<br>)+(<\/(?:ul|ol|blockquote|details|div)>)/gi,'$1');
 return html.replace(/\u0000CODE(\d+)\u0000/g,(_,index:string)=>codes[Number(index)]??'');
}

export function descriptionFragment(text:string):DocumentFragment {
 const hasBBCode=/\[(?:b|i|u|s|strike|url|img|list|quote|code|spoiler|heading|h[1-6]|size|color|font|center|left|right|justify|br)(?:\]|=)/i.test(text);
 const html=hasBBCode?bbcode(text):marked.parse(text,{async:false,gfm:true,breaks:true});
 const clean=DOMPurify.sanitize(html,{
  ALLOWED_TAGS:['p','div','span','br','hr','h1','h2','h3','h4','h5','h6','strong','b','em','i','u','s','del','sub','sup','ul','ol','li','blockquote','pre','code','table','thead','tbody','tr','th','td','a','img','details','summary'],
  ALLOWED_ATTR:['href','src','alt','title','colspan','rowspan','start'],
  ALLOW_DATA_ATTR:false,ALLOW_ARIA_ATTR:false,RETURN_DOM_FRAGMENT:true,
 });
 for(const link of Array.from(clean.querySelectorAll('a'))) {
  const url=descriptionUrl(link.getAttribute('href'));
  if(url){link.setAttribute('href',url);link.setAttribute('rel','noreferrer noopener');}else link.removeAttribute('href');
 }
 for(const image of Array.from(clean.querySelectorAll('img'))) {
  const url=descriptionUrl(image.getAttribute('src'));
  if(url&&new URL(url).protocol==='https:'&&['media.forgecdn.net','staticdelivery.nexusmods.com','images.nexusmods.com'].includes(new URL(url).hostname)){
   image.setAttribute('src',url);image.setAttribute('loading','lazy');image.setAttribute('referrerpolicy','no-referrer');
  }else image.replaceWith(document.createTextNode(image.getAttribute('alt')??''));
 }
 return clean;
}
