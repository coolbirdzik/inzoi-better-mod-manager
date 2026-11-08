import {test,expect,type Page} from '@playwright/test';

async function library(page:Page,libraryDir='C:/Library'){
 await page.addInitScript(libraryDir=>{
  const w=window as any;w.isTauri=true;w.calls=[];let callback=0;
  let settings=JSON.parse(localStorage.getItem('sources.settings')||'null')??{mods_dir:'C:/Game/Mods',library_dir:libraryDir,scan_subfolders:true,source_dirs:['D:/Collection']};
  const mods=JSON.parse(localStorage.getItem('sources.mods')||'null')??[
   {id:'game',name:'Installed Hair',project:'GameHair',path:'C:/Game/Mods/88849/Hair',managed:false,enabled:true,tags:[]},
   {id:'dress',name:'Linen Dress',project:'LinenDress',path:'D:/Collection/Clothing/Dresses/Linen',managed:false,enabled:false,tags:['Clothing'],metadata:{'mod_manifest.json':{NexusUrl:'https://www.nexusmods.com/inzoi/mods/42',Custom:{Compatibility:'Patch 1.2',Rating:5},bEnable:false},'meta.ini':{'General.modID':'42'},'Game library':{links:{websiteUrl:'https://www.curseforge.com/inzoi/mods/linen'}}}},
   {id:'hair',name:'Short Hair',project:'Hair',path:'D:/Collection/Hair/Short',managed:false,enabled:false,tags:[]}
  ].map(m=>({author:'Creator',version:'1.0',description:'Description',source:'',thumbnail:null,...m}));
  w.__TAURI_INTERNALS__={transformCallback:()=>++callback,invoke:async(cmd:string,args:any)=>{
   if(cmd==='load_nexus_api_key'||cmd==='load_curseforge_api_key')return 'TEST_KEY';
   w.calls.push({cmd,args});
   if(cmd==='snapshot')return {mods:[...mods.filter((m:any)=>!m.path.startsWith('D:')||settings.source_dirs.some((source:any)=>{const path=typeof source==='string'?source:source.path;return path==='D:/Collection'&&(typeof source==='string'||source.scan_subfolders)})),...(w.extraMods??[])],settings,warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return ['E:/MoreMods','F:/Favorites','D:/Collection/'];
   if(cmd==='save_settings'){settings=args.settings;localStorage.setItem('sources.settings',JSON.stringify(settings));}
   if(cmd==='set_mod_tags'){
    if(w.failTags)throw new Error('Cannot save tags');
    mods.find((m:any)=>m.id===args.id).tags=[...new Set(args.tags.map((s:string)=>s.trim()).filter(Boolean))];
    localStorage.setItem('sources.mods',JSON.stringify(mods));
   }
   return null;
  }};
 },libraryDir);
 await page.goto('/');await expect(page.locator('.mod-row')).toHaveCount(3);
}

test('opening Downloads from its SVG icon passes the exact folder path to the native folder command',async({page})=>{
 await library(page,'F:\\InZoi mod');
 await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await page.getByRole('tab',{name:/^Source folders/}).click();
 const source=page.locator('.managed-source-row').filter({hasText:'Downloads'});
 await source.getByRole('button',{name:'Open folder',exact:true}).locator('svg').click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='open_folder')?.args)).toEqual({path:'F:/InZoi mod/Downloads'});
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='plugin:opener|open_path'))).toBe(false);
});

