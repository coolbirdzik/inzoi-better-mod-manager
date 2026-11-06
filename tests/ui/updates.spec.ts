import {test,expect,type Page} from '@playwright/test';
async function mockDesktop(page:Page,portable=false,fail=false){
 await page.addInitScript(({portable,fail})=>{
  const w=window as any;w.isTauri=true;w.calls=[];let id=0;
  w.__TAURI_INTERNALS__={transformCallback:()=>++id,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods:[],settings:{mods_dir:'C:/test/Mods',library_dir:'C:/test/library'}};
   if(cmd==='pending_links')return w.pending??[];
   if(cmd==='is_portable')return portable;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:updater|check')return {rid:1,currentVersion:'0.1.3',version:'0.1.4'};
   if(cmd==='plugin:updater|download_and_install'&&fail)throw new Error('Invalid signature');
   return null;
  }};
 },{portable,fail});
}
test('startup downloads, installs and restarts without user interaction',async({page})=>{
 await mockDesktop(page);await page.goto('/');
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.map((c:any)=>c.cmd))).toContain('plugin:process|restart');
 const install=await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:updater|download_and_install'));
 expect(install.args.restartAfterInstall).toBe(true);
});
test('portable uses its replacement updater and checks again while open',async({page})=>{
 await mockDesktop(page,true);await page.clock.install();await page.goto('/');
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='update_portable').length)).toBe(1);
 await page.clock.fastForward(60_001);
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='update_portable').length)).toBe(2);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:updater|download_and_install'))).toBe(false);
});
test('failed verification never restarts and updates defer while editing settings',async({page})=>{
 await mockDesktop(page,false,true);await page.clock.install();await page.goto('/');
 await expect(page.getByText('Invalid signature',{exact:false})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:process|restart'))).toBe(false);
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
