import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page){
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  let settings=JSON.parse(localStorage.getItem('navigation.settings')||'null')??{mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true,source_dirs:[{path:'D:/Collection',scan_subfolders:true}]};
  const mods=[
   {id:'dress',name:'Linen Dress',project:'Linen',path:'D:/Collection/Clothing/Dresses/Linen',managed:false,enabled:false},
   {id:'hair',name:'Short Hair',project:'Hair',path:'D:/Collection/Hair/Short',managed:false,enabled:false},
   {id:'nexus',name:'Nexus Outfit',project:'Outfit',path:'C:/Library/Downloads/1/Outfit',managed:true,enabled:false,library_root:'C:/Library/Downloads'},
   {id:'game',name:'Installed Hair',project:'GameHair',path:'C:/Game/Mods/GameHair',managed:false,enabled:true}
  ].map(m=>({author:'Creator',version:'1',source:'',description:'',thumbnail:null,...m}));
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods,settings,warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='save_settings'){if(w.failSave)throw 'Cannot save settings';settings=args.settings;localStorage.setItem('navigation.settings',JSON.stringify(settings));}
   if(cmd==='plugin:dialog|open')return ['E:/MoreMods'];
   if(cmd==='plugin:dialog|message')return args.buttons.OkCancelCustom[0];
   if(cmd==='set_mod_enabled'){const m=mods.find(m=>m.id===args.id)!;m.enabled=args.enabled;return m.id;}
   if(cmd==='remove_mod'){const index=mods.findIndex(m=>m.id===args.id);mods.splice(index,1);}
   return null;
  }};
 });
 await page.goto('/');await expect(page.locator('.mod-row')).toHaveCount(4);
}
const managed=(page:Page)=>page.getByRole('button',{name:/^All mods \d+$/});
async function mouseButton(page:Page,button:3|4){
 await page.locator('main').dispatchEvent('mousedown',{button,bubbles:true,cancelable:true});
 await page.locator('main').dispatchEvent('mouseup',{button,bubbles:true,cancelable:true});
 await page.locator('main').dispatchEvent('auxclick',{button,bubbles:true,cancelable:true});
}

test('scan subfolders is visible before picking folders, and choices survive save and reload',async({page})=>{
 await desktop(page);await managed(page).click();await page.getByRole('tab',{name:/^Source folders/}).click();
 await expect(page.locator('.managed-source-row').filter({hasText:'Managed mods folder'}).getByRole('checkbox',{name:'Scan subfolders'})).toBeVisible();
 await page.getByRole('button',{name:'Add source folders',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'Add source folders',exact:true});
 const scan=dialog.getByRole('checkbox',{name:'Scan subfolders'});
 await expect(scan).toBeInViewport();
 await page.screenshot({path:'test-results/source-scan-choice.png',fullPage:true});
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:dialog|open'))).toBe(false);
 await scan.uncheck();await dialog.getByRole('button',{name:'Choose folder',exact:true}).click();
 await expect(scan).not.toBeChecked();await dialog.getByRole('button',{name:'Add source folders',exact:true}).click();
 await page.reload();await managed(page).click();await page.getByRole('tab',{name:/^Source folders/}).click();
 const source=page.locator('.managed-source-row').filter({hasText:'MoreMods'});
 await expect(source.getByRole('checkbox',{name:'Scan subfolders'})).not.toBeChecked();
 await source.getByRole('checkbox',{name:'Scan subfolders'}).check();
 await page.evaluate(()=>{(window as any).failSave=true});await source.getByRole('checkbox',{name:'Scan subfolders'}).click();
 await expect(page.getByRole('alert')).toContainText('Cannot save settings');
 await expect(source.getByRole('checkbox',{name:'Scan subfolders'})).toBeChecked();
});