test('Managed mods collects sources, remembers each scan choice, and removing a source only detaches it',async({page})=>{
 await library(page);
 await expect(page.getByRole('button',{name:'Add source folders',exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await expect(page.locator('.mod-row')).toHaveCount(3);
 await page.getByRole('tab',{name:/^Source folders/}).click();
 const sources=page.getByRole('region',{name:'Source folders',exact:true});
 await expect(sources.getByRole('button',{name:/Downloads/})).toBeVisible();
 await page.getByRole('button',{name:'Add source folders',exact:true}).click();
 const dialog=page.getByRole('dialog',{name:'Add source folders',exact:true});
 await expect(dialog.getByRole('checkbox',{name:'Scan subfolders'})).toBeChecked();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='save_settings'))).toBe(false);
 await dialog.getByRole('checkbox',{name:'Scan subfolders'}).uncheck();
 await dialog.getByRole('button',{name:'Choose folder',exact:true}).click();
 await dialog.getByRole('button',{name:'Add source folders',exact:true}).click();
 await expect(sources.locator('.source-folder').filter({hasText:'MoreMods'})).toBeVisible();
 await expect(sources.locator('.source-folder').filter({hasText:'Favorites'})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='save_settings').args.settings.source_dirs)).toEqual([{path:'D:/Collection',scan_subfolders:true},{path:'E:/MoreMods',scan_subfolders:false},{path:'F:/Favorites',scan_subfolders:false}]);
 await page.reload();await page.getByRole('button',{name:/^All mods \d+$/}).click();await page.getByRole('tab',{name:/^Source folders/}).click();
 const more=sources.locator('.managed-source-row').filter({hasText:'MoreMods'});
 await expect(more.getByRole('checkbox',{name:'Scan subfolders'})).not.toBeChecked();
 await more.getByRole('checkbox',{name:'Scan subfolders'}).check();
 const collection=sources.locator('.managed-source-row').filter({hasText:'Collection'});
 await collection.getByRole('checkbox',{name:'Scan subfolders'}).uncheck();
 await page.getByRole('tab',{name:'Mods',exact:true}).click();await expect(page.locator('.mod-row')).toHaveCount(1);
 await page.reload();await page.getByRole('button',{name:/^All mods \d+$/}).click();await page.getByRole('tab',{name:/^Source folders/}).click();
 await expect(collection.getByRole('checkbox',{name:'Scan subfolders'})).not.toBeChecked();
 await expect(more.getByRole('checkbox',{name:'Scan subfolders'})).toBeChecked();
 await sources.getByRole('button',{name:'Remove source Collection',exact:true}).click();
 await expect(sources.getByRole('button',{name:'Remove source Collection',exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:/^All mods \d+$/}).click();await expect(page.locator('.mod-row')).toHaveCount(1);
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>['remove_mod','import_folder','adopt_mod'].includes(c.cmd)))).toBe(false);
});

