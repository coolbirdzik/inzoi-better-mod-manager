import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page,enabled=true,options:{count?:number;failCheck?:string;nativeLogs?:any[];logError?:string;clipboardFails?:boolean}={}){
 await page.addInitScript(({enabled,options})=>{
  const w=window as any;w.isTauri=true;w.calls=[];w.callbacks={};w.listeners=[];w.pendingDownload={};let cb=0,sequence=0;
  w.mods=JSON.parse(localStorage.getItem('mod-update.mods')||'null')??[
   {id:'nexus',name:'Nexus Hair',project:'Hair',path:'C:/Library/old',enabled,source:'https://www.nexusmods.com/inzoi/mods/42',metadata:{'nexus.json':{Download:{mod_id:42,file_id:1,file:{version:'1.0'}}}}},
   {id:'cf',name:'CF Dress',project:'Dress',path:'C:/Library/dress',enabled:false,source:'',metadata:{'curseforge.json':{Download:{provider:'curseforge',mod_id:123,file_id:1}}}},
   {id:'local',name:'Local Mod',project:'Local',path:'C:/Library/local',enabled:false,source:''}
  ].map(m=>({author:'Creator',version:'1.0',description:'',managed:true,thumbnail:null,...m}));
  if(options.count)w.mods=Array.from({length:options.count},(_,index)=>({...w.mods[1],id:`cf-${index}`,name:`CF Dress ${index+1}`,project:`Dress${index+1}`}));
  w.failCheck=options.failCheck;w.logSnapshot=options.nativeLogs;w.logError=options.logError;
  Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async(text:string)=>{if(options.clipboardFails)throw new Error('Clipboard denied');w.copiedLog=text;}}});
  w.emit=(event:string,payload:any)=>w.listeners.filter((l:any)=>l.event===event).forEach((l:any)=>w.callbacks[l.handler]({payload}));
  w.__TAURI_INTERNALS__={transformCallback:(fn:any)=>{w.callbacks[++cb]=fn;return cb},invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return{mods:w.mods,settings:{mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true},warnings:[]};
   if(cmd==='pending_links')return[];
   if(cmd==='read_mod_check_log'){if(w.logError)throw w.logError;return w.logSnapshot?{entries:w.logSnapshot,path:'C:/AppData/logs/mod-update-checks.jsonl'}:null;}
   if(cmd==='load_nexus_api_key')return 'NEXUS_TEST_KEY';
   if(cmd==='load_curseforge_api_key')return 'CF_TEST_KEY';
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:event|listen'){w.listeners.push(args);return w.listeners.length;}
   if(cmd==='plugin:dialog|message')return args.buttons.OkCancelCustom[w.cancelRestore?1:0];
   if(cmd==='check_mod_update'){
    if(w.failCheck)throw typeof w.failCheck==='string'?w.failCheck:'CurseForge API returned HTTP 403';
    return{id:args.id,provider:args.id==='cf'?'curseforge':'nexus',mod_id:args.id==='cf'?123:42,current_version:'1.0',latest_version:'2.0',status:args.id==='cf'?'upToDate':w.manual?'manual':'available',file_id:2,token:'checked-update',url:'https://www.nexusmods.com/inzoi/mods/42?tab=files&file_id=2'};
   }
   if(cmd==='prepare_mod_update'||cmd==='parse_nxm')return{request_id:`download-${++sequence}`,provider:'nexus',mod_id:42,file_id:2,url:'https://www.nexusmods.com/inzoi/mods/42?tab=files&file_id=2'};
   if(cmd==='download_nxm')return new Promise((resolve,reject)=>{w.pendingDownload={resolve,reject}});
   if(cmd==='install_mod_update'){
    if(w.failInstall)throw 'The update contains the wrong ProjectName. Original mod was kept.';
    const old=w.mods.find((m:any)=>m.id==='nexus');old.superseded_by='new';
    w.mods=w.mods.filter((m:any)=>m.id!=='new');w.mods.push({...old,id:'new',version:'2.0',path:'C:/Library/Downloads/Hair2',superseded_by:null,previous_version:old.id,enabled:old.enabled});old.enabled=false;
    localStorage.setItem('mod-update.mods',JSON.stringify(w.mods));return 'new';
   }
   if(cmd==='restore_previous_mod'){
    const current=w.mods.find((m:any)=>m.id===args.id),old=w.mods.find((m:any)=>m.id===current.previous_version);
    old.enabled=current.enabled;old.superseded_by=null;current.enabled=false;current.superseded_by=old.id;
    localStorage.setItem('mod-update.mods',JSON.stringify(w.mods));return old.id;
   }
   return null;
  }};
 },{enabled,options});
 await page.goto('/');await expect(page.locator('.mod-row')).toHaveCount(options.count??3);
}
async function check(page:Page){await page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/}).click();await expect(page.locator('.mod-update-badge')).toHaveCount(1);await expect(page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/})).toBeEnabled();}
async function start(page:Page){await check(page);await page.locator('.mod-update-section').getByRole('button',{name:'Update mod',exact:true}).click();await expect.poll(()=>page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='download_nxm'))).toBe(true);}
async function finish(page:Page){await page.evaluate(()=>(window as any).pendingDownload.resolve({path:'C:/Library/Downloads/Hair2/update.zip',bytes:4096,can_import:true}));}

