import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page){
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];w.downloads={};w.callbacks={};w.listeners=[];let callback=0;
  let settings=JSON.parse(localStorage.getItem('downloads.settings')||'null')??{mods_dir:'C:/Mods',library_dir:'C:/Library',scan_subfolders:true};
  const mods:any[]=[];
  w.emit=(event:string,payload:any)=>{for(const listener of w.listeners.filter((l:any)=>l.event===event))w.callbacks[listener.handler]({event,id:0,payload})};
  w.__TAURI_INTERNALS__={transformCallback:(fn:any)=>{w.callbacks[++callback]=fn;return callback},invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='plugin:event|listen'){w.listeners.push(args);return w.listeners.length;}
   if(cmd==='snapshot')return {mods,settings,warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return 'D:/Nexus downloads';
   if(cmd==='save_settings'){settings={...args.settings};localStorage.setItem('downloads.settings',JSON.stringify(settings));}
   if(cmd==='download_nxm')return new Promise((resolve,reject)=>{w.downloads[args.requestId]={resolve,reject}});
   if(cmd==='cancel_nxm'){w.downloads[args.requestId].reject('downloadCancelled');}
   if(cmd==='import_download'){
    if(w.failImports)throw 'Import failed';
    mods.push({id:args.path,name:'Downloaded outfit',project:args.path,author:'Creator',version:'1',path:args.path.replace(/mod\.zip$/,'Imported'),managed:true,enabled:false,source:'',description:'',thumbnail:null});
   }
   return null;
  }};
 });
 await page.goto('/');await page.getByRole('button',{name:'Settings'}).click();
 await page.getByLabel('Nexus API key (this session only)',{exact:true}).fill('SESSION_KEY');await page.keyboard.press('Escape');
}
async function send(page:Page,id:number){
 await page.evaluate(id=>(window as any).emit('nexus-link',{request_id:`job-${id}`,mod_id:id,file_id:id*10,url:`https://www.nexusmods.com/inzoi/mods/${id}`}),id);
 await expect(page.getByRole('dialog',{name:'Download from Nexus'})).toBeVisible();
}
async function finish(page:Page,id:number,zip=true){
 await page.evaluate(({id,zip})=>(window as any).downloads[`job-${id}`].resolve({path:`D:/Nexus downloads/Outfit [nexus-${id}]/files/file-${id*10}/mod.${zip?'zip':'7z'}`,bytes:1048576,can_import:zip}),{id,zip});
}

test('incoming Nexus popup needs only OK; ZIP downloads import into Managed mods in the background',async({page})=>{
 await desktop(page);await send(page,1);
 const popup=page.getByRole('dialog',{name:'Download from Nexus'});
 await expect(popup.getByRole('button',{name:'OK',exact:true})).toBeFocused();
 await expect(popup.getByRole('textbox')).toHaveCount(0);
 await expect(popup).toContainText('C:/Library/Downloads');
 await page.screenshot({path:'test-results/nexus-confirm.png',fullPage:true});
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(0);
 await popup.getByRole('button',{name:'OK',exact:true}).click();await expect(popup).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Add mod',exact:true})).toBeEnabled();
 await page.getByRole('button',{name:/^Downloads/}).click();
 const panel=page.getByRole('dialog',{name:'Downloads',exact:true});await expect(panel.getByText('Downloading',{exact:true})).toBeVisible();
 await page.evaluate(()=>(window as any).emit('nexus-download-progress',{request_id:'job-1',received:524288,total:1048576}));
 await expect(panel.getByRole('progressbar')).toHaveAttribute('value','524288');
 await finish(page,1);await expect(panel.getByText('Added to Managed mods',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='import_download').args)).toEqual({path:'D:/Nexus downloads/Outfit [nexus-1]/files/file-10/mod.zip'});
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^Managed mods \d+$/}).click();
 await expect(page.locator('.mod-row')).toContainText('Downloaded outfit');
 await expect(page.getByRole('button',{name:'Enable in game',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('SESSION_KEY');
});

test('queue runs one download at a time, can pause waiting work, cancel, and retry',async({page})=>{
 await desktop(page);await send(page,1);await page.getByRole('button',{name:'OK',exact:true}).click();
 await send(page,2);await page.getByRole('button',{name:'OK',exact:true}).click();
 await page.getByRole('button',{name:/^Downloads/}).click();const panel=page.getByRole('dialog',{name:'Downloads',exact:true});
 await expect(panel.getByText('Waiting',{exact:true})).toHaveCount(1);
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await panel.getByRole('button',{name:'Pause queue',exact:true}).click();await finish(page,1);
 await expect(panel.getByText('Added to Managed mods',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await panel.getByRole('button',{name:'Resume queue',exact:true}).click();
 await expect(panel.getByText('Downloading',{exact:true})).toBeVisible();await panel.getByRole('button',{name:'Cancel',exact:true}).click();
 await expect(panel.getByText('Cancelled',{exact:true})).toBeVisible();
 await panel.getByRole('button',{name:'Retry download',exact:true}).click();await expect(panel.getByText('Downloading',{exact:true})).toBeVisible();
 await finish(page,2);await expect(panel.getByText('Added to Managed mods',{exact:true})).toHaveCount(2);
 await panel.getByRole('button',{name:'Clear finished',exact:true}).click();await expect(panel.getByText('No downloads yet',{exact:true})).toBeVisible();
});

test('download destination persists, cancel never downloads, and Downloads stays at the bottom',async({page})=>{
 await page.setViewportSize({width:900,height:600});await desktop(page);
 const button=page.getByRole('button',{name:/^Downloads/});await expect(button).toBeInViewport();
 await button.click();await page.getByRole('button',{name:'Change',exact:true}).click();
 await expect(page.getByRole('dialog')).toContainText('D:/Nexus downloads');
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Default download folder',{exact:true})).toHaveValue('D:/Nexus downloads');
 await page.getByLabel('Nexus API key (this session only)',{exact:true}).fill('SESSION_KEY');await page.keyboard.press('Escape');
 await send(page,1);await expect(page.getByRole('dialog')).toContainText('D:/Nexus downloads');
 await page.getByRole('button',{name:'Cancel',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='download_nxm'))).toBe(false);
 await page.screenshot({path:'test-results/download-button-900.png',fullPage:true});
});

test('failed imports retain the archive and retry importing without downloading it again',async({page})=>{
 await desktop(page);await page.evaluate(()=>{(window as any).failImports=true});
 await send(page,1);await page.getByRole('button',{name:'OK',exact:true}).click();await page.getByRole('button',{name:/^Downloads/}).click();
 await finish(page,1);const panel=page.getByRole('dialog',{name:'Downloads',exact:true});
 await expect(panel.getByText('Could not add to Managed mods',{exact:true})).toBeVisible();
 await expect(panel.getByRole('button',{name:'Open folder',exact:true})).toBeEnabled();
 await page.evaluate(()=>{(window as any).failImports=false});await panel.getByRole('button',{name:'Retry import',exact:true}).click();
 await expect(panel.getByText('Added to Managed mods',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await page.screenshot({path:'test-results/download-queue.png',fullPage:true});
});

test('7z/RAR remain available and are clearly marked as requiring extraction',async({page})=>{
 await desktop(page);await send(page,1);await page.getByRole('button',{name:'OK',exact:true}).click();await page.getByRole('button',{name:/^Downloads/}).click();
 await finish(page,1,false);await expect(page.getByText('Downloaded · extraction needed',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='import_download'))).toBe(false);
 await expect(page.getByRole('button',{name:'Open folder',exact:true})).toBeEnabled();
});
