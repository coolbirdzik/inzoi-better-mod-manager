import {test,expect,type Page} from '@playwright/test';

async function desktop(page:Page){
 await page.addInitScript(()=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  let settings=JSON.parse(localStorage.getItem('test.settings')||'null')??{mods_dir:'C:/Mods',library_dir:'C:/library',scan_subfolders:true};
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods:[],settings,warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return w.cancelPicker?null:'C:/ChosenMods';
   if(cmd==='save_settings'){settings={...args.settings};localStorage.setItem('test.settings',JSON.stringify(settings));}
   return null;
  }};
 });
 await page.goto('/');await expect(page.getByRole('button',{name:'Add mod',exact:true})).toBeEnabled();
}

test('folder picker exposes scan option before importing and supports cancel',async({page})=>{
 await desktop(page);
 await page.getByRole('button',{name:'Add mod',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'Import mods'});
 await expect(dialog.getByText('C:/ChosenMods',{exact:true})).toBeVisible();
 await expect(dialog.getByRole('checkbox',{name:'Scan subfolders'})).toBeChecked();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='import_folder'))).toBe(false);
 await dialog.getByRole('checkbox').uncheck();await dialog.getByRole('button',{name:'Import mods',exact:true}).click();
 await expect(dialog).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='import_folder').args)).toEqual({path:'C:/ChosenMods',scanSubfolders:false});
 await page.getByRole('button',{name:'Add mod',exact:true}).click();
 await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='import_folder').length)).toBe(1);
 await page.evaluate(()=>{(window as any).cancelPicker=true});
 await page.getByRole('button',{name:'Add mod',exact:true}).click();await expect(dialog).not.toBeVisible();
});

test('settings preserve scan choice after folder selection and reload, and default new imports to it',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:'Settings'}).click();
 const dialog=page.getByRole('dialog');const checkbox=dialog.getByRole('checkbox',{name:'Scan subfolders'});
 await expect(checkbox).toBeChecked();await checkbox.uncheck();
 await dialog.getByRole('button',{name:'Choose folder',exact:true}).last().click();
 await expect(checkbox).not.toBeChecked();
 await dialog.getByRole('button',{name:'Save settings',exact:true}).click();
 await expect(dialog).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='save_settings').args.settings)).toEqual({mods_dir:'C:/Mods',library_dir:'C:/ChosenMods',scan_subfolders:false});
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(checkbox).not.toBeChecked();
 await page.keyboard.press('Escape');await page.getByRole('button',{name:'Add mod',exact:true}).click();
 await expect(page.getByRole('dialog').getByRole('checkbox')).not.toBeChecked();
 await page.getByRole('dialog').getByRole('checkbox').check();
 await page.getByRole('dialog').getByRole('button',{name:'Import mods',exact:true}).click();
 await expect(page.getByRole('dialog')).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='import_folder').args.scanSubfolders)).toBe(true);
});

test('add source in Managed mods offers scan choice and leaves game and download folders alone',async({page})=>{
 await desktop(page);await page.getByRole('button',{name:/^Managed mods \d+$/}).click();
 await page.getByRole('button',{name:'Add source folders',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'Add source folders',exact:true});
 await expect(dialog.getByRole('checkbox',{name:'Scan subfolders'})).toBeVisible();
 await expect(dialog.getByRole('button',{name:'Add source folders',exact:true})).toBeDisabled();
 await dialog.getByRole('button',{name:'Choose folder',exact:true}).click();
 await expect(dialog.getByText('C:/ChosenMods',{exact:true})).toBeVisible();
 await dialog.getByRole('checkbox',{name:'Scan subfolders'}).uncheck();
 await dialog.getByRole('button',{name:'Add source folders',exact:true}).click();
 await expect(dialog).not.toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='save_settings').args.settings)).toEqual({mods_dir:'C:/Mods',library_dir:'C:/library',scan_subfolders:true,source_dirs:[{path:'C:/ChosenMods',scan_subfolders:false}]});
 await expect(page.locator('.source-folder').filter({hasText:'Downloads'})).toBeVisible();
 await page.getByRole('button',{name:'Add source folders',exact:true}).click();
 await page.getByRole('dialog').getByRole('button',{name:'Cancel',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='save_settings').length)).toBe(1);
});
