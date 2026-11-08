import {test, expect, type Page} from '@playwright/test';

async function library(page:Page) {
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  const mods=Array.from({length:12},(_,i)=>({id:`mod-${i}`,name:`Collection ${i+1}`,author:`Creator ${i+1}`,project:`Project_${i}`,version:'1.0',description:`Description ${i+1}`,path:`C:/Mods/${i}`,thumbnail:i===0?'preview.png':i===1?'broken.png':i===3?'https://media.forgecdn.net/avatars/mod.jpg':null,managed:i!==2,enabled:i%2===0,source:''}));
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,convertFileSrc:(path:string)=>`/test-images/${path}`,invoke:async(cmd:string,args:any)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods,settings:{mods_dir:'C:/Mods',library_dir:'C:/library'},warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return 'C:/Pictures/custom.png';
   if(cmd==='plugin:dialog|message')return args.buttons.OkCancelCustom[w.cancelRemoval?1:0];
   if(cmd==='remove_mod'){mods.splice(mods.findIndex(m=>m.id===args.id),1);return;}
   if(cmd==='set_mod_thumbnail'){mods.find(m=>m.id===args.id)!.thumbnail='custom.png';return;}
   if(cmd==='set_mod_enabled'){const mod=mods.find(m=>m.id===args.id)!;if(w.failToggle)throw new Error('Cannot change activation');mod.enabled=args.enabled;return mod.id;}
   return null;
  }};
 });
 await page.route('**/test-images/**',route=>route.request().url().endsWith('broken.png')?route.fulfill({status:404}):route.fulfill({contentType:'image/png',body:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64')}));
 await page.route('https://media.forgecdn.net/**',route=>route.fulfill({contentType:'image/png',body:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64')}));
 await page.goto('/');
 await expect(page.locator('.mod-row')).toHaveCount(12);
}

test('grid shows thumbnails, missing and failed images, and keeps selection across layouts',async({page})=>{
 await library(page);
 await expect(page.getByRole('button',{name:'Grid view',exact:true})).toHaveAttribute('aria-pressed','true');
 const cards=page.locator('.mod-row');
 const first=await cards.nth(0).boundingBox(),second=await cards.nth(1).boundingBox();
 expect(second!.x).toBeGreaterThan(first!.x);expect(second!.y).toBe(first!.y);
 await expect(cards.nth(0).locator('img')).toBeVisible();
 expect(await cards.nth(0).locator('img').evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBeGreaterThan(0);
 await expect(cards.nth(1).getByText('No thumbnail')).toBeVisible();
 await expect(cards.nth(2).getByText('No thumbnail')).toBeVisible();
 await expect(cards.nth(3).locator('img')).toHaveAttribute('src','https://media.forgecdn.net/avatars/mod.jpg');
 expect(await cards.nth(3).locator('img').evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBeGreaterThan(0);
 await cards.nth(3).click();
 await expect(page.getByRole('heading',{name:'Collection 4',exact:true})).toBeVisible();
 await page.getByRole('button',{name:'List view',exact:true}).click();
 await expect(cards.nth(3)).toHaveAttribute('aria-pressed','true');
 await page.reload();await expect(page.getByRole('button',{name:'List view',exact:true})).toHaveAttribute('aria-pressed','true');
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('Creator 10');
 await expect(cards).toHaveCount(1);await expect(cards.first()).toContainText('Collection 10');
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('missing');await expect(page.getByRole('heading',{name:'No mods found'})).toBeVisible();
});

test('grid keyboard moves by columns and preserves managed actions',async({page})=>{
 await library(page);const cards=page.locator('.mod-row');
 const columns=await cards.evaluateAll(nodes=>nodes.filter(n=>n.getBoundingClientRect().top===nodes[0].getBoundingClientRect().top).length);
 await cards.first().focus();await page.keyboard.press('ArrowDown');
 await expect(cards.nth(columns)).toBeFocused();await expect(cards.nth(columns)).toHaveAttribute('aria-pressed','true');
 await page.keyboard.press('ArrowRight');await expect(cards.nth(columns+1)).toBeFocused();
 await cards.first().click();await page.getByRole('button',{name:'Disable mod',exact:true}).click();
 await expect(cards.first().locator('.status')).toHaveText('Disabled');
});

test('choose thumbnail works for managed and external mods and updates the inspector',async({page})=>{
 await library(page);
 for(const index of [0,2]){
  await page.locator('.mod-row').nth(index).click();
  await page.getByRole('button',{name:'Choose thumbnail',exact:true}).click();
  await expect(page.locator('.mod-row').nth(index).locator('img')).toHaveAttribute('src','/test-images/custom.png');
  await expect(page.locator('.cover img')).toHaveAttribute('src','/test-images/custom.png');
 }
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='set_mod_thumbnail').map((c:any)=>c.args))).toEqual([{id:'mod-0',path:'C:/Pictures/custom.png'},{id:'mod-2',path:'C:/Pictures/custom.png'}]);
 await page.screenshot({path:'test-results/mod-grid.png',fullPage:true});
});

test('grid fits the minimum desktop width',async({page})=>{
 await page.setViewportSize({width:900,height:600});await library(page);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
 const card=await page.locator('.mod-row').first().boundingBox();const panel=await page.locator('.mod-list').boundingBox();
 expect(card!.width).toBeLessThanOrEqual(panel!.width);
 await expect(page.locator('footer')).toBeInViewport();
 await page.screenshot({path:'test-results/mod-grid-900.png',fullPage:true});
});

test('inspector resizes with drag and keyboard, remembers its width and clamps on a smaller window',async({page})=>{
 await library(page);
 const splitter=page.getByRole('separator',{name:'Resize details panel'});
 const panel=page.locator('.inspector-panel');
 const before=(await panel.boundingBox())!.width;
 const handle=(await splitter.boundingBox())!;
 await page.mouse.move(handle.x+handle.width/2,handle.y+40);
 await page.mouse.down();await page.mouse.move(handle.x+handle.width/2-130,handle.y+40,{steps:8});await page.mouse.up();
 await expect.poll(async()=>(await panel.boundingBox())!.width).toBe(before+130);
 await page.reload();await expect.poll(async()=>(await panel.boundingBox())!.width).toBe(before+130);
 await splitter.focus();await page.keyboard.press('ArrowRight');
 await expect(splitter).toBeFocused();await expect(splitter).toHaveAttribute('aria-valuenow',String(before+120));
 await page.keyboard.press('End');await expect(splitter).toHaveAttribute('aria-valuenow','600');
 await page.setViewportSize({width:900,height:600});
 await expect.poll(async()=>(await page.locator('.mod-list').boundingBox())!.width).toBeGreaterThanOrEqual(240);
 await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
 expect((await panel.boundingBox())!.width).toBeLessThan(600);
 await page.setViewportSize({width:1280,height:720});await expect(splitter).toHaveAttribute('aria-valuenow','600');
 await splitter.focus();await page.keyboard.press('Home');await expect(splitter).toHaveAttribute('aria-valuenow','240');
 await splitter.dblclick();await expect(splitter).toHaveAttribute('aria-valuenow','310');
});

test('inspector information starts collapsed and actions stay fixed while details scroll',async({page})=>{
 await page.setViewportSize({width:900,height:600});await library(page);
 const details=page.locator('.inspector details');
 await expect(details).toHaveCount(4);
 expect(await details.evaluateAll(nodes=>nodes.every(node=>!(node as HTMLDetailsElement).open))).toBe(true);
 await expect(page.locator('.description')).toBeHidden();
 const actions=page.locator('.inspector-actions');
 const before=await actions.boundingBox();
 const title=await page.locator('.inspector h2').boundingBox();
 expect(title!.y+title!.height).toBeLessThan(before!.y);
 await page.screenshot({path:'test-results/inspector-collapsed-900.png',fullPage:true});
 // Expand via the actual controls, then scroll the independent information area.
 for(const summary of await page.locator('.inspector summary').all())await summary.click();
 const scroll=page.locator('.inspector-scroll');
 await scroll.evaluate(node=>node.scrollTop=node.scrollHeight);
 expect(await scroll.evaluate(node=>node.scrollTop)).toBeGreaterThan(0);
 await expect(actions).toHaveCSS('flex-shrink','0');
 expect((await actions.boundingBox())!.y).toBe(before!.y);
 for(const name of ['Disable mod','Edit','Remove','Open folder','Choose thumbnail']){
  await expect(actions.getByRole('button',{name,exact:true})).toBeInViewport();
 }
 await actions.getByRole('button',{name:'Disable mod',exact:true}).click();
 await expect(actions.getByRole('button',{name:'Enable in game',exact:true})).toBeEnabled();
 await page.screenshot({path:'test-results/inspector-sticky-900.png',fullPage:true});
 await page.locator('.mod-row').nth(1).click();
 expect(await details.evaluateAll(nodes=>nodes.every(node=>!(node as HTMLDetailsElement).open))).toBe(true);
});


test('one library exposes direct switches for game and library mods, with status filters',async({page})=>{
 await library(page);
 await expect(page.getByRole('button',{name:/^Managed mods/})).toHaveCount(0);
 await expect(page.getByRole('button',{name:/^Installed in game/})).toHaveCount(0);
 await expect(page.getByRole('switch')).toHaveCount(12);
 const game=page.locator('.mod-card').filter({hasText:'Collection 3'});
 await game.getByRole('switch').click();
 await expect(game.getByRole('switch')).toHaveAttribute('aria-checked','false');
 await expect(game.locator('.status')).toHaveText('Disabled');
 await expect(page.locator('.mod-row')).toHaveCount(12);
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='set_mod_enabled').args)).toEqual({id:'mod-2',enabled:false});
 await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('enabled');
 await expect(page.locator('.mod-row')).toHaveCount(5);
 await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('disabled');
 await expect(page.locator('.mod-row')).toHaveCount(7);
 await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('all');
 await page.setViewportSize({width:900,height:600});
 await expect(page.getByRole('switch').first()).toBeInViewport();
 await page.screenshot({path:'test-results/unified-library-900.png',fullPage:true});
 expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
});

test('deleting an enabled mod confirms once and cancel preserves it',async({page})=>{
 await library(page);await page.locator('.mod-row').first().click();
 const remove=page.getByRole('button',{name:'Remove',exact:true});
 await expect(remove).toBeEnabled();
 await page.evaluate(()=>{(window as any).cancelRemoval=true});await remove.click();
 await expect(page.locator('.mod-row')).toHaveCount(12);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='remove_mod'))).toBe(false);
 await page.evaluate(()=>{(window as any).cancelRemoval=false});await remove.click();
 await expect(page.locator('.mod-row')).toHaveCount(11);
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='remove_mod').map((c:any)=>c.args))).toEqual([{id:'mod-0'}]);
});

test('failed activation keeps switch state and shows the error',async({page})=>{
 await library(page);await page.evaluate(()=>{(window as any).failToggle=true});
 const control=page.getByRole('switch').first();await control.click();
 await expect(page.getByRole('alert')).toContainText('Cannot change activation');
 await expect(control).toHaveAttribute('aria-checked','true');
 await expect(control).toBeEnabled();
});