test('checks both providers with their own keys and filters available updates without checking local mods',async({page})=>{
 await desktop(page);await check(page);
 const calls=await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='check_mod_update').map((c:any)=>({id:c.args.id,apiKey:c.args.apiKey})));
 expect(calls).toEqual([{id:'nexus',apiKey:'NEXUS_TEST_KEY'},{id:'cf',apiKey:'CF_TEST_KEY'}]);
 await expect(page.locator('.mod-update-section')).toContainText('Current version: 1.0');
 await expect(page.locator('.mod-update-section')).toContainText('New version: 2.0');
 await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('updates');
 await expect(page.locator('.mod-row')).toHaveCount(1);await expect(page.locator('.mod-row')).toContainText('Nexus Hair');
 await page.setViewportSize({width:900,height:600});await page.screenshot({path:'test-results/mod-updates-900.png',fullPage:true});
 expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
});

for(const enabled of [true,false])test(`update keeps the previous version and preserves enabled=${enabled} through restart and restore`,async({page})=>{
 await desktop(page,enabled);await start(page);
 await expect(page.locator('aside .mod-update-count')).toHaveText('1');
 await expect(page.locator('.mod-row').filter({hasText:'Nexus Hair'}).locator('.status')).toHaveText(enabled?'Enabled':'Disabled');
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='install_mod_update'))).toBe(false);
 await finish(page);
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='install_mod_update'))).toBe(true);
 await expect(page.locator('.mod-row')).toHaveCount(3);
 await expect(page.locator('aside .mod-update-count')).toHaveCount(0);
 await expect(page.locator('.mod-row').filter({hasText:'Nexus Hair'}).locator('.status')).toHaveText(enabled?'Enabled':'Disabled');
 expect(await page.evaluate(()=>(window as any).mods.find((m:any)=>m.id==='nexus').superseded_by)).toBe('new');
 await page.reload();await page.locator('.mod-row').filter({hasText:'Nexus Hair'}).click();
 const panel=page.locator('.mod-update-section');await panel.locator('summary').click();
 await page.evaluate(()=>{(window as any).cancelRestore=true});await panel.getByRole('button',{name:'Restore previous version',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='restore_previous_mod'))).toBe(false);
 await page.evaluate(()=>{(window as any).cancelRestore=false});await panel.getByRole('button',{name:'Restore previous version',exact:true}).click();
 await expect(page.locator('.mod-row')).toHaveCount(3);await expect(page.locator('.mod-row').filter({hasText:'Nexus Hair'}).locator('.status')).toHaveText(enabled?'Enabled':'Disabled');
 expect(await page.evaluate(()=>(window as any).mods.find((m:any)=>m.id==='nexus').superseded_by)).toBe(null);
});

