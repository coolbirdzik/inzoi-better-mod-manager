import {test,expect,type Page} from '@playwright/test';
import {readFileSync} from 'node:fs';

async function desktop(page:Page,initialSettings?:{library_dir:string;downloads_dir?:string}){
 await page.addInitScript(initialSettings=>{
  const w=window as any;w.isTauri=true;w.calls=[];w.downloads={};w.callbacks={};w.listeners=[];let callback=0;
  let settings=JSON.parse(localStorage.getItem('downloads.settings')||'null')??{mods_dir:'C:/Mods',library_dir:'C:/Library',scan_subfolders:true,...initialSettings};
  const mods:any[]=[];w.archives=JSON.parse(localStorage.getItem('downloads.archives')||'[]');
  w.emit=(event:string,payload:any)=>{for(const listener of w.listeners.filter((l:any)=>l.event===event))w.callbacks[listener.handler]({event,id:0,payload})};
  w.__TAURI_INTERNALS__={transformCallback:(fn:any)=>{w.callbacks[++callback]=fn;return callback},invoke:async(cmd:string,args:any)=>{
   w.calls.push({cmd,args});
   if(cmd==='plugin:event|listen'){w.listeners.push(args);return w.listeners.length;}
   if(cmd==='snapshot')return {mods:[...mods,...w.archives.filter((a:any)=>!mods.some(m=>m.path.startsWith(a.path.replace(/[^/]+$/,''))))],settings,warnings:[]};
   if(cmd==='pending_links')return [];
   if(cmd==='is_portable')return true;
   if(cmd==='update_portable')return false;
   if(cmd==='plugin:dialog|open')return 'D:/Nexus downloads';
   if(cmd==='save_settings'){settings={...args.settings};localStorage.setItem('downloads.settings',JSON.stringify(settings));}
   if(cmd==='download_nxm')return new Promise((resolve,reject)=>{w.downloads[args.requestId]={resolve,reject}});
   if(cmd==='cancel_nxm'){w.downloads[args.requestId].reject('downloadCancelled');}
   if(cmd==='import_download'){
    if(w.failImports)throw 'Import failed';
    mods.push({id:args.path,name:'Downloaded outfit',project:args.path,author:'Creator',version:'1',path:args.path.replace(/mod\.zip$/,'Imported'),managed:true,enabled:false,source:'',description:'',thumbnail:w.importedThumbnail??null});
   }
   return null;
  }};
 },initialSettings);
 await page.goto('/');await page.getByRole('button',{name:'Settings'}).click();
 await page.getByLabel('Nexus API key',{exact:true}).fill('SESSION_KEY');await page.keyboard.press('Escape');
}
async function send(page:Page,id:number){
 await page.evaluate(id=>(window as any).emit('nexus-link',{request_id:`job-${id}`,mod_id:id,file_id:id*10,url:`https://www.nexusmods.com/inzoi/mods/${id}`}),id);
 await expect(page.getByRole('dialog',{name:'Download from Nexus'})).toBeVisible();
}
async function finish(page:Page,id:number,zip=true,format=zip?'zip':'7z'){
 await page.evaluate(({id,zip,format})=>{
  const w=window as any;
  const path=`D:/Nexus downloads/Outfit [nexus-${id}]/files/file-${id*10}/mod.${format}`;
  w.archives=w.archives.filter((a:any)=>a.path!==path);
  w.archives.push({id:`archive-${id}`,name:`Downloaded package ${id}`,project:`NexusDownload_${id}`,path,archive:true,managed:false,enabled:false,author:'Creator',version:'3.7',description:'Downloaded archive',source:`https://www.nexusmods.com/inzoi/mods/${id}`,thumbnail:w.archiveThumbnail??null,library_root:'D:/Nexus downloads',metadata:{'nexus.json':{Download:{mod_id:id,file_id:id*10}}}});
  localStorage.setItem('downloads.archives',JSON.stringify(w.archives));
  w.downloads[`job-${id}`].resolve({path,bytes:1048576,can_import:zip});
 },{id,zip,format});
}

