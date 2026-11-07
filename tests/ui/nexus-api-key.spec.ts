import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page){
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods:[],settings:{mods_dir:'C:/Mods',library_dir:'C:/library',scan_subfolders:true},warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='parse_nxm')return {request_id:'request-1',mod_id:123,file_id:456,url:'https://www.nexusmods.com/inzoi/mods/123'};
   if(cmd==='download_nxm')return {path:'C:/downloads/mod.zip',bytes:123,can_import:false};
   return null;
  }};
 });
 await page.goto('/');
}

test('Settings explains API keys and opens the exact Nexus settings URL',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByText('Open the Nexus API keys page, copy your API key, then paste it here.')).toBeVisible();
 const input=page.getByLabel('Nexus API key (this session only)',{exact:true});
 await expect(input).toBeVisible();await expect(input).toHaveAttribute('type','password');
 await page.getByRole('button',{name:'Get API key from Nexus',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url')?.args.url)).toBe('https://www.nexusmods.com/settings/api-keys');
 await expect(page.getByRole('button',{name:'Sign in to Nexus',exact:true})).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd.startsWith('nexus_auth')||c.cmd==='nexus_login'))).toBe(false);
});

test('one session key is shared across Settings and Nexus links and used for downloading',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 await page.getByLabel('Nexus API key (this session only)',{exact:true}).fill('  TEST_SESSION_KEY  ');
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^Downloads/}).click();
 await page.getByRole('textbox',{name:'Link',exact:true}).fill('nxm://inzoi/mods/123/files/456');
 await page.getByRole('button',{name:'Add download',exact:true}).click();
 await expect(page.getByRole('dialog',{name:'Download from Nexus'}).getByLabel('Nexus API key (this session only)',{exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:'OK',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='download_nxm')?.args)).toEqual({requestId:'request-1',apiKey:'TEST_SESSION_KEY'});
 expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('TEST_SESSION_KEY');
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Nexus API key (this session only)',{exact:true})).toHaveValue('');
});

test('download requires a nonempty key and clearing it applies in both dialogs',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:/^Downloads/}).click();
 await page.getByRole('textbox',{name:'Link',exact:true}).fill('nxm://inzoi/mods/123/files/456');
 await page.getByRole('button',{name:'Add download',exact:true}).click();
 const download=page.getByRole('button',{name:'OK',exact:true});
 await expect(download).toBeDisabled();
 const input=page.getByLabel('Nexus API key (this session only)',{exact:true});
 await input.fill('   ');await expect(download).toBeDisabled();
 await input.fill('TEST_SESSION_KEY');await expect(download).toBeEnabled();
 await page.getByRole('button',{name:'Clear API key',exact:true}).click();
 await expect(input).toHaveValue('');await expect(download).toBeDisabled();
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Nexus API key (this session only)',{exact:true})).toHaveValue('');
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='download_nxm'))).toBe(false);
});

test('Vietnamese exposes the same key-page instructions',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 await page.getByRole('dialog').locator('select').selectOption('vi');
 await expect(page.getByRole('button',{name:'Lấy API key từ Nexus',exact:true})).toBeVisible();
 await expect(page.getByText('Mở trang API keys của Nexus, sao chép API key rồi dán vào ô bên dưới.')).toBeVisible();
});
