import {test, expect, type Page} from '@playwright/test';

async function library(page:Page) {
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  const mods=Array.from({length:12},(_,i)=>({id:`mod-${i}`,name:`Collection ${i+1}`,author:`Creator ${i+1}`,project:`Project_${i}`,version:'1.0',description:`Description ${i+1}`,path:`C:/Mods/${i}`,thumbnail:i===0?'preview.png':i===1?'broken.png':i===3?'https://media.forgecdn.net/avatars/mod.jpg':null,managed:i!==2,enabled:i%2===0,source:''}));
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,convertFileSrc:(path:string)=>`/test-images/${path}`,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods,settings:{mods_dir:'C:/Mods',library_dir:'C:/library'},warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return 'C:/Pictures/custom.png';
   if(cmd==='set_mod_thumbnail'){mods.find(m=>m.id===args.id)!.thumbnail='custom.png';return;}
   if(cmd==='toggle_mod'){const mod=mods.find(m=>m.id===args.id)!;mod.enabled=!mod.enabled;return;}
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
 const columns=await cards.evaluateAll(nodes=>nodes.filter(n=>(n as HTMLElement).offsetTop===(nodes[0] as HTMLElement).offsetTop).length);
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
