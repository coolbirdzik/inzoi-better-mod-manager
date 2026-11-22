import {test,expect} from '@playwright/test';
test('startup shows a splash, then a library loading state until the first scan finishes',async({page})=>{
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;let id=0;let release:()=>void=()=>{};
  w.finishScan=()=>release();
  const scanned=new Promise<void>(resolve=>{release=resolve});
  w.__TAURI_INTERNALS__={transformCallback:()=>++id,invoke:async(cmd:string)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   if(cmd==='snapshot'){await scanned;return {mods:[],settings:{mods_dir:'C:/test/Mods',library_dir:'C:/test/library'},warnings:[]}}
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return false;
   return null;
  }};
 });
 await page.goto('/');
 await expect(page.locator('#boot-splash')).toHaveCount(0);
 const loading=page.getByRole('status').filter({hasText:'Loading your mod library'});
 await expect(loading).toBeVisible();
 await expect(page.getByText('A fresh start for your library',{exact:false})).toHaveCount(0);
 await page.evaluate(()=>(window as any).finishScan());
 await expect(loading).toHaveCount(0);
 await expect(page.getByText('A fresh start for your library',{exact:false})).toBeVisible();
});
test('the splash is painted by the page itself before the app bundle runs',async({page})=>{
 await page.route('**/assets/*.js',route=>route.abort());
 await page.goto('/');
 const splash=page.locator('#boot-splash');
 await expect(splash).toBeVisible();
 await expect(splash).toContainText('Better Mod Manager');
});
