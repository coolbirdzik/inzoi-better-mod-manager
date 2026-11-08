import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page,keys:{nexus?:string;curseforge?:string}={}){
 await page.addInitScript(k=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;const saved:any={nexus:k.nexus??null,curseforge:k.curseforge??null};
  const settings={mods_dir:'C:/Mods',library_dir:'C:/library',scan_subfolders:true};
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return{mods:[],settings,warnings:[]};
   if(cmd==='pending_links')return[];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='load_nexus_api_key')return saved.nexus;
   if(cmd==='load_curseforge_api_key')return saved.curseforge;
   if(cmd==='save_nexus_api_key')saved.nexus=args.apiKey||null;
   if(cmd==='save_curseforge_api_key')saved.curseforge=args.apiKey||null;
   return null;
  }};
 },keys);
 await page.goto('/');
}
const dialog=(page:Page)=>page.getByRole('dialog',{name:'Connect your accounts',exact:true});

test('first launch without keys asks for both before the app can be used, and badges stay until connected',async({page})=>{
 await desktop(page);
 const panel=dialog(page);await expect(panel).toBeVisible();
 await expect(panel.getByText('Not connected')).toHaveCount(2);
 await expect(panel.getByRole('button',{name:'Continue',exact:true})).toHaveCount(0);
 await expect(panel.getByRole('button',{name:'Skip for now',exact:true})).toBeEnabled();
 await panel.locator('#nexus-api-key').fill('NEXUS');await expect(panel.getByRole('button',{name:'Continue',exact:true})).toHaveCount(0);
 await expect(panel.getByText('Connected',{exact:true})).toHaveCount(1);
 await panel.locator('#curseforge-api-key').fill('CF');const next=panel.getByRole('button',{name:'Continue',exact:true});await expect(next).toBeEnabled();
 await next.click();await expect(panel).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd.startsWith('save_')).map((c:any)=>c.cmd))).toContain('save_curseforge_api_key');
 await expect(page.getByRole('status',{name:/missing/})).toHaveCount(0);
});

test('one missing key shows the dialog, can be closed only by continuing, and leaves a badge on Settings if cleared later',async({page})=>{
 await desktop(page,{nexus:'NEXUS'});
 const panel=dialog(page);await expect(panel).toBeVisible();
 await expect(panel.getByText('Connected',{exact:true})).toHaveCount(1);
 await page.keyboard.press('Escape');await expect(panel).toBeVisible();
 await panel.locator('#curseforge-api-key').fill('CF');await panel.getByRole('button',{name:'Continue',exact:true}).click();
 await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByRole('status',{name:/missing/})).toHaveCount(0);
 await page.getByRole('dialog').locator('#nexus-api-key').fill('');
 await page.keyboard.press('Escape');
 await expect(page.getByRole('status',{name:'1 API key(s) missing'})).toBeVisible();
});

test('with both keys saved there is no onboarding and no badge',async({page})=>{
 await desktop(page,{nexus:'NEXUS',curseforge:'CF'});
 await expect(page.getByRole('button',{name:'Settings'})).toBeEnabled();
 await expect(dialog(page)).toHaveCount(0);
 await expect(page.getByRole('status',{name:/missing/})).toHaveCount(0);
});

test('skipping leaves keys empty, keeps the badge and skips update checks for providers without a key',async({page})=>{
 await desktop(page);
 await dialog(page).getByRole('button',{name:'Skip for now',exact:true}).click();
 await expect(dialog(page)).toHaveCount(0);
 await expect(page.getByRole('status',{name:'2 API key(s) missing'})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd.startsWith('save_')))).toBe(false);
 await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByRole('dialog').getByText('Not connected')).toHaveCount(2);
});
