import {test, expect, type Page} from '@playwright/test';

async function library(page:Page) {
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;let callback=0;
  const manifest=(patches:string[])=>({'mod_manifest.json':{NonAssets:patches.map(p=>`/non_ufs/JsonPatch/${p}_patch.autosave.json`)}});
  const mod=(id:string,name:string,metadata:Record<string,unknown>={})=>({id,name,author:'Creator',project:id,version:'1.0',description:'',path:`C:/Mods/${id}`,thumbnail:null,managed:true,enabled:true,source:'',metadata});
  const mods=[
   mod('dress','Silk Dress',manifest(['GarmentFullMesh','GarmentFull'])),
   mod('hair','Wavy Hair',{'Game library':{primaryCategoryId:8380,categories:[{iD:8380,name:'Hairstyle'}]}}),
   mod('script','Camera Script',{'mod_manifest.json':{nonAssets:['/non_ufs/Lua/Camera.lua']}}),
   mod('heels','Strap Heels',manifest(['GarmentShoesMesh','GarmentShoes'])),
   mod('cf-other','Earrings',{...manifest(['AttachedAccessory']),'curseforge.json':{Download:{mod:{primaryCategoryId:1,categories:[{id:1,name:'Others'}]}}}}),
   mod('plain','Mystery Pak'),
   mod('lipstick','Ruby Lipstick',manifest(['AppearanceMakeup'])),
   mod('new-category','Cozy Sofa',{'curseforge.json':{Download:{mod:{categories:[{id:9,name:'Build Mode'}]}}}}),
  ];
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,convertFileSrc:(path:string)=>path,invoke:async(cmd:string)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   if(cmd==='snapshot')return {mods,settings:{mods_dir:'C:/Mods',library_dir:'C:/library'},warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   return null;
  }};
 });
 await page.goto('/');
 await expect(page.locator('.mod-row')).toHaveCount(8);
}

const headings=(page:Page)=>page.locator('.mod-group-head span').allTextContents();
const order=(page:Page)=>page.locator('.mod-row strong').allTextContents();

test('mods are grouped by provider category or manifest patches, largest first and Other last',async({page})=>{
 await library(page);
 expect(await headings(page)).toEqual(['Outfits','Accessories','Build Mode','Face & makeup','Gameplay & scripts','Hairstyles','Other']);
 expect(await order(page)).toEqual(['Silk Dress','Strap Heels','Earrings','Cozy Sofa','Ruby Lipstick','Camera Script','Wavy Hair','Mystery Pak']);
 await expect(page.locator('.mod-group-head').first()).toContainText('2');
 // Searching matches the category name too.
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('hairstyles');
 expect(await order(page)).toEqual(['Wavy Hair']);
 await expect(page.locator('.mod-group-head')).toHaveCount(0);
 await page.screenshot({path:'test-results/mod-categories-search.png'});
});

test('keyboard follows grouped order and grouping can be turned off and stays off',async({page})=>{
 await library(page);
 await page.getByRole('button',{name:'List view',exact:true}).click();
 const cards=page.locator('.mod-row');
 await cards.first().focus();
 for(const name of ['Strap Heels','Earrings','Cozy Sofa']){await page.keyboard.press('ArrowDown');await expect(page.locator('.mod-row:focus strong')).toHaveText(name)}
 await page.getByRole('button',{name:'Grid view',exact:true}).click();
 await cards.nth(1).click();await cards.nth(1).focus();await page.keyboard.press('ArrowDown');
 await expect(page.locator('.mod-row:focus strong')).toHaveText('Earrings');
 await page.screenshot({path:'test-results/mod-categories-grid.png'});
 const toggle=page.getByRole('button',{name:'Group by category',exact:true});
 await expect(toggle).toHaveAttribute('aria-pressed','true');
 await toggle.click();
 await expect(page.locator('.mod-group-head')).toHaveCount(0);
 expect(await order(page)).toEqual(['Silk Dress','Wavy Hair','Camera Script','Strap Heels','Earrings','Mystery Pak','Ruby Lipstick','Cozy Sofa']);
 await page.reload();await expect(cards).toHaveCount(8);
 await expect(page.getByRole('button',{name:'Group by category',exact:true})).toHaveAttribute('aria-pressed','false');
 await expect(page.locator('.mod-group-head')).toHaveCount(0);
});
