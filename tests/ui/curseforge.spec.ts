import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page,curseforgeKey:string|null='CF_TEST_KEY'){
 const saved:{nexus:string|null;curseforge:string|null}={nexus:'NEXUS_TEST_KEY',curseforge:curseforgeKey};
 await page.exposeFunction('providerCredential',({provider,save,key}:{provider:'nexus'|'curseforge';save?:boolean;key?:string})=>{
  if(save)saved[provider]=key?.trim()||null;
  return saved[provider];
 });
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0,request=0;
  let mods=JSON.parse(localStorage.getItem('cf.mods')||'null')??[
   {id:'nexus',name:'Nexus Outfit',source:'https://www.nexusmods.com/inzoi/mods/123',metadata:{'Game library':{gameId:88849}}},
   {id:'cf',name:'CurseForge Hair',metadata:{'curseforge.json':{Download:{provider:'curseforge',mod_id:123,file_id:987,url:'https://www.curseforge.com/inzoi/mods/silk-hair/files/987'}}}},
   {id:'game',name:'Installed CF Dress',metadata:{'Game library':{gameId:88849,id:321}}},
   {id:'other',name:'GitHub Mod',source:'https://github.com/example/mod'},
   {id:'local',name:'Local Mod'}
  ].map(m=>({project:m.id,path:`C:/Library/${m.id}`,author:'Creator',version:'1',description:'',thumbnail:null,managed:true,enabled:false,source:'',...m}));
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return{settings:{mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true,source_dirs:[]},mods,warnings:[]};
   if(cmd==='pending_links')return[];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='load_curseforge_api_key'||cmd==='load_nexus_api_key')return w.providerCredential({provider:cmd.includes('curseforge')?'curseforge':'nexus'});
   if(cmd==='save_curseforge_api_key'||cmd==='save_nexus_api_key')return w.providerCredential({provider:cmd.includes('curseforge')?'curseforge':'nexus',save:true,key:args.apiKey});
   if(cmd==='parse_nxm'){
    const url=new URL(args.link);const parts=url.pathname.split('/');
    return{provider:'curseforge',request_id:`cf-${++request}`,mod_id:0,slug:parts[3],file_id:Number(parts[5]||0),url:args.link};
   }
   if(cmd==='download_nxm'){
    if(w.failDownload)throw 'CurseForge API returned HTTP 403';
    return{path:'C:/Library/Downloads/Download Hair [curseforge-123]/files/file-987/hair.zip',bytes:4096,can_import:true};
   }
   if(cmd==='import_download'){
    mods.push({id:'downloaded',name:'Downloaded CF Hair',project:'DownloadedHair',path:args.path.replace('hair.zip','Hair'),author:'Hair Creator',version:'1',description:'',thumbnail:null,managed:true,enabled:false,source:'',metadata:{'curseforge.json':{Download:{provider:'curseforge',mod_id:123,file_id:987,url:'https://www.curseforge.com/inzoi/mods/download-hair/files/987'}}}});
    localStorage.setItem('cf.mods',JSON.stringify(mods));
   }
   return null;
  }};
 });
 await page.goto('/');await expect(page.locator('.mod-row')).toHaveCount(5);
 return saved;
}
async function paste(page:Page,link='https://www.curseforge.com/inzoi/mods/download-hair/files/987'){
 await page.getByRole('button',{name:'Downloads',exact:true}).click();
 await page.getByRole('textbox',{name:'Link',exact:true}).fill(link);
 await page.getByRole('button',{name:'Add download',exact:true}).click();
 await expect(page.getByRole('dialog',{name:'Download from CurseForge',exact:true})).toBeVisible();
}

test('cards name the actual provider and preserve explicit Nexus provenance over game metadata',async({page})=>{
 await desktop(page);
 for(const [name,source] of [['Nexus Outfit','Nexus Mods'],['CurseForge Hair','CurseForge'],['Installed CF Dress','CurseForge'],['GitHub Mod','github.com'],['Local Mod','Local files']]){
  await expect(page.locator('.mod-row').filter({hasText:name}).locator('.mod-origin')).toHaveText(source);
 }
 await page.reload();await expect(page.locator('.mod-row').filter({hasText:'CurseForge Hair'}).locator('.mod-origin')).toHaveText('CurseForge');
});