test('folder browsing in Managed mods preserves nested groups and searches descendants',async({page})=>{
 await library(page);await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await page.getByRole('tab',{name:/^Source folders/}).click();
 await page.locator('.source-folder').filter({hasText:'Collection'}).click();
 await expect(page.locator('.folder-cards button')).toHaveCount(2);
 await expect(page.locator('.mod-row')).toHaveCount(0);
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('Linen');
 await expect(page.locator('.mod-row')).toHaveCount(1);
 await page.getByRole('textbox',{name:'Search',exact:true}).clear();
 await page.locator('.folder-cards button').filter({hasText:'Clothing'}).click();
 await page.locator('.folder-cards button').filter({hasText:'Dresses'}).click();
 await expect(page.locator('.mod-row')).toHaveCount(1);
 await expect(page.locator('.mod-row')).toContainText('Linen Dress');
 await expect(page.getByRole('button',{name:'Take control',exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:'Parent folder',exact:true}).click();
 await expect(page.locator('.folder-cards button')).toContainText('Dresses');
 await page.getByRole('button',{name:/^All mods \d+$/}).click();await page.getByRole('combobox',{name:'Status',exact:true}).selectOption('enabled');
 await expect(page.locator('.mod-row')).toContainText('Installed Hair');
});

test('Downloads and attached local mods appear together in Managed mods',async({page})=>{
 await library(page);await page.evaluate(()=>{(window as any).extraMods=[{id:'nexus',name:'Nexus Outfit',project:'NexusOutfit',path:'C:/Library/Downloads/1/Outfit',managed:true,enabled:false,author:'Creator',version:'1',source:'',description:'',thumbnail:null,library_root:'C:/Library/Downloads'},{id:'legacy',name:'Existing Outfit',project:'ExistingOutfit',path:'C:/Library/Nexus/Existing',managed:true,enabled:false,author:'Creator',version:'1',source:'',description:'',thumbnail:null,library_root:'C:/Library/Nexus'}]});
 await page.getByRole('button',{name:'Refresh library',exact:true}).click();
 await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await expect(page.locator('.mod-row')).toHaveCount(5);
 await expect(page.locator('.mod-row').filter({hasText:'Existing Outfit'})).toBeVisible();
 await expect(page.locator('.mod-row').filter({hasText:'Nexus Outfit'})).toBeVisible();
 await expect(page.locator('.mod-row').filter({hasText:'Linen Dress'})).toBeVisible();
 await page.getByRole('tab',{name:/^Source folders/}).click();
 await page.screenshot({path:'test-results/managed-source-folders.png',fullPage:true});
 await expect(page.getByText('Nexus staging',{exact:true})).toHaveCount(0);
 await expect(page.locator('.source-folder').filter({hasText:'C:/Library/Nexus'})).toHaveCount(0);
 await page.locator('.source-folder').filter({hasText:'Downloads'}).click();
 await page.locator('.folder-cards button').filter({hasText:'1'}).click();
 await expect(page.locator('.mod-row')).toHaveCount(1);
 await expect(page.locator('.mod-row')).toContainText('Nexus Outfit');
});

test('inspector exposes nested metadata and links, and tags persist and filter the library',async({page})=>{
 await library(page);await page.locator('.mod-row').filter({hasText:'Linen Dress'}).click();
 const inspector=page.getByRole('region',{name:'Metadata'});
 await inspector.locator('summary').filter({hasText:'Metadata'}).click();
 await expect(inspector.getByText('mod_manifest.json.Custom.Compatibility',{exact:true})).toBeVisible();
 await expect(inspector.getByText('Patch 1.2',{exact:true})).toBeVisible();
 await expect(inspector.getByText('false',{exact:true})).toBeVisible();
 const nexus=inspector.getByRole('button',{name:'https://www.nexusmods.com/inzoi/mods/42',exact:true});
 await inspector.locator('summary').filter({hasText:'Links'}).click();
 await expect(nexus).toHaveCount(1);await nexus.click();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='plugin:opener|open_url').args.url)).toBe('https://www.nexusmods.com/inzoi/mods/42');
 await expect(inspector.getByRole('button',{name:'https://www.curseforge.com/inzoi/mods/linen',exact:true})).toBeVisible();
 await inspector.locator('.inspector-tags > summary').click();
 await inspector.getByRole('textbox',{name:'Add tag',exact:true}).fill('Summer favorites');
 await inspector.getByRole('textbox',{name:'Add tag',exact:true}).press('Enter');
 await expect(inspector.getByRole('button',{name:'Remove tag Summer favorites',exact:true})).toBeVisible();
 await page.reload();await page.getByRole('combobox',{name:'Filter by tag'}).selectOption('Summer favorites');
 await expect(page.locator('.mod-row')).toHaveCount(1);await expect(page.locator('.mod-row')).toContainText('Linen Dress');
 await inspector.locator('.inspector-tags > summary').click();
 await page.getByRole('button',{name:'Remove tag Summer favorites',exact:true}).click();
 await expect(page.getByRole('combobox',{name:'Filter by tag'})).toHaveValue('');
 await expect(page.locator('.mod-row')).toHaveCount(3);
 await page.getByRole('textbox',{name:'Search',exact:true}).fill('Clothing');
 await expect(page.locator('.mod-row')).toHaveCount(1);
 await page.screenshot({path:'test-results/library-metadata-tags.png',fullPage:true});
});

