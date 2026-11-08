import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page){
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];w.cancelledScans=[];let callback=0;
  let settings=JSON.parse(localStorage.getItem('discovery.settings')||'null')??{mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true,source_dirs:[]};
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return{settings,mods:settings.source_dirs.map((source:any,index:number)=>({id:String(index),name:source.path.includes('Vortex')?'Vortex Hair':'Local Outfit',project:`Project${index}`,author:'Creator',version:'1',description:'',path:`${source.path}/Mod`,thumbnail:null,managed:false,enabled:false,source:'',metadata:{}})),warnings:[]};
   if(cmd==='pending_links')return[];
   if(cmd==='list_scan_drives')return['C:\\','D:\\','E:\\'];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return w.cancelPicker?null:['F:/Custom Mods'];
   if(cmd==='cancel_discovery'){w.cancelledScans.push(args.requestId);w.finishScan?.();return null}
   if(cmd==='discover_mods'){
    if(w.scanFails)throw new Error('Cannot read Vortex data. Close Vortex and scan again');
    args.onProgress?.onmessage({current_path:'D:/Vortex/custom staging',entries:1250,mods:4,archives:0});
    if(w.holdScan)await new Promise(resolve=>{w.finishScan=resolve});
    if(w.slowScan)await new Promise(resolve=>setTimeout(resolve,500));
    const folder=(path:string,origin:string,mods:string[])=>({path,origin,mods,already_added:settings.source_dirs.some((s:any)=>s.path===path&&s.scan_subfolders)});
    return{folders:args.paths?[folder(args.paths[0],'local',['Custom mod'])]:[folder('D:/Vortex/inzoi/mods','vortex',['Vortex Hair','Disabled Hair']),folder('E:/My Mods','local',['Local Outfit']),{path:'C:/Game/Mods',origin:'game',mods:['Installed mod'],already_added:true}],warnings:['Unreadable folder: E:/Restricted'],archives:w.archives??[],cancelled:w.cancelledScans.includes(args.requestId)};
   }
   if(cmd==='save_settings'){settings=args.settings;localStorage.setItem('discovery.settings',JSON.stringify(settings))}
   return null;
  }};
 });
 await page.goto('/');await expect(page.getByRole('button',{name:/^(Scan computer|Quét mod trên máy)$/})).toBeEnabled();
}
const dialog=(page:Page)=>page.getByRole('dialog',{name:'Scan computer',exact:true});

test('scan previews Vortex and local mods without writing, adds only selected sources and persists',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 const panel=dialog(page);await expect(panel.getByText('Vortex / Nexus Mods · 2 mods')).toBeVisible();
 await expect(panel.getByText('Disabled Hair',{exact:false})).toBeVisible();
 await expect(panel.getByRole('checkbox')).toHaveCount(3);
 await expect(panel.getByRole('checkbox').nth(2)).toBeDisabled();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='save_settings'))).toBe(false);
 await panel.getByRole('checkbox').nth(1).uncheck();
 await panel.getByRole('button',{name:'Add selected sources (1)',exact:true}).click();
 await expect(panel).not.toBeVisible();await expect(page.locator('.mod-title strong').filter({hasText:'Vortex Hair'})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='save_settings').args.settings.source_dirs)).toEqual([{path:'D:/Vortex/inzoi/mods',scan_subfolders:true}]);
 await page.reload();await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 await expect(dialog(page).getByRole('checkbox').first()).toBeDisabled();
 await page.screenshot({path:'test-results/mod-discovery.png',fullPage:true});
});

test('manual scanning, cancelled folder picker, scan warnings and empty selection',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 const panel=dialog(page);await panel.getByRole('button',{name:'Scan a folder',exact:true}).click();
 await expect(panel.getByText('F:/Custom Mods',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='discover_mods').at(-1).args)).toMatchObject({paths:['F:/Custom Mods'],allDrives:false});
 await page.evaluate(()=>(window as any).cancelPicker=true);await panel.getByRole('button',{name:'Scan a folder',exact:true}).click();
 await expect(panel.getByText('F:/Custom Mods',{exact:true})).toBeVisible();
 await panel.getByRole('checkbox').uncheck();await expect(panel.getByRole('button',{name:'Add selected sources (0)'})).toBeDisabled();
 await panel.locator('summary').click();await expect(panel.getByText('Unreadable folder: E:/Restricted')).toBeVisible();
 await page.keyboard.press('Escape');await expect(panel).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>['import_folder','save_settings','set_mod_enabled'].includes(c.cmd)))).toBe(false);
});

test('failed scan reports error and automatic retry recovers',async({page})=>{
 await desktop(page);await page.evaluate(()=>(window as any).scanFails=true);
 await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 await expect(dialog(page).getByRole('alert')).toContainText('Cannot read Vortex data');
 await page.evaluate(()=>(window as any).scanFails=false);await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 await expect(dialog(page).getByText('D:/Vortex/inzoi/mods',{exact:true})).toBeVisible();await expect(dialog(page).getByRole('alert')).toHaveCount(0);
});

