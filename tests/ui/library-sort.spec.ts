import { test, expect, type Page } from '@playwright/test';

async function library(page:Page, options:{saved?:string;blockedStorage?:boolean}={}) {
 await page.addInitScript(options=>{
  if(options.saved && !sessionStorage.getItem('sort-seeded')) {
   localStorage.setItem('inzoimm.librarySort',options.saved);sessionStorage.setItem('sort-seeded','true');
  }
  if(options.blockedStorage) {
   Storage.prototype.getItem=()=>{throw new Error('Storage denied')};
   Storage.prototype.setItem=()=>{throw new Error('Storage denied')};
  }
  const w=window as any;w.isTauri=true;w.available=['old','middle'];w.mods=[
   {id:'old',name:'Mod 10',added_at:1000,enabled:false},
   {id:'unknown',name:'Unknown date',enabled:false},
   {id:'new',name:'Mod 2',added_at:3000,enabled:true},
   {id:'middle',name:'Alpha',added_at:2000,enabled:true},
   {id:'hidden',name:'Hidden previous version',added_at:4000,superseded_by:'new',enabled:false}
  ].map(m=>({author:'Creator',project:m.id,path:`C:/Library/${m.id}`,version:'1',description:'',thumbnail:null,source:'https://www.nexusmods.com/inzoi/mods/42',managed:true,tags:['Clothes'],...m}));
  w.__TAURI_INTERNALS__={transformCallback:()=>1,invoke:async(cmd:string,args:any)=>{
   if(cmd==='snapshot')return {mods:w.mods,settings:{mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true},warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='load_nexus_api_key')return 'TEST_KEY';
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='check_mod_update')return {id:args.id,provider:'nexus',mod_id:42,current_version:'1',latest_version:'2',status:w.available.includes(args.id)?'available':'upToDate',token:'token',file_id:2};
   return null;
  }};
 },options);
 await page.goto('/');await expect(page.locator('.mod-row')).toHaveCount(4);
}
const names=(page:Page)=>page.locator('.mod-row .mod-title strong');
const sort=(page:Page)=>page.getByRole('combobox',{name:'Sort by',exact:true});
const badge=(page:Page)=>page.locator('aside .mod-update-count');
async function check(page:Page) {
 const button=page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/});await button.click();await expect(button).toBeEnabled();
}

test('default puts updates before newest and counts all current updates across filters and refreshes',async({page})=>{
 await library(page);
 await expect(sort(page)).toHaveValue('recommended');
 await expect(names(page)).toHaveText(['Mod 2','Alpha','Mod 10','Unknown date']);
 await expect(badge(page)).toHaveCount(0);
 await check(page);
 await expect(names(page)).toHaveText(['Alpha','Mod 10','Mod 2','Unknown date']);
 await expect(badge(page)).toHaveText('2');await expect(badge(page)).toHaveAttribute('aria-label','Updates (2)');
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('Mod 2');
 await expect(names(page)).toHaveText(['Mod 2']);await expect(badge(page)).toHaveText('2');
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('');
 await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('disabled');
 await expect(names(page)).toHaveText(['Mod 10','Unknown date']);await expect(badge(page)).toHaveText('2');
 await page.getByRole('button',{name:/^All mods/}).click();
 await page.evaluate(()=>{(window as any).mods=(window as any).mods.filter((m:any)=>m.id!=='middle')});
 await page.getByRole('button',{name:'Refresh library',exact:true}).click();await expect(badge(page)).toHaveText('1');
 await page.evaluate(()=>{(window as any).available=[]});await check(page);
 await expect(badge(page)).toHaveCount(0);await expect(names(page)).toHaveText(['Mod 2','Mod 10','Unknown date']);
});

test('sort modes persist across restart, layout, source, search and status changes',async({page})=>{
 await library(page);await check(page);
 await sort(page).selectOption('newest');await expect(names(page)).toHaveText(['Mod 2','Alpha','Mod 10','Unknown date']);
 await sort(page).selectOption('oldest');await expect(names(page)).toHaveText(['Mod 10','Alpha','Mod 2','Unknown date']);
 await sort(page).selectOption('nameAsc');await expect(names(page)).toHaveText(['Alpha','Mod 2','Mod 10','Unknown date']);
 await page.locator('.mod-row').filter({hasText:'Mod 10'}).click();
 await sort(page).selectOption('nameDesc');await expect(names(page)).toHaveText(['Unknown date','Mod 10','Mod 2','Alpha']);
 await expect(page.locator('.mod-row').filter({hasText:'Mod 10'})).toHaveAttribute('aria-pressed','true');
 await page.getByRole('button',{name:'List view',exact:true}).click();
 await page.getByRole('combobox',{name:'Browse a source',exact:true}).selectOption('C:/Library');
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('Mod');
 await expect(names(page)).toHaveText(['Mod 10','Mod 2']);
 await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('enabled');await expect(names(page)).toHaveText(['Mod 2']);
 await page.reload();await expect(sort(page)).toHaveValue('nameDesc');await expect(names(page)).toHaveText(['Unknown date','Mod 10','Mod 2','Alpha']);
 expect(await page.evaluate(()=>localStorage.getItem('inzoimm.librarySort'))).toBe('nameDesc');
 await page.getByRole('tab',{name:/Source folders/}).click();await expect(sort(page)).toHaveCount(0);
 await page.getByRole('tab',{name:'Mods',exact:true}).click();await expect(sort(page)).toHaveValue('nameDesc');
 for(const width of [900,1280]) {
  await page.setViewportSize({width,height:600});
  await expect(sort(page)).toBeVisible();await expect(page.getByRole('button',{name:'Grid view',exact:true})).toBeVisible();
  const bounds=await sort(page).boundingBox(), panel=await page.locator('.workspace').boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(panel!.x);expect(bounds!.x+bounds!.width).toBeLessThanOrEqual(panel!.x+panel!.width);
  const controls=await page.locator('.library-scopes>.source-scope,.library-scopes>.status-filter,.library-scopes>.library-sort,.library-toolbar>.view-switch').evaluateAll(nodes=>nodes.map(node=>{const r=node.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}}));
  expect(controls).toHaveLength(4);
  for(const [index,control] of controls.entries()) {
   expect(control.height).toBe(30);expect(control.y).toBe(controls[0].y);
   if(index)expect(control.x).toBeGreaterThanOrEqual(controls[index-1].x+controls[index-1].width);
  }
  expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(width);
  await page.screenshot({path:`test-results/library-sort-${width}.png`});
 }
});

test('invalid saved sorting falls back to the default',async({page})=>{
 await library(page,{saved:'not-a-sort'});await expect(sort(page)).toHaveValue('recommended');
 await expect(names(page)).toHaveText(['Mod 2','Alpha','Mod 10','Unknown date']);
});

test('sorting still works when preferences cannot be read or written',async({page})=>{
 await library(page,{blockedStorage:true});await expect(sort(page)).toHaveValue('recommended');
 await sort(page).selectOption('oldest');await expect(names(page)).toHaveText(['Mod 10','Alpha','Mod 2','Unknown date']);
 await page.getByRole('button',{name:'List view',exact:true}).click();await expect(sort(page)).toHaveValue('oldest');
});
