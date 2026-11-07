import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page,options:{savedKey?:string;holdLoad?:boolean;holdFirstSave?:boolean;failSave?:boolean;startupLink?:boolean}={}){
 let savedKey=options.savedKey??null;
 let releaseLoad=()=>{}, releaseSave=()=>{};
 const loadGate=new Promise<void>(resolve=>{releaseLoad=resolve});
 const saveGate=new Promise<void>(resolve=>{releaseSave=resolve});
 let saveCount=0;
 await page.exposeBinding('nativeCredentials',async(_,{cmd,apiKey}:{cmd:string;apiKey?:string})=>{
  if(cmd==='load_nexus_api_key'){if(options.holdLoad)await loadGate;return savedKey}
  if(options.holdFirstSave&&saveCount++===0)await saveGate;
  if(options.failSave)throw new Error('nexusKeySaveFailed');
  savedKey=apiKey||null;
  return null;
 });
 await page.addInitScript(({startupLink})=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='load_nexus_api_key'||cmd==='save_nexus_api_key')return w.nativeCredentials({cmd,...args});
   if(cmd==='snapshot')return {mods:[],settings:{mods_dir:'C:/Mods',library_dir:'C:/library',scan_subfolders:true},warnings:[]};
   if(cmd==='pending_links')return startupLink?[{request_id:'startup',mod_id:123,file_id:456,url:'https://www.nexusmods.com/inzoi/mods/123'}]:[];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='parse_nxm')return {request_id:'request-1',mod_id:123,file_id:456,url:'https://www.nexusmods.com/inzoi/mods/123'};
   if(cmd==='download_nxm')return {path:'C:/downloads/mod.zip',bytes:123,can_import:false};
   return null;
  }};
 },{startupLink:options.startupLink??false});
 await page.goto('/');
 return {releaseLoad,releaseSave,get savedKey(){return savedKey}};
}

test('Settings explains API keys and opens the exact Nexus settings URL',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByText('Open the Nexus API keys page, copy your API key, then paste it here.')).toBeVisible();
 const input=page.getByLabel('Nexus API key',{exact:true});
 await expect(input).toBeVisible();await expect(input).toHaveAttribute('type','password');
 await page.getByRole('button',{name:'Get API key from Nexus',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url')?.args.url)).toBe('https://www.nexusmods.com/settings/api-keys');
 await expect(page.getByRole('button',{name:'Sign in to Nexus',exact:true})).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd.startsWith('nexus_auth')||c.cmd==='nexus_login'))).toBe(false);
});

test('key is shared across dialogs and restored after restart without storing it in browser settings',async({page})=>{
 const credential=await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 await page.getByLabel('Nexus API key',{exact:true}).fill('  TEST_SESSION_KEY  ');
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^Downloads/}).click();
 await page.getByRole('textbox',{name:'Link',exact:true}).fill('nxm://inzoi/mods/123/files/456');
 await page.getByRole('button',{name:'Add download',exact:true}).click();
 await expect(page.getByRole('dialog',{name:'Download from Nexus'}).getByLabel('Nexus API key',{exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:'OK',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='download_nxm')?.args)).toEqual({requestId:'request-1',apiKey:'TEST_SESSION_KEY'});
 expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('TEST_SESSION_KEY');
 expect(credential.savedKey).toBe('TEST_SESSION_KEY');
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Nexus API key',{exact:true})).toHaveValue('TEST_SESSION_KEY');
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^Downloads/}).click();
 await page.getByRole('textbox',{name:'Link',exact:true}).fill('nxm://inzoi/mods/123/files/456');
 await page.getByRole('button',{name:'Add download',exact:true}).click();
 await expect(page.getByRole('dialog',{name:'Download from Nexus'}).getByLabel('Nexus API key',{exact:true})).toHaveCount(0);
 await expect(page.getByRole('button',{name:'OK',exact:true})).toBeEnabled();
});

test('clearing a key removes the saved credential and stays cleared after restart',async({page})=>{
 const credential=await desktop(page);await page.getByRole('button',{name:/^Downloads/}).click();
 await page.getByRole('textbox',{name:'Link',exact:true}).fill('nxm://inzoi/mods/123/files/456');
 await page.getByRole('button',{name:'Add download',exact:true}).click();
 const download=page.getByRole('button',{name:'OK',exact:true});
 await expect(download).toBeDisabled();
 const input=page.getByLabel('Nexus API key',{exact:true});
 await input.fill('   ');await expect(download).toBeDisabled();
 await input.fill('TEST_SESSION_KEY');await expect(download).toBeEnabled();
 await page.getByRole('button',{name:'Clear API key',exact:true}).click();
 await expect(input).toHaveValue('');await expect(download).toBeDisabled();
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Nexus API key',{exact:true})).toHaveValue('');
 await expect.poll(()=>credential.savedKey).toBe(null);
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Nexus API key',{exact:true})).toHaveValue('');
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='download_nxm'))).toBe(false);
});

test('startup Nexus links wait for saved key loading and do not request the key again',async({page})=>{
 const credential=await desktop(page,{savedKey:'RESTORED_FAKE_KEY',holdLoad:true,startupLink:true});
 await expect(page.getByRole('dialog',{name:'Download from Nexus'})).toHaveCount(0);
 credential.releaseLoad();
 const popup=page.getByRole('dialog',{name:'Download from Nexus'});
 await expect(popup).toBeVisible();await expect(popup.getByLabel('Nexus API key',{exact:true})).toHaveCount(0);
 await expect(popup.getByRole('button',{name:'OK',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='save_nexus_api_key'))).toBe(false);
});

test('clearing while a save is in progress wins over the earlier write',async({page})=>{
 const credential=await desktop(page,{holdFirstSave:true});
 await page.getByRole('button',{name:'Settings'}).click();
 await page.getByLabel('Nexus API key',{exact:true}).fill('FAKE_KEY_TO_REMOVE');
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='save_nexus_api_key').length)).toBe(1);
 await page.getByRole('button',{name:'Clear API key',exact:true}).click();
 credential.releaseSave();
 await expect(page.locator('#nexus-api-key-hint')).toContainText('remembered securely');
 expect(credential.savedKey).toBe(null);
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Nexus API key',{exact:true})).toHaveValue('');
});

test('credential storage failure is visible and never falls back to browser storage',async({page})=>{
 await desktop(page,{failSave:true});await page.getByRole('button',{name:'Settings'}).click();
 await page.getByLabel('Nexus API key',{exact:true}).fill('FAKE_KEY_MUST_NOT_LEAK');
 await expect(page.getByRole('dialog').getByRole('alert')).toContainText('Could not save or remove');
 expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('FAKE_KEY_MUST_NOT_LEAK');
 expect(await page.getByRole('dialog').getByRole('alert').textContent()).not.toContain('FAKE_KEY_MUST_NOT_LEAK');
});

test('Vietnamese exposes the same key-page instructions',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 await page.getByRole('dialog').locator('select').selectOption('vi');
 await expect(page.getByRole('button',{name:'Lấy API key từ Nexus',exact:true})).toBeVisible();
 await expect(page.getByText('Mở trang API keys của Nexus, sao chép API key rồi dán vào ô bên dưới.')).toBeVisible();
});