test('mouse back/forward restore folders and selection, while new navigation clears forward history',async({page})=>{
 await desktop(page);await expect(page.getByRole('button',{name:'Back',exact:true})).toBeDisabled();
 await managed(page).click();await page.getByRole('tab',{name:/^Source folders/}).click();await page.locator('.source-folder').filter({hasText:'Collection'}).click();
 await page.locator('.folder-cards button').filter({hasText:'Clothing'}).click();
 await page.locator('.folder-cards button').filter({hasText:'Dresses'}).click();
 await page.locator('.mod-row').filter({hasText:'Linen Dress'}).click();
 await mouseButton(page,3);await expect(page.getByRole('heading',{name:'Clothing',exact:true})).toBeVisible();
 await mouseButton(page,4);await expect(page.getByRole('heading',{name:'Dresses',exact:true})).toBeVisible();
 await expect(page.locator('.mod-row').filter({hasText:'Linen Dress'})).toHaveAttribute('aria-pressed','true');
 await page.keyboard.press('Alt+ArrowLeft');await expect(page.getByRole('heading',{name:'Clothing',exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Back',exact:true}).click();await expect(page.getByRole('heading',{name:'Collection',exact:true})).toBeVisible();
 await page.locator('.folder-cards button').filter({hasText:'Hair'}).click();
 await expect(page.getByRole('button',{name:'Forward',exact:true})).toBeDisabled();
 await page.getByRole('tab',{name:/^Source folders/}).click();await page.getByRole('button',{name:'Add source folders',exact:true}).click();
 await mouseButton(page,3);await expect(page.getByRole('dialog')).toBeVisible();
 await page.keyboard.press('Escape');await expect(page.getByRole('heading',{name:'Source folders',exact:true,level:1})).toBeVisible();
});

test('native browser history and real Chromium mouse side buttons each move only one step',async({page})=>{
 await desktop(page);await managed(page).click();await page.getByRole('tab',{name:/^Source folders/}).click();await page.locator('.source-folder').filter({hasText:'Collection'}).click();
 await page.locator('.folder-cards button').filter({hasText:'Clothing'}).click();
 await page.locator('.folder-cards button').filter({hasText:'Dresses'}).click();
 const cdp=await page.context().newCDPSession(page);
 for(const type of ['mousePressed','mouseReleased'])await cdp.send('Input.dispatchMouseEvent',{type,button:'back',buttons:type==='mousePressed'?8:0,clickCount:1,x:450,y:120});
 await expect(page.getByRole('heading',{name:'Clothing',exact:true})).toBeVisible();
 await page.waitForTimeout(200);await expect(page.getByRole('heading',{name:'Clothing',exact:true})).toBeVisible();
 for(const type of ['mousePressed','mouseReleased'])await cdp.send('Input.dispatchMouseEvent',{type,button:'forward',buttons:type==='mousePressed'?16:0,clickCount:1,x:450,y:120});
 await expect(page.getByRole('heading',{name:'Dresses',exact:true})).toBeVisible();
 await page.goBack();await expect(page.getByRole('heading',{name:'Clothing',exact:true})).toBeVisible();
 await page.goForward();await expect(page.getByRole('heading',{name:'Dresses',exact:true})).toBeVisible();
});

test('right-click source menus change scan settings, open folders and detach without deleting files',async({page})=>{
 await desktop(page);await managed(page).click();await page.getByRole('tab',{name:/^Source folders/}).click();
 const source=page.locator('.managed-source-row').filter({hasText:'Collection'});
 await source.locator('.source-folder').click({button:'right'});let menu=page.getByRole('menu',{name:'Context menu'});
 await expect(menu.getByRole('menuitemcheckbox',{name:'Scan subfolders'})).toHaveAttribute('aria-checked','true');
 await menu.getByRole('menuitemcheckbox',{name:'Scan subfolders'}).click();
 await expect(source.getByRole('checkbox',{name:'Scan subfolders'})).not.toBeChecked();
 await source.locator('.source-folder').click({button:'right'});await menu.getByRole('menuitem',{name:'Open folder',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='open_folder').args.path)).toBe('D:/Collection');
 await source.locator('.source-folder').focus();await page.keyboard.press('Shift+F10');await expect(menu).toBeVisible();
 await page.keyboard.press('ArrowDown');await expect(menu.getByRole('menuitem',{name:'Open folder',exact:true})).toBeFocused();
 await page.keyboard.press('Escape');await expect(source.locator('.source-folder')).toBeFocused();
 await source.locator('.source-folder').click({button:'right'});await menu.getByRole('menuitem',{name:'Remove source Collection',exact:true}).click();
 await expect(source).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='remove_mod'))).toBe(false);
});

test('right-click mod and folder menus match their targets and stay inside the window',async({page})=>{
 await page.setViewportSize({width:900,height:600});await desktop(page);
 const external=page.locator('.mod-row').filter({hasText:'Linen Dress'});
 await external.click({button:'right'});const menu=page.getByRole('menu',{name:'Context menu'});
 await expect(menu.getByRole('menuitem',{name:'Enable in game',exact:true})).toBeVisible();
 await expect(menu.getByRole('menuitem',{name:'Remove',exact:true})).toBeEnabled();
 await menu.getByRole('menuitem',{name:'Edit tags',exact:true}).click();await expect(page.getByRole('textbox',{name:'Add tag',exact:true})).toBeFocused();
 const nexus=page.locator('.mod-row').filter({hasText:'Nexus Outfit'});
 await nexus.click({button:'right'});await menu.getByRole('menuitem',{name:'Enable in game',exact:true}).click();
 await nexus.click({button:'right'});await expect(menu.getByRole('menuitem',{name:'Remove',exact:true})).toBeEnabled();
 await menu.getByRole('menuitem',{name:'Disable mod',exact:true}).click();
 await managed(page).click();await page.getByRole('tab',{name:/^Source folders/}).click();await page.locator('.source-folder').filter({hasText:'Collection'}).click();
 const folder=page.locator('.folder-cards button').filter({hasText:'Clothing'});await folder.click({button:'right'});
 await expect(menu.getByRole('menuitem',{name:'Open',exact:true})).toBeVisible();
 await expect(menu.getByRole('menuitemcheckbox',{name:'Scan subfolders'})).toHaveCount(0);
 await menu.getByRole('menuitem',{name:'Open',exact:true}).click();await expect(page.getByRole('heading',{name:'Clothing',exact:true})).toBeVisible();
 await page.mouse.click(898,580,{button:'right'});
 const bounds=await menu.boundingBox();expect(bounds!.x).toBeGreaterThan(500);expect(bounds!.y).toBeGreaterThan(250);expect(bounds!.x+bounds!.width).toBeLessThanOrEqual(900);expect(bounds!.y+bounds!.height).toBeLessThanOrEqual(600);
 await page.screenshot({path:'test-results/navigation-context-900.png',fullPage:true});
 await page.getByRole('heading',{name:'Clothing',exact:true}).click();await expect(menu).toHaveCount(0);
});
