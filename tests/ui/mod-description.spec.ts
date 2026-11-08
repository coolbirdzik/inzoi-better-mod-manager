import {test,expect,type Page} from '@playwright/test';

async function library(page:Page) {
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];
  const html='<h2>Soft linen, everyday wear</h2><p>A relaxed summer dress with <strong>four colour variants</strong> and a clean, natural silhouette.</p><h3>Installation</h3><ol><li>Import the archive into your library.</li><li>Enable the mod before starting inZOI.</li></ol><blockquote><p>Choose one version of the dress at a time.</p></blockquote><p><a href="https://www.nexusmods.com/inzoi/mods/42">Read the author’s notes</a></p><table><thead><tr><th>Version</th><th>Compatibility</th></tr></thead><tbody><tr><td>1.2</td><td>Current game release</td></tr></tbody></table>';
  w.mods=JSON.parse(localStorage.getItem('description.mods')||'null')??[
   {id:'html',name:'Linen Dress',description:html,metadata:{'mod_manifest.json':{ProjectName:'LinenDress',Custom:{Compatibility:'Patch 1.2',Optional:false}},'nexus.json':{Download:{mod:{description:html}}}}},
   {id:'markdown',name:'Markdown Outfit',description:'# Everyday outfit\n\nA **soft** finish with *subtle* details.\n\n- Sand\n- Cloud\n\n[Author guide](https://example.com/guide)\n\n```text\n<literal markup>\n```'},
   {id:'bbcode',name:'Nexus BBCode',description:'[heading]Installation[/heading]\n[b]Create a look[/b] with [i]linen textures[/i].\n[list][*]Sand[*]Cloud[/list]\n[list=1][*]Import[*]Enable[/list]\n[url=https://example.com/bbcode]Full guide[/url]\n[spoiler=Compatibility]Current game version[/spoiler]\n[code][b]literal[/b][/code]'},
   {id:'fallback',name:'Saved Description',description:'',archive:true,metadata:{'curseforge.json':{Download:{mod:{description:'<h2>Saved author description</h2><p>From the downloaded metadata.</p>'}}}}},
   {id:'empty',name:'No Description',description:''},
  ].map(m=>({author:'Mira',version:'1.2',path:`C:/Library/${m.id}`,project:m.id,thumbnail:null,managed:true,enabled:false,source:'',tags:[],...m}));
  w.__TAURI_INTERNALS__={transformCallback:()=>1,invoke:async(cmd:string,args:any)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return{mods:w.mods,settings:{mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true},warnings:[]};
   if(cmd==='pending_links')return[];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='set_mod_tags'){if(w.failTags)throw new Error('Could not save tags');w.mods.find((m:any)=>m.id===args.id).tags=args.tags;localStorage.setItem('description.mods',JSON.stringify(w.mods));}
   return null;
  }};
 });
 await page.goto('/');await expect(page.locator('.mod-row')).toHaveCount(5);
}
async function select(page:Page,name:string){await page.locator('.mod-row').filter({has:page.getByText(name,{exact:true})}).click();}
const expand=(page:Page)=>page.getByRole('button',{name:'View full metadata',exact:true});
const reader=(page:Page)=>page.getByRole('dialog',{name:'Mod details',exact:true});
async function inline(page:Page){await page.locator('.inspector-section>summary').filter({hasText:/^Description$/}).click();return page.locator('.inspector-body>.inspector-section .rich-description');}