test('folders and inspector fit minimum desktop size',async({page})=>{
 await page.setViewportSize({width:900,height:600});await library(page);
 await page.locator('.mod-row').filter({hasText:'Linen Dress'}).click();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
 await expect(page.locator('footer')).toBeInViewport();
 await expect(page.getByRole('button',{name:'Settings'})).toBeVisible();
 await page.screenshot({path:'test-results/library-sources-900.png',fullPage:true});
});

test('failed tag save keeps the input and shows the failure',async({page})=>{
 await library(page);await page.locator('.mod-row').filter({hasText:'Linen Dress'}).click();
 await page.evaluate(()=>{(window as any).failTags=true});
 await page.locator('.inspector-tags > summary').click();
 const input=page.getByRole('textbox',{name:'Add tag',exact:true});
 await input.fill('Keep this tag');await input.press('Enter');
 await expect(page.getByRole('alert')).toContainText('Cannot save tags');
 await expect(input).toHaveValue('Keep this tag');
 await expect(page.getByRole('button',{name:'Remove tag Keep this tag',exact:true})).toHaveCount(0);
});

test('source management has its own tab, scoped search, browse selector and small-window layout',async({page})=>{
 await library(page);
 await page.evaluate(()=>{
  localStorage.setItem('sources.settings',JSON.stringify({mods_dir:'C:/Game/Mods',library_dir:'C:/Library',scan_subfolders:true,source_dirs:[
   {path:'D:/Collection',scan_subfolders:true},
   {path:'E:/Mod collections/Creator collections/Clothing and accessories/Favorites',scan_subfolders:false},
   {path:'F:/Games/inZOI/My curated mods/Home decor',scan_subfolders:true}
  ]}));
 });
 await page.reload();await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await expect(page.getByRole('tab',{name:'Mods',exact:true})).toHaveAttribute('aria-selected','true');
 await expect(page.locator('.managed-source-row')).toHaveCount(0);
 await page.getByRole('combobox',{name:'Browse a source',exact:true}).selectOption('D:/Collection');
 await expect(page.getByRole('heading',{name:'Collection',exact:true})).toBeVisible();
 await page.getByRole('tab',{name:/^Source folders/}).click();
 await expect(page.getByRole('region',{name:'Mod details'})).toHaveCount(0);
 const sources=page.getByRole('region',{name:'Source folders',exact:true});
 await expect(sources.getByRole('region',{name:'Your folders',exact:true})).toBeVisible();
 await expect(sources.getByRole('region',{name:'App folders',exact:true})).toBeVisible();
 await expect(sources.locator('.managed-source-row')).toHaveCount(5);
 const search=page.getByRole('textbox',{name:'Search source folders…',exact:true});
 await search.fill('Creator collections');
 await expect(sources.locator('.managed-source-row')).toHaveCount(1);
 await expect(sources.locator('.source-folder')).toContainText('Favorites');
 await search.fill('missing folder');await expect(page.getByRole('heading',{name:'No matching folders',exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Clear source search',exact:true}).click();
 await page.setViewportSize({width:1280,height:900});
 await page.screenshot({path:'test-results/source-manager.png',fullPage:true});
 await page.setViewportSize({width:900,height:600});
 expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(900);
 expect(await page.locator('.sources-workspace').evaluate(node=>node.scrollWidth-node.clientWidth)).toBe(0);
 await expect(page.getByRole('button',{name:'Downloads',exact:true})).toBeInViewport();
 await page.screenshot({path:'test-results/source-manager-900.png',fullPage:true});
 await page.getByRole('tab',{name:'Mods',exact:true}).click();
 await expect(page.getByRole('textbox',{name:'Search',exact:true})).toHaveValue('');
 await page.getByRole('tab',{name:'Mods',exact:true}).focus();await page.keyboard.press('ArrowRight');
 await expect(page.getByRole('tab',{name:/^Source folders/})).toHaveAttribute('aria-selected','true');
 await page.keyboard.press('Alt+ArrowLeft');await expect(page.getByRole('tab',{name:'Mods',exact:true})).toHaveAttribute('aria-selected','true');
});