test('incoming Nexus popup needs only OK; ZIP downloads import into Managed mods in the background',async({page})=>{
 await desktop(page);await send(page,1);
 const popup=page.getByRole('dialog',{name:'Download from Nexus'});
 await expect(popup.getByRole('button',{name:'OK',exact:true})).toBeFocused();
 await expect(popup.getByRole('textbox')).toHaveCount(0);
 await expect(popup).toContainText('C:\\Library\\Downloads');
 await page.screenshot({path:'test-results/nexus-confirm.png',fullPage:true});
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(0);
 await popup.getByRole('button',{name:'OK',exact:true}).click();await expect(popup).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Add mod',exact:true})).toBeEnabled();
 await page.getByRole('button',{name:/^Downloads/}).click();
 const panel=page.getByRole('dialog',{name:'Downloads',exact:true});await expect(panel.getByText('Downloading',{exact:true})).toBeVisible();
 await page.evaluate(()=>(window as any).emit('nexus-download-progress',{request_id:'job-1',received:524288,total:1048576}));
 await expect(panel.getByRole('progressbar')).toHaveAttribute('value','524288');
 await finish(page,1);await expect(panel.getByText('Added to library',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='import_download').args)).toEqual({path:'D:/Nexus downloads/Outfit [nexus-1]/files/file-10/mod.zip'});
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await expect(page.locator('.mod-row')).toContainText('Downloaded outfit');
 await expect(page.getByRole('button',{name:'Enable in game',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('SESSION_KEY');
});

test('a downloaded Nexus thumbnail loads in the library and inspector without sending the API key',async({page})=>{
 const image='https://staticdelivery.nexusmods.com/mods/6919/images/476/preview.jpg';
 const config=JSON.parse(readFileSync(new URL('../../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
 await page.route('http://127.0.0.1:1422/',async route=>{
  const response=await route.fetch();
  await route.fulfill({response,headers:{...response.headers(),'content-security-policy':config.app.security.csp}});
 });
 const requests:Record<string,string>[]=[];
 await page.route(image,async route=>{
  requests.push(await route.request().allHeaders());
  await route.fulfill({contentType:'image/png',body:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64')});
 });
 await desktop(page);await page.evaluate(image=>{(window as any).importedThumbnail=image},image);
 await send(page,476);await page.getByRole('button',{name:'OK',exact:true}).click();
 await page.getByRole('button',{name:/^Downloads/}).click();await finish(page,476);
 await expect(page.getByText('Added to library',{exact:true})).toBeVisible();
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^All mods \d+$/}).click();
 const card=page.locator('.mod-row').first();
 await expect(card.locator('img')).toHaveAttribute('src',image);
 await expect.poll(()=>card.locator('img').evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBeGreaterThan(0);
 await card.click();await expect(page.locator('.cover img')).toHaveAttribute('src',image);
 await expect.poll(()=>page.locator('.cover img').evaluate((img:HTMLImageElement)=>img.naturalWidth)).toBeGreaterThan(0);
 expect(requests.length).toBeGreaterThan(0);
 for(const headers of requests){expect(headers).not.toHaveProperty('apikey');expect(headers).not.toHaveProperty('authorization');expect(headers).not.toHaveProperty('referer');expect(JSON.stringify(headers)).not.toContain('SESSION_KEY')}
 await page.screenshot({path:'test-results/nexus-thumbnail.png',fullPage:true});
});

test('queue runs one download at a time, can pause waiting work, cancel, and retry',async({page})=>{
 await desktop(page);await send(page,1);await page.getByRole('button',{name:'OK',exact:true}).click();
 await send(page,2);await page.getByRole('button',{name:'OK',exact:true}).click();
 await page.getByRole('button',{name:/^Downloads/}).click();const panel=page.getByRole('dialog',{name:'Downloads',exact:true});
 await expect(panel.getByText('Waiting',{exact:true})).toHaveCount(1);
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await panel.getByRole('button',{name:'Pause queue',exact:true}).click();await finish(page,1);
 await expect(panel.getByText('Added to library',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await panel.getByRole('button',{name:'Resume queue',exact:true}).click();
 await expect(panel.getByText('Downloading',{exact:true})).toBeVisible();await panel.getByRole('button',{name:'Cancel',exact:true}).click();
 await expect(panel.getByText('Cancelled',{exact:true})).toBeVisible();
 await panel.getByRole('button',{name:'Retry download',exact:true}).click();await expect(panel.getByText('Downloading',{exact:true})).toBeVisible();
 await finish(page,2);await expect(panel.getByText('Added to library',{exact:true})).toHaveCount(2);
 await panel.getByRole('button',{name:'Clear finished',exact:true}).click();await expect(panel.getByText('No downloads yet',{exact:true})).toBeVisible();
});

test('download destination persists, cancel never downloads, and Downloads stays at the bottom',async({page})=>{
 await page.setViewportSize({width:900,height:600});await desktop(page);
 const button=page.getByRole('button',{name:/^Downloads/});await expect(button).toBeInViewport();
 await button.click();await page.getByRole('button',{name:'Change',exact:true}).click();
 await expect(page.getByRole('dialog')).toContainText('D:\\Nexus downloads');
 await page.reload();await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Default download folder',{exact:true})).toHaveValue('D:/Nexus downloads');
 await page.getByLabel('Nexus API key',{exact:true}).fill('SESSION_KEY');await page.keyboard.press('Escape');
 await send(page,1);await expect(page.getByRole('dialog')).toContainText('D:\\Nexus downloads');
 await page.getByRole('button',{name:'Cancel',exact:true}).click();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='download_nxm'))).toBe(false);
 await page.screenshot({path:'test-results/download-button-900.png',fullPage:true});
});

test('failed imports retain the archive and retry importing without downloading it again',async({page})=>{
 await desktop(page);await page.evaluate(()=>{(window as any).failImports=true});
 await send(page,1);await page.getByRole('button',{name:'OK',exact:true}).click();await page.getByRole('button',{name:/^Downloads/}).click();
 await finish(page,1);const panel=page.getByRole('dialog',{name:'Downloads',exact:true});
 await expect(panel.getByText('Could not add to library',{exact:true})).toBeVisible();
 await expect(panel.getByRole('button',{name:'Open folder',exact:true})).toBeEnabled();
 await page.evaluate(()=>{(window as any).failImports=false});await panel.getByRole('button',{name:'Retry import',exact:true}).click();
 await expect(panel.getByText('Added to library',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await page.screenshot({path:'test-results/download-queue.png',fullPage:true});
});

test('7z/RAR remain available and are clearly marked as requiring extraction',async({page})=>{
 await desktop(page);await send(page,1);await page.getByRole('button',{name:'OK',exact:true}).click();await page.getByRole('button',{name:/^Downloads/}).click();
 await finish(page,1,false);await expect(page.getByText('Downloaded · extraction needed',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as any).calls.some((c:any)=>c.cmd==='import_download'))).toBe(false);
 const panel=page.getByRole('dialog',{name:'Downloads',exact:true});
 await expect(panel.getByRole('button',{name:'Open folder',exact:true})).toBeEnabled();
 await panel.getByRole('button',{name:'Open folder',exact:true}).locator('svg').click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='open_folder')?.args)).toEqual({path:'D:/Nexus downloads/Outfit [nexus-1]/files/file-10'});
});

test('RAR downloads remain in Managed mods after restarting, with metadata and no installation actions',async({page})=>{
 const image='https://staticdelivery.nexusmods.com/mods/7480/images/651/preview.jpg';
 await page.route(image,route=>route.fulfill({contentType:'image/png',body:Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64')}));
 await desktop(page,{library_dir:'C:/Library',downloads_dir:'D:/Nexus downloads'});
 await page.evaluate(image=>{(window as any).archiveThumbnail=image},image);
 await send(page,651);await page.getByRole('button',{name:'OK',exact:true}).click();
 await page.getByRole('button',{name:/^Downloads/}).click();await finish(page,651,false,'rar');
 await expect(page.getByText('Downloaded · extraction needed',{exact:true})).toBeVisible();
 await page.keyboard.press('Escape');await page.getByRole('button',{name:/^All mods \d+$/}).click();
 const card=page.locator('.mod-row').filter({hasText:'Downloaded package 651'});
 await expect(card).toContainText('Downloaded archive');await card.click();
 await expect(card.locator('img')).toHaveAttribute('src',image);
 await expect(page.locator('.inspector-status')).toHaveText('Not installed');
 await expect(page.getByRole('button',{name:'Enable in game',exact:true})).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Copy to library',exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:'Open folder',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as any).calls.find((c:any)=>c.cmd==='open_folder')?.args)).toEqual({path:'D:/Nexus downloads/Outfit [nexus-651]/files/file-6510'});
 await page.reload();await page.getByRole('button',{name:/^All mods \d+$/}).click();
 await expect(card).toBeVisible();await expect(page.locator('.inspector-status')).toHaveText('Not installed');
 await page.locator('.mod-metadata summary').filter({hasText:'Metadata'}).click();
 await expect(page.locator('.metadata-fields')).toContainText('Archive file');
 await expect(page.locator('.metadata-fields')).toContainText('651');
 await card.click({button:'right'});
 const menu=page.getByRole('menu',{name:'Context menu'});
 await expect(menu.getByRole('menuitem',{name:'Copy to library',exact:true})).toHaveCount(0);
 await expect(menu.getByRole('menuitem',{name:'Enable in game',exact:true})).toHaveCount(0);
 await page.keyboard.press('Escape');
 await page.screenshot({path:'test-results/downloaded-archive-library.png',fullPage:true});
});

for(const {name,library_dir,downloads_dir,expected,placeholder=expected} of [
 {name:'Windows library with spaces',library_dir:'F:\\InZoi mod',expected:'F:\\InZoi mod\\Downloads'},
 {name:'Windows library with trailing separator',library_dir:'F:\\InZoi mod\\',expected:'F:\\InZoi mod\\Downloads'},
 {name:'custom Windows download folder with mixed separators',library_dir:'F:\\InZoi mod',downloads_dir:'F:\\My mods/Archives',expected:'F:\\My mods\\Archives',placeholder:'F:\\InZoi mod\\Downloads'},
 {name:'UNC library',library_dir:'\\\\server\\mods',expected:'\\\\server\\mods\\Downloads'},
 {name:'macOS library',library_dir:'/Users/test/Mods/',expected:'/Users/test/Mods/Downloads'},
])test(`download path stays consistent in confirmation and queue: ${name}`,async({page})=>{
 await desktop(page,{library_dir,downloads_dir});
 await page.getByRole('button',{name:'Settings'}).click();
 await expect(page.getByLabel('Default download folder',{exact:true})).toHaveAttribute('placeholder',placeholder);
 await page.keyboard.press('Escape');await send(page,476);
 const path=page.locator('.download-confirm-path');
 await expect(path).toHaveText(expected);await expect(path).toHaveAttribute('title',expected);
 if(name==='Windows library with spaces')await page.screenshot({path:'test-results/download-confirm-windows-path.png',fullPage:true});
 await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await page.getByRole('button',{name:/^Downloads/}).click();
 await expect(page.locator('.download-destination p')).toHaveText(expected);
});


test('confirming Nexus sends its download icon to the queue without blocking downloads or navigation',async({page})=>{
 await desktop(page);await send(page,1);
 const popup=page.getByRole('dialog',{name:'Download from Nexus',exact:true});
 const origin=(await popup.locator('.download-confirm>svg').boundingBox())!;
 const destination=(await page.locator('.downloads-button>svg').boundingBox())!;
 await popup.getByRole('button',{name:'OK',exact:true}).click();await expect(popup).toHaveCount(0);
 const flight=page.locator('.download-handoff-flight');await expect(flight).toBeVisible();
 await flight.evaluate(node=>{const animation=node.getAnimations()[0];animation.pause();animation.currentTime=0;});
 const start=(await flight.boundingBox())!;expect(Math.abs(start.x+start.width/2-origin.x-origin.width/2)).toBeLessThan(2);expect(Math.abs(start.y+start.height/2-origin.y-origin.height/2)).toBeLessThan(2);
 expect(await flight.evaluate(node=>getComputedStyle(node).pointerEvents)).toBe('none');
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(1);
 await flight.evaluate(node=>{node.getAnimations()[0].currentTime=350;});await page.screenshot({path:'test-results/download-handoff-nexus.png'});
 await page.getByRole('button',{name:/^Downloads/}).click();await expect(page.getByRole('dialog',{name:'Downloads',exact:true})).toBeVisible();
 await flight.evaluate(node=>{node.getAnimations()[0].currentTime=619;});const end=(await flight.boundingBox())!;
 expect(Math.abs(end.x+end.width/2-destination.x-destination.width/2)).toBeLessThan(3);expect(Math.abs(end.y+end.height/2-destination.y-destination.height/2)).toBeLessThan(3);
 await flight.evaluate(node=>node.getAnimations()[0].finish());await expect(flight).toHaveCount(0);await expect(page.getByRole('status').filter({hasText:'Added to downloads'})).toBeVisible();
 await expect(page.locator('.download-handoff-notice')).toHaveCount(0,{timeout:4000});
});

test('cancel does not animate and reduced motion confirms the queue without a flying icon',async({page})=>{
 await page.emulateMedia({reducedMotion:'reduce'});await desktop(page);await send(page,1);
 const popup=page.getByRole('dialog',{name:'Download from Nexus',exact:true});await popup.getByRole('button',{name:'Cancel',exact:true}).click();await expect(page.locator('.download-handoff-flight')).toHaveCount(0);await expect(page.getByRole('status').filter({hasText:'Added to downloads'})).toHaveCount(0);
 expect(await page.evaluate(()=>(window as any).calls.filter((c:any)=>c.cmd==='download_nxm').length)).toBe(0);
 await send(page,2);await popup.getByRole('button',{name:'OK',exact:true}).click();await expect(page.locator('.download-handoff-flight')).toHaveCount(0);await expect(page.getByRole('status').filter({hasText:'Added to downloads'})).toBeVisible();
 expect(await page.locator('.downloads-button').evaluate(node=>node.getAnimations().every(animation=>!(animation.effect as KeyframeEffect).getKeyframes().some(frame=>frame.transform)))).toBe(true);
 await page.getByRole('button',{name:/^Downloads/}).click();await expect(page.getByRole('dialog',{name:'Downloads',exact:true})).toContainText('Downloading');
});