test('a failed installation leaves the original enabled and retries the saved archive without downloading again',async({page})=>{
 await desktop(page);await page.evaluate(()=>{(window as any).failInstall=true});await start(page);await finish(page);
 await page.getByRole('button',{name:/^Downloads/}).click();
 const panel=page.getByRole('dialog',{name:'Downloads',exact:true});await expect(panel).toContainText('Original mod was kept');
 expect(await page.evaluate(()=>(window as any).mods.find((m:any)=>m.id==='nexus').enabled)).toBe(true);
 await page.evaluate(()=>{(window as any).failInstall=false});await panel.getByRole('button',{name:'Retry import',exact:true}).click();
 await expect(panel).toContainText('Mod updated');
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
});

test('a signed Nexus link from the website completes the checked update after a direct download fails',async({page})=>{
 await desktop(page);await start(page);await page.evaluate(()=>(window as any).pendingDownload.reject('Nexus API returned HTTP 403'));
 await page.getByRole('button',{name:/^Downloads/}).click();const panel=page.getByRole('dialog',{name:'Downloads',exact:true});await expect(panel).toContainText('HTTP 403');
 await panel.getByRole('textbox',{name:'Link',exact:true}).fill('nxm://inzoi/mods/42/files/2?key=FAKE_SIGNED_KEY&expires=9999999999');
 await panel.getByRole('button',{name:'Add download',exact:true}).click();await page.getByRole('button',{name:'OK',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(2);
 await finish(page);await expect.poll(()=>page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='install_mod_update'))).toBe(true);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='import_download'))).toBe(false);
});

test('unverified variants offer the source page and failed checks stay visible without changing mods',async({page})=>{
 await desktop(page);await page.evaluate(()=>{(window as any).manual=true});await page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/}).click();
 await page.locator('.mod-update-section summary').click();
 await expect(page.locator('.mod-update-section').getByRole('button',{name:'Review source files',exact:true})).toBeVisible();
 await expect(page.getByRole('button',{name:'Update mod',exact:true})).toHaveCount(0);
 await page.evaluate(()=>{(window as any).failCheck=true});await page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/}).click();
 await expect(page.locator('.mod-update-section').getByRole('alert')).toContainText('HTTP 403');
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>['install_mod_update','download_nxm'].includes(c.cmd)))).toBe(false);
});


test('all 21 failed checks have searchable persistent logs and redact keys and signed links when copied',async({page})=>{
 await desktop(page,true,{count:21,failCheck:'CurseForge API returned HTTP 403 | CF_TEST_KEY NEXUS_TEST_KEY https://api.curseforge.com/v1/mods?key=SIGNED_SECRET#TOKEN'});
 await page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/}).click();
 await expect(page.locator('.check-log-notice')).toContainText('21 mod update checks failed.');
 await page.locator('.check-log-notice').getByRole('button',{name:'View log',exact:true}).click();
 const panel=page.getByRole('dialog',{name:'Mod update log',exact:true});
 await expect(panel.getByRole('listitem')).toHaveCount(21);await expect(panel.locator('.check-log-causes')).toContainText('CurseForge · 21');
 await expect(panel.locator('.check-log-causes')).toContainText('HTTP 403: access denied');
 await panel.getByRole('button',{name:'Copy log',exact:true}).click();await expect(panel.getByRole('button',{name:'Copied',exact:true})).toBeVisible();
 const copied=await page.evaluate(()=>(window as any).copiedLog);expect(copied).toContain('http_403');expect(copied).toContain('CF Dress 21');
 for(const secret of ['CF_TEST_KEY','NEXUS_TEST_KEY','SIGNED_SECRET','#TOKEN'])expect(copied).not.toContain(secret);
 const saved=await page.evaluate(()=>localStorage.getItem('inzoimm.modCheckLog'));for(const secret of ['CF_TEST_KEY','NEXUS_TEST_KEY','SIGNED_SECRET','#TOKEN'])expect(saved).not.toContain(secret);
 await panel.getByRole('searchbox',{name:'Search logs',exact:true}).fill('CF Dress 21');await expect(panel.getByRole('listitem')).toHaveCount(1);
 await page.setViewportSize({width:900,height:600});await page.screenshot({path:'test-results/check-log-900.png'});
 const bounds=await panel.boundingBox();expect(bounds!.x).toBeGreaterThanOrEqual(0);expect(bounds!.y+bounds!.height).toBeLessThanOrEqual(600);expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
 await panel.getByRole('button',{name:'View mod',exact:true}).click();await expect(panel).toHaveCount(0);await expect(page.locator('.inspector h2')).toHaveText('CF Dress 21');
 await page.reload();await page.getByRole('button',{name:'View log',exact:true}).click();await expect(panel.getByRole('listitem')).toHaveCount(21);
 await page.keyboard.press('Escape');await expect(panel).toHaveCount(0);
});