test('scanning disables repeat operations and cancel does not add sources',async({page})=>{
 await desktop(page);await page.evaluate(()=>(window as any).slowScan=true);
 await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 await expect(dialog(page).getByRole('status')).toHaveText('Scanning for mods…');
 await expect(dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true})).toBeDisabled();
 await dialog(page).getByRole('button',{name:'Cancel',exact:true}).click();
 await expect(dialog(page)).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='save_settings'))).toBe(false);
});

test('discovery dialog fits the minimum desktop window and traps keyboard focus',async({page})=>{
 await page.setViewportSize({width:900,height:600});await desktop(page);
 await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 const panel=dialog(page);await expect(panel.getByText('D:/Vortex/inzoi/mods',{exact:true})).toBeVisible();
 await expect(panel.getByRole('button',{name:'Add selected sources (2)'})).toBeInViewport();
 expect(await panel.evaluate(node=>node.scrollWidth-node.clientWidth)).toBe(0);
 await panel.getByRole('button',{name:'Add selected sources (2)'}).focus();await page.keyboard.press('Tab');
 await expect(panel.getByRole('button',{name:'Close',exact:true})).toBeFocused();
 await page.screenshot({path:'test-results/mod-discovery-900.png',fullPage:true});
});

test('Vietnamese scan interface and recursive upgrade of an existing source',async({page})=>{
 await page.addInitScript(()=>{localStorage.setItem('inzoimm.language','vi');localStorage.setItem('discovery.settings',JSON.stringify({mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true,source_dirs:[{path:'D:/Vortex/inzoi/mods',scan_subfolders:false}]}))});
 await desktop(page);await page.getByRole('button',{name:'Quét mod trên máy',exact:true}).click();
 const panel=page.getByRole('dialog',{name:'Quét mod trên máy',exact:true});
 await panel.getByRole('button',{name:'Quét các ổ đã chọn (3)',exact:true}).click();
 await expect(panel.getByRole('checkbox').first()).toBeEnabled();
 await panel.getByRole('checkbox').nth(1).uncheck();await panel.getByRole('button',{name:'Thêm nguồn đã chọn (1)',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='save_settings').args.settings.source_dirs)).toEqual([{path:'D:/Vortex/inzoi/mods',scan_subfolders:true}]);
});


test('full-drive scan is the default, streams real counts, can stop and retain partial results',async({page})=>{
 await desktop(page);await page.evaluate(()=>(window as any).holdScan=true);
 await page.getByRole('button',{name:'Scan computer',exact:true}).click();const panel=dialog(page);
 await panel.getByRole('button',{name:'E:',exact:true}).click();await panel.getByRole('button',{name:'Scan selected drives (2)',exact:true}).click();
 await expect(panel.getByText('1250 entries scanned · 4 mods found',{exact:true})).toBeVisible();
 await expect(panel.getByText('D:/Vortex/custom staging',{exact:true})).toBeVisible();
 const start=await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='discover_mods').args);
 expect(start.allDrives).toBe(true);expect(start.paths).toBeNull();expect(start.drives).toEqual(['C:\\','D:\\']);
 await panel.getByRole('button',{name:'Stop scan',exact:true}).click();
 await expect(panel.getByText('Scan stopped.',{exact:false})).toBeVisible();
 await expect(panel.getByRole('button',{name:'Add selected sources (2)',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='cancel_discovery').args.requestId)).toBe(start.requestId);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='save_settings'))).toBe(false);
});

test('quick scan changes scope and closing an active scan requests cancellation',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='discover_mods'))).toBe(false);
 await dialog(page).getByRole('button',{name:'Quick scan',exact:true}).click();
 await expect(dialog(page).getByText('D:/Vortex/inzoi/mods',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='discover_mods').at(-1).args.allDrives)).toBe(false);
 await dialog(page).getByRole('button',{name:'Close',exact:true}).click();
 await page.evaluate(()=>(window as any).holdScan=true);await page.getByRole('button',{name:'Scan computer',exact:true}).click();
 await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();
 await expect(dialog(page).getByRole('button',{name:'Stop scan',exact:true})).toBeEnabled();
 await page.keyboard.press('Escape');await expect(dialog(page)).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Scan computer',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='cancel_discovery').length)).toBe(1);
});

test('Nexus archives stay separate from source selection, open their folder and import only on request',async({page})=>{
 await desktop(page);await page.evaluate(()=>(window as any).archives=[{path:'D:/Vortex/downloads/inzoi/Hair.zip',name:'Hair.zip'},{path:'D:/Vortex/downloads/inzoi/Outfit.7z',name:'Outfit.7z'}]);
 await page.getByRole('button',{name:'Scan computer',exact:true}).click();await dialog(page).getByRole('button',{name:'Scan selected drives (3)',exact:true}).click();const section=dialog(page).getByRole('region',{name:'Nexus download archives',exact:true});
 await expect(section.getByText('Outfit.7z',{exact:true})).toBeVisible();
 await expect(section.getByRole('button',{name:'Import ZIP',exact:true})).toHaveCount(1);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='import_zip'))).toBe(false);
 await section.getByRole('button',{name:'Open folder',exact:true}).first().click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='open_folder').args.path)).toBe('D:/Vortex/downloads/inzoi');
 await section.getByRole('button',{name:'Import ZIP',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='import_zip').args.path)).toBe('D:/Vortex/downloads/inzoi/Hair.zip');
 await expect(dialog(page)).not.toBeVisible();
});