test('HTML descriptions read as formatted content and safe links open through the native opener',async({page})=>{
 await library(page);const rich=await inline(page);
 await expect(rich.getByRole('heading',{name:'Soft linen, everyday wear'})).toBeVisible();
 await expect(rich.locator('strong')).toHaveText('four colour variants');await expect(rich.locator('ol li')).toHaveCount(2);
 await expect(rich.locator('blockquote')).toContainText('Choose one version');await expect(rich.locator('table')).toContainText('Compatibility');
 await rich.getByRole('link',{name:'Read the author’s notes'}).focus();await page.keyboard.press('Enter');
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url').args.url)).toBe('https://www.nexusmods.com/inzoi/mods/42');
 await expect(page).toHaveURL('http://127.0.0.1:1422/');
 await expand(page).click();await expect(reader(page).getByRole('heading',{name:'Soft linen, everyday wear'})).toBeVisible();
 await page.setViewportSize({width:900,height:600});
 const bounds=await reader(page).boundingBox();expect(bounds).toMatchObject({x:0,y:0,width:900,height:600});
 expect(await reader(page).evaluate(node=>node.scrollWidth)).toBeLessThanOrEqual(900);
 await page.screenshot({path:'test-results/mod-description-fullscreen-900.png'});
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='edit_mod'))).toBe(false);
});

test('Markdown and Nexus BBCode render headings, nested formatting, lists, spoilers and literal code',async({page})=>{
 await library(page);await select(page,'Markdown Outfit');let rich=await inline(page);
 await expect(rich.getByRole('heading',{name:'Everyday outfit'})).toBeVisible();await expect(rich.locator('strong')).toHaveText('soft');
 await expect(rich.locator('li')).toHaveText(['Sand','Cloud']);await expect(rich.locator('pre code')).toHaveText('<literal markup>');
 await expect(rich.getByRole('link',{name:'Author guide'})).toHaveAttribute('href','https://example.com/guide');
 await select(page,'Nexus BBCode');rich=await inline(page);
 await expect(rich.getByRole('heading',{name:'Installation'})).toBeVisible();await expect(rich.locator('strong')).toHaveText('Create a look');
 await expect(rich.locator('em')).toHaveText('linen textures');await expect(rich.locator('ul li')).toHaveText(['Sand','Cloud']);
 await expect(rich.locator('ol li')).toHaveText(['Import','Enable']);await expect(rich.getByRole('link',{name:'Full guide'})).toHaveAttribute('href','https://example.com/bbcode');
 await rich.locator('summary').click();await expect(rich.locator('details')).toContainText('Current game version');
 await expect(rich.locator('pre code')).toHaveText('[b]literal[/b]');
});

test('fullscreen metadata keeps focus, supports keyboard tabs and edits tags without changing selection',async({page})=>{
 await library(page);await expand(page).click();const panel=reader(page);
 await expect(panel.getByRole('button',{name:'Close',exact:true})).toBeFocused();
 await panel.getByRole('tab',{name:'Description',exact:true}).focus();await page.keyboard.press('ArrowRight');
 await expect(panel.getByRole('tab',{name:'Metadata',exact:true})).toBeFocused();
 await expect(panel.getByText('mod_manifest.json.Custom.Compatibility',{exact:true})).toBeVisible();
 await expect(panel.getByText('Patch 1.2',{exact:true})).toBeVisible();await expect(panel.getByText('false',{exact:true})).toBeVisible();
 await expect(panel.getByRole('heading',{name:'Soft linen, everyday wear'})).toBeVisible();
 await panel.getByRole('textbox',{name:'Add tag',exact:true}).fill('Summer');await page.keyboard.press('Enter');
 await expect(panel.getByRole('button',{name:'Remove tag Summer',exact:true})).toBeVisible();
 await page.evaluate(()=>{(window as any).failTags=true});await panel.getByRole('textbox',{name:'Add tag',exact:true}).fill('Autumn');await page.keyboard.press('Enter');
 await expect(panel.getByRole('alert')).toContainText('Could not save tags');await expect(panel.getByRole('textbox',{name:'Add tag',exact:true})).toHaveValue('Autumn');
 await page.evaluate(()=>{(window as any).failTags=false});await panel.getByRole('textbox',{name:'Add tag',exact:true}).press('Enter');
 await expect(panel.getByRole('button',{name:'Remove tag Autumn',exact:true})).toBeVisible();await expect(panel.getByRole('alert')).toHaveCount(0);
 await panel.getByRole('textbox',{name:'Add tag',exact:true}).focus();await page.keyboard.press('Control+k');
 expect(await page.evaluate(()=>document.activeElement?.closest('dialog')!==null)).toBe(true);
 await page.keyboard.press('Shift+Tab');expect(await page.evaluate(()=>document.activeElement?.closest('dialog')!==null)).toBe(true);
 await page.setViewportSize({width:900,height:600});await page.screenshot({path:'test-results/mod-metadata-fullscreen-900.png'});
 await page.keyboard.press('Escape');await expect(panel).toHaveCount(0);await expect(expand(page)).toBeFocused();
 await expect(page.locator('.inspector .identity-copy>h2')).toHaveText('Linen Dress');await expect(page.locator('.mod-row').filter({hasText:'Linen Dress'})).toHaveAttribute('aria-pressed','true');
 await expand(page).click();await panel.getByRole('button',{name:'Close',exact:true}).click();await expect(expand(page)).toBeFocused();
 await page.reload();await expand(page).click();await panel.getByRole('tab',{name:'Metadata',exact:true}).click();await expect(panel.getByRole('button',{name:'Remove tag Summer',exact:true})).toBeVisible();
});