test('native history merges by check ID and offers a selectable copy fallback',async({page})=>{
 const entry={id:'saved',run_id:'previous',timestamp:Date.now()-1000,mod_id:'nexus',mod_name:'Nexus Hair',project:'Hair',provider:'nexus',status:'failed',code:'http_429',duration_ms:200,detail:'Nexus API returned HTTP 429 | GET /v1/games/inzoi/mods/42/files.json'};
 await desktop(page,true,{nativeLogs:[entry],clipboardFails:true});
 await page.getByRole('button',{name:'View log',exact:true}).click();const panel=page.getByRole('dialog',{name:'Mod update log',exact:true});
 await expect(panel.getByRole('listitem')).toHaveCount(1);await expect(panel).toContainText('too many requests');await expect(panel).toContainText('C:/AppData/logs/mod-update-checks.jsonl');
 await panel.getByRole('button',{name:'Copy log',exact:true}).click();await expect(panel.getByRole('textbox',{name:'Copy log',exact:true})).toHaveValue(/http_429/);
 await page.keyboard.press('Escape');await check(page);
 const id=await page.evaluate(()=>JSON.parse(localStorage.getItem('inzoimm.modCheckLog')!)[0].id);
 await page.evaluate(({entry,id})=>{(window as any).logSnapshot=[entry,{...entry,id,run_id:'previous',detail:'native replacement'}]}, {entry,id});
 await page.getByRole('button',{name:'View log',exact:true}).click();await panel.getByRole('checkbox',{name:'Errors only',exact:true}).uncheck();await panel.getByRole('checkbox',{name:'Latest run only',exact:true}).uncheck();
 await expect(panel.getByRole('listitem')).toHaveCount(3);await expect(panel).toContainText('native replacement');
});

for(const [failure,explanation] of [['curseforgeApiKeyRequired','Missing API key'],['Command check_mod_update not found','desktop backend does not match']])test(`classifies ${failure} and keeps frontend errors when backend log cannot load`,async({page})=>{
 await desktop(page,true,{failCheck:failure,logError:'Could not read log file'});await page.getByRole('button',{name:/^Check mod updates(?: \d+)?$/}).click();
 await expect(page.locator('.check-log-notice')).toContainText('2 mod update checks failed.');await page.locator('.check-log-notice').getByRole('button',{name:'View log',exact:true}).click();
 const panel=page.getByRole('dialog',{name:'Mod update log',exact:true});await expect(panel.getByRole('listitem')).toHaveCount(2);await expect(panel.getByRole('alert')).toContainText('Could not read log file');await expect(panel.locator('.check-log-causes')).toContainText(explanation);
 await panel.getByRole('button',{name:'Settings',exact:true}).click();await expect(panel).toHaveCount(0);await expect(page.getByRole('dialog',{name:'Settings',exact:true})).toBeVisible();
});