test('filter and view controls do not overlap at wide or narrow sizes or after inspector resizing',async({page})=>{
 await desktop(page);
 for(const width of [900,1180,1920,2560]){
  await page.setViewportSize({width,height:780});
  const split=page.getByRole('separator',{name:'Resize details panel'});
  for(const key of ['Home','End']){
   await split.focus();await page.keyboard.press(key);
   const status=(await page.getByRole('combobox',{name:'Status',exact:true}).boundingBox())!;
   const source=(await page.getByRole('combobox',{name:'Browse a source',exact:true}).boundingBox())!;
   const view=(await page.getByRole('group',{name:'View layout',exact:true}).boundingBox())!;
   expect(source.x+source.width).toBeLessThanOrEqual(status.x-5);
   expect(status.x+status.width).toBeLessThanOrEqual(view.x-5);
   expect(status.width).toBeLessThanOrEqual(141);
   expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(width);
  }
 }
 await page.screenshot({path:'test-results/providers-toolbar-wide.png',fullPage:true});
});

test('CurseForge downloads use their own saved key, import disabled, and retain the provider after restart',async({page})=>{
 await desktop(page);await paste(page);
 const dialog=page.getByRole('dialog',{name:'Download from CurseForge',exact:true});
 await expect(dialog.getByLabel('CurseForge API key',{exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:'OK',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='import_download')?.args.path)).toContain('[curseforge-123]');
 const download=await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='download_nxm').args);
 expect(download.apiKey).toBe('CF_TEST_KEY');
 expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toMatch(/CF_TEST_KEY|NEXUS_TEST_KEY/);
 const card=page.locator('.mod-row').filter({hasText:'Downloaded CF Hair'});
 await expect(card.locator('.mod-origin')).toHaveText('CurseForge');await expect(card.locator('.status')).toHaveText('Disabled');
 await page.reload();await expect(card.locator('.mod-origin')).toHaveText('CurseForge');
});

test('a missing CurseForge key is requested at startup and can be cleared independently of Nexus',async({page})=>{
 const saved=await desktop(page,null);
 const onboarding=page.getByRole('dialog',{name:'Connect your accounts',exact:true});
 await expect(onboarding.getByRole('button',{name:'Skip for now',exact:true})).toBeEnabled();
 await onboarding.getByRole('button',{name:'Open CurseForge developer console',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url').args.url)).toBe('https://console.curseforge.com/#/api-keys');
 await onboarding.getByLabel('CurseForge API key',{exact:true}).fill(' CF_NEW_TEST_KEY ');
 await expect(onboarding.getByRole('button',{name:'Continue',exact:true})).toBeEnabled();
 await onboarding.getByRole('button',{name:'Continue',exact:true}).click();
 await expect.poll(()=>saved.curseforge).toBe('CF_NEW_TEST_KEY');
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('CurseForge API key',{exact:true})).toHaveValue('CF_NEW_TEST_KEY');
 await expect(page.getByLabel('Nexus API key',{exact:true})).toHaveValue('NEXUS_TEST_KEY');
 await page.getByRole('button',{name:'Clear CurseForge API key',exact:true}).click();
 await expect.poll(()=>saved.curseforge).toBe(null);expect(saved.nexus).toBe('NEXUS_TEST_KEY');
});

test('CurseForge API failures stay visible with a link to the original page',async({page})=>{
 await desktop(page);await page.evaluate(()=>{(window as any).failDownload=true});await paste(page);
 await page.getByRole('button',{name:'OK',exact:true}).click();
 await page.getByRole('button',{name:/^Downloads/}).click();
 const panel=page.getByRole('dialog',{name:'Downloads',exact:true});
 await expect(panel).toContainText('CurseForge API returned HTTP 403');
 await panel.getByRole('button',{name:'Open on CurseForge',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url').args.url)).toContain('curseforge.com/inzoi/mods/download-hair');
 await expect(panel.getByRole('button',{name:'Retry download',exact:true})).toBeEnabled();
});


test('CurseForge confirmation uses the same handoff and does not animate on cancel',async({page})=>{
 await desktop(page);await paste(page);const dialog=page.getByRole('dialog',{name:'Download from CurseForge',exact:true});
 await dialog.getByRole('button',{name:'Cancel',exact:true}).click();await expect(page.locator('.download-handoff-flight')).toHaveCount(0);
 await paste(page);await dialog.getByRole('button',{name:'OK',exact:true}).click();const flight=page.locator('.download-handoff-flight');await expect(flight).toBeVisible();
 await flight.evaluate(node=>node.getAnimations()[0].finish());await expect(flight).toHaveCount(0);await expect(page.getByRole('status').filter({hasText:'Added to downloads'})).toBeVisible();
 await page.getByRole('button',{name:/^Downloads/}).click();await expect(page.getByRole('dialog',{name:'Downloads',exact:true})).toContainText('Added to library');
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
});
