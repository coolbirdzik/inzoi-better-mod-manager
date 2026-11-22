import {test,expect,type Page} from '@playwright/test';
async function mockDesktop(page:Page,portable=false,fail=false){
 await page.addInitScript(({portable,fail})=>{
  const w=window as any;w.isTauri=true;w.calls=[];let id=0;
  w.__TAURI_INTERNALS__={transformCallback:()=>++id,invoke:async(cmd:string,args:any)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods:[],settings:{mods_dir:'C:/test/Mods',library_dir:'C:/test/library'},warnings:[]};
   if(cmd==='pending_links')return w.pending??[];
   if(cmd==='is_portable')return portable;
   if(cmd==='update_portable')return false;
   if(cmd==='check_portable_update')return w.portableVersion??null;
   if(cmd==='plugin:updater|check'){if(w.primaryError)throw new Error(w.primaryError);return w.noUpdate?null:{rid:1,currentVersion:'0.1.3',version:'0.1.4'}}
   if(cmd==='check_app_update_fallback'){if(w.apiError)throw new Error(w.apiError);return w.fallbackMetadata??null}
   if(cmd==='plugin:updater|download_and_install'&&fail)throw new Error('Invalid signature');
   return null;
  }};
 },{portable,fail});
}
test('startup only offers the update and installs after the user chooses to',async({page})=>{
 await mockDesktop(page);await page.clock.install();await page.goto('/');
 const prompt=page.getByRole('status',{name:'App updates'});
 await expect(prompt).toContainText('Version 0.1.4 available');
 await page.clock.fastForward(60_001);
 const cmds=()=>page.evaluate(()=>(window as any).calls.map((c:any)=>c.cmd));
 expect(await cmds()).not.toContain('plugin:updater|download_and_install');
 expect(await cmds()).not.toContain('plugin:process|restart');
 await prompt.getByRole('button',{name:'Download and restart',exact:true}).click();
 await expect.poll(cmds).toContain('plugin:process|restart');
 const install=await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:updater|download_and_install'));
 expect(install.args.restartAfterInstall).toBe(true);
});
test('later hides the offer without installing until the user asks again',async({page})=>{
 await mockDesktop(page);await page.clock.install();await page.goto('/');
 const prompt=page.getByRole('status',{name:'App updates'});
 await prompt.getByRole('button',{name:'Later',exact:true}).click();
 await expect(prompt).toHaveCount(0);
 await page.clock.fastForward(60_001);
 await expect(prompt).toHaveCount(0);
 const calls=await page.evaluate(()=>(window as any).calls.map((c:any)=>c.cmd));
 expect(calls.filter((c:string)=>c==='plugin:updater|check').length).toBe(1);
 expect(calls).not.toContain('plugin:updater|download_and_install');
 await page.getByRole('button',{name:'App updates'}).click();
 await expect(prompt).toContainText('Version 0.1.4 available');
});
test('portable checks without installing and keeps checking while open',async({page})=>{
 await mockDesktop(page,true);await page.clock.install();await page.goto('/');
 const count=(cmd:string)=>page.evaluate(cmd=>(window as any).calls.filter((c:any)=>c.cmd===cmd).length,cmd);
 await expect.poll(()=>count('check_portable_update')).toBe(1);
 await page.clock.fastForward(60_001);
 await expect.poll(()=>count('check_portable_update')).toBe(2);
 expect(await count('update_portable')).toBe(0);
 expect(await count('plugin:updater|download_and_install')).toBe(0);
});
test('portable update is applied only after the user chooses to',async({page})=>{
 await page.addInitScript(()=>{(window as any).portableVersion='0.1.4'});
 await mockDesktop(page,true);await page.goto('/');
 const prompt=page.getByRole('status',{name:'App updates'});
 await expect(prompt).toContainText('Version 0.1.4 available');
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='update_portable'))).toBe(false);
 await prompt.getByRole('button',{name:'Download and restart',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='update_portable'))).toBe(true);
});
test('failed verification never restarts',async({page})=>{
 await mockDesktop(page,false,true);await page.clock.install();await page.goto('/');
 await page.getByRole('status',{name:'App updates'}).getByRole('button',{name:'Download and restart',exact:true}).click();
 await expect(page.getByText('Invalid signature',{exact:false})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:process|restart'))).toBe(false);
});
test('update checks defer while editing settings',async({page})=>{
 await page.addInitScript(()=>{(window as any).noUpdate=true});
 await mockDesktop(page);await page.clock.install();await page.goto('/');
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='plugin:updater|check').length)).toBe(1);
 await page.getByRole('button',{name:'Settings'}).click();await page.clock.fastForward(60_001);
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='plugin:updater|check').length)).toBe(1);
 await page.keyboard.press('Escape');await page.clock.fastForward(60_001);
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='plugin:updater|check').length)).toBe(2);
});