test('saved metadata supplies missing descriptions for archives and empty descriptions stay readable',async({page})=>{
 await library(page);await select(page,'Saved Description');await expand(page).click();
 await expect(reader(page).getByRole('heading',{name:'Saved author description'})).toBeVisible();await page.keyboard.press('Escape');
 await select(page,'No Description');await expand(page).click();await expect(reader(page).locator('.rich-description')).toContainText('This mod has no description.');
});

test('untrusted descriptions strip executable markup, unsafe links and unapproved image sources in both views',async({page})=>{
 const requests:string[]=[];page.on('request',request=>requests.push(request.url()));
 await library(page);
 await page.evaluate(()=>{
  const hostile='<h2>Author notes</h2><script>window.descriptionAttack=true</script><style>body{display:none}</style><svg onload="window.descriptionAttack=true"></svg><iframe src="https://evil.test/frame"></iframe><form><input autofocus></form><a href="javascript:window.descriptionAttack=true">Unsafe script</a><a href="file:///C:/secret">Local file</a><a href="/settings">Relative link</a><a href="https://user:password@example.com">Credentials</a><a href="https://example.com/safe" onclick="window.descriptionAttack=true" style="position:fixed" id="settings">Safe guide</a><img src="https://evil.test/tracker.png" onerror="window.descriptionAttack=true" alt="Unapproved picture"><img src="data:image/svg+xml,attack"><img src="https://media.forgecdn.net/avatars/safe.png" onerror="window.descriptionAttack=true">';
  (window as any).mods[0].description=hostile;(window as any).mods[0].metadata={'metadata.json':{description:hostile}};
 });
 await page.route('https://media.forgecdn.net/**',route=>route.fulfill({contentType:'image/png',body:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64')}));
 await page.getByRole('button',{name:'Refresh library',exact:true}).click();let rich=await inline(page);
 await expect(rich.locator('script,style,svg,iframe,form,input,[onclick],[onerror],[style],[id]')).toHaveCount(0);
 await expect(rich.locator('a[href]')).toHaveCount(1);await expect(rich.locator('img')).toHaveCount(1);
 await expect(rich.locator('img')).toHaveAttribute('referrerpolicy','no-referrer');await expect(rich).toContainText('Unapproved picture');
 await expand(page).click();await reader(page).getByRole('tab',{name:'Metadata',exact:true}).click();rich=reader(page).locator('.metadata-fields .rich-description');
 await expect(rich.locator('script,style,svg,iframe,form,input,[onclick],[onerror],[style],[id]')).toHaveCount(0);
 await expect(rich.locator('a[href]')).toHaveCount(1);await rich.getByRole('link',{name:'Safe guide'}).click();
 expect(await page.evaluate(()=>(window as any).descriptionAttack)).toBeUndefined();
 expect(requests.some(url=>url.includes('evil.test'))).toBe(false);
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url').args.url)).toBe('https://example.com/safe');
});