test('incoming Nexus link waits through the startup update check',async({page})=>{
 await mockDesktop(page,true);
 await page.addInitScript(()=>{(window as any).pending=[{request_id:'test-link',mod_id:123,file_id:456,url:'https://www.nexusmods.com/inzoi/mods/123'}]});
 await page.goto('/');
 await expect(page.getByRole('dialog')).toBeVisible();
 await expect(page.getByText('inZOI · Mod #123 · File #456',{exact:false})).toBeVisible();
});

test('release gateway failure falls back to GitHub API and up-to-date result clears the error',async({page})=>{
 await mockDesktop(page);await page.addInitScript(()=>{(window as any).primaryError='Could not fetch a valid release JSON from the remote'});
 await page.goto('/');
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.map((c:any)=>c.cmd))).toContain('check_app_update_fallback');
 await expect(page.getByText('App is up to date',{exact:true})).toBeVisible();
 await expect(page.getByRole('status',{name:'App updates'})).toHaveCount(0);
 await expect(page.getByRole('alert')).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Scan computer',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:updater|download_and_install'))).toBe(false);
});

test('fallback uses the official updater resource and never bypasses signature failure',async({page})=>{
 await mockDesktop(page,false,true);await page.addInitScript(()=>{const w=window as any;w.primaryError='Could not fetch a valid release JSON from the remote';w.fallbackMetadata={rid:88,currentVersion:'0.1.3',version:'0.1.4',rawJson:{version:'0.1.4'}}});
 await page.goto('/');
 await page.getByRole('status',{name:'App updates'}).getByRole('button',{name:'Download and restart',exact:true}).click();
 await expect(page.getByRole('status',{name:'App updates'})).toContainText('Invalid signature');
 const calls=await page.evaluate(()=>(window as any).calls);
 expect(calls.find((c:any)=>c.cmd==='plugin:updater|download_and_install').args.rid).toBe(88);
 expect(calls.some((c:any)=>c.cmd==='plugin:process|restart')).toBe(false);
 await expect(page.getByRole('alert')).toHaveCount(0);
});

test('unavailable update services have their own retryable notice while the library stays usable',async({page})=>{
 await mockDesktop(page);await page.addInitScript(()=>{const w=window as any;w.primaryError='Could not fetch a valid release JSON from the remote';w.apiError='GitHub release API returned HTTP 503'});
 await page.goto('/');const notice=page.getByRole('status',{name:'App updates'});
 await expect(notice).toContainText('Your mod library is still available');
 await expect(notice).toContainText('HTTP 503');await expect(page.getByRole('alert')).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Scan computer',exact:true})).toBeEnabled();
 await expect(page.getByRole('button',{name:'Settings'})).toBeEnabled();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:process|restart'))).toBe(false);
 await page.evaluate(()=>{const w=window as any;w.primaryError='';w.apiError=''});
 await notice.getByRole('button',{name:'Retry app update',exact:true}).click();
 await expect(notice).toContainText('Version 0.1.4 available');
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:process|restart'))).toBe(false);
});
