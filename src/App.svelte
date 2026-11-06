<script lang="ts">
 import { onMount } from 'svelte';
 import { invoke, convertFileSrc, isTauri } from '@tauri-apps/api/core';
 import { listen } from '@tauri-apps/api/event';
 import { open, confirm } from '@tauri-apps/plugin-dialog';
 import { openPath, openUrl } from '@tauri-apps/plugin-opener';
 import { check, type Update } from '@tauri-apps/plugin-updater';
 import { relaunch } from '@tauri-apps/plugin-process';
 import { getCurrent } from '@tauri-apps/plugin-deep-link';
 import { IconSearch, IconPlus, IconRefresh, IconSettings, IconBox, IconFolder, IconLink, IconArrowUpRight, IconDownload, IconTrash, IconPencil, IconCommand, IconCheck, IconX, IconChevronRight } from '@tabler/icons-svelte';
 import type { Mod, Snapshot, Settings } from './lib/types';
 let data=$state<Snapshot|null>(null), query=$state(''), filter=$state('all'), selected=$state(''), busy=$state(false), error=$state(''), message=$state('');
 let settingsOpen=$state(false), draft=$state<Settings|null>(null), editOpen=$state(false), editName=$state(''), editDescription=$state('');
 let linkOpen=$state(false), link=$state(''), nexus=$state<{mod_id:number;file_id:number;url:string}|null>(null), update=$state<Update|null>(null);
 let input:HTMLInputElement;
 const filtered=$derived((data?.mods??[]).filter(m=>(filter==='all'||filter==='managed'&&m.managed||filter==='external'&&!m.managed||filter==='enabled'&&m.enabled)&&`${m.name} ${m.author} ${m.project}`.toLowerCase().includes(query.toLowerCase())));
 const active=$derived(filtered.find(m=>m.id===selected)??filtered[0]);
 async function run(action:()=>Promise<void>){if(busy)return;busy=true;error='';message='';try{await action()}catch(e){error=String(e)}finally{busy=false}}
 async function refresh(){data=await invoke<Snapshot>('snapshot')}
 async function add(){await run(async()=>{const p=await open({directory:true,multiple:false,title:'Chọn thư mục mod inZOI'});if(p){await invoke('import_folder',{path:p});await refresh();message='Đã thêm mod vào thư viện. Bật mod khi bạn muốn cài vào game.'}})}
 async function toggle(m:Mod){await run(async()=>{await invoke('toggle_mod',{id:m.id});await refresh()})}
 async function remove(m:Mod){if(await confirm(`Chuyển “${m.name}” vào thùng rác của thư viện?`,{title:'Gỡ mod',kind:'warning'})){await run(async()=>{await invoke('remove_mod',{id:m.id});await refresh()})}}
 async function updates(){await run(async()=>{update=await check();message=update?`Có bản cập nhật ${update.version}.`:'Bạn đang dùng phiên bản mới nhất.'})}
 async function receiveLink(raw:string){await run(async()=>{nexus=await invoke('parse_nxm',{link:raw});linkOpen=true;link=raw})}
 onMount(()=>{if(!isTauri()){error='Mở ứng dụng desktop bằng npm run desktop để đọc thư viện trên máy.';return}
  refresh().catch(e=>error=String(e));const unlisten=listen<{mod_id:number;file_id:number;url:string}>('nexus-link',e=>{nexus=e.payload;linkOpen=true});getCurrent().then(urls=>{for(const u of urls??[])if(u.startsWith('nxm:'))void receiveLink(u)}).catch(e=>error=String(e));return()=>{void unlisten.then(f=>f())};
 });
 function keyboard(e:KeyboardEvent){if((e.ctrlKey||e.metaKey)&&e.key==='k'){e.preventDefault();input?.focus()}if(e.key==='Escape'){settingsOpen=false;linkOpen=false;editOpen=false}if(e.target instanceof HTMLInputElement||e.target instanceof HTMLTextAreaElement)return;if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();const i=filtered.findIndex(m=>m.id===active?.id);selected=filtered[Math.max(0,Math.min(filtered.length-1,i+(e.key==='ArrowDown'?1:-1)))]?.id??''}}
</script>
<svelte:window onkeydown={keyboard}/>
<div class="shell">
 <aside>
  <div class="brand"><img src="/app-icon.png" alt=""/><div>inZOI<span>Better Mod Manager</span></div></div>
  <div class="section-label">THƯ VIỆN</div>
  {#each [{id:'all',name:'Tất cả mod',icon:IconBox},{id:'enabled',name:'Đang bật',icon:IconCheck},{id:'managed',name:'Do app quản lý',icon:IconFolder},{id:'external',name:'Đã có trong game',icon:IconArrowUpRight}] as tab}
   <button class:chosen={filter===tab.id} onclick={()=>filter=tab.id}><tab.icon size={18}/>{tab.name}<small>{(data?.mods??[]).filter(m=>tab.id==='all'||tab.id==='enabled'&&m.enabled||tab.id==='managed'&&m.managed||tab.id==='external'&&!m.managed).length}</small></button>
  {/each}
  <div class="section-label space">KẾT NỐI</div>
  <button onclick={()=>{linkOpen=true;nexus=null}}><IconLink size={18}/>Liên kết Nexus / Vortex</button>
  <button onclick={()=>void updates()} disabled={busy}><IconDownload size={18}/>Cập nhật ứng dụng</button>
  <div class="aside-bottom"><div class="game"><span class="game-dot"></span><div>inZOI<small>Thư viện trên máy của bạn</small></div></div><button onclick={()=>{draft=data?{...data.settings}:null;settingsOpen=true}}><IconSettings size={18}/>Cài đặt<kbd>⌘ ,</kbd></button></div>
 </aside>
 <main>
  <header><div class="search"><IconSearch size={20}/><input bind:this={input} bind:value={query} placeholder="Tìm mod, tác giả, tên dự án…" aria-label="Tìm mod"/><kbd>Ctrl K</kbd></div><button class="icon" title="Quét lại thư viện" disabled={busy} onclick={()=>void run(refresh)}><IconRefresh size={18} class={busy?'spin':''}/></button></header>
  <div class="page-title"><div><div class="breadcrumb">THƯ VIỆN CÁ NHÂN <IconChevronRight size={12}/> inZOI</div><h1>{filter==='external'?'Mod đã có trong game':filter==='enabled'?'Đang bật':filter==='managed'?'Thư viện của bạn':'Tất cả mod'}</h1><p>{filtered.length} mod · Dữ liệu cục bộ, trong tầm kiểm soát</p></div><button class="primary" disabled={busy||!data} onclick={add}><IconPlus size={17}/>Thêm mod</button></div>
  {#if error}<div class="notice error" role="alert">{error}<button class="icon" onclick={()=>error=''} aria-label="Đóng lỗi"><IconX size={16}/></button></div>{/if}
  {#if message}<div class="notice" role="status">{message}</div>{/if}
  {#if update}<div class="notice">Bản mới {update.version}<button onclick={()=>void run(async()=>{await update!.downloadAndInstall();await relaunch()})}>Tải và khởi động lại</button></div>{/if}
  {#if data?.warnings.length}<details class="warnings"><summary>{data.warnings.length} thông tin cần kiểm tra khi quét</summary>{#each data.warnings as warning}<p>{warning}</p>{/each}</details>{/if}
  <div class="workspace"><section class="mod-list" aria-label="Danh sách mod"><div class="list-head"><span>MOD</span><span>TRẠNG THÁI</span></div>
  {#each filtered as mod (mod.id)}<button class="mod-row" class:selected={active?.id===mod.id} onclick={()=>selected=mod.id}><div class="thumb">{#if mod.thumbnail}<img src={convertFileSrc(mod.thumbnail)} alt=""/>{:else}<IconBox size={25} stroke={1.25}/>{/if}</div><div class="mod-title"><strong>{mod.name||mod.project}</strong><span>{mod.author||'Chưa rõ tác giả'} <span class="dot-sep">·</span> {mod.managed?'Thư viện riêng':'Trong game'}</span></div><span class:enabled={mod.enabled} class="status">{mod.enabled?'Đang bật':'Đã tắt'}</span></button>{/each}
  {#if filtered.length===0}<div class="empty"><IconBox size={42} stroke={1}/><h2>{query?'Không tìm thấy mod':'Một khởi đầu mới cho thư viện'}</h2><p>{query?'Thử một tên hoặc tác giả khác.':'Thêm thư mục mod, hoặc chọn thư mục Mods của inZOI trong Cài đặt để nhận diện mod đang có.'}</p>{#if !query}<button onclick={add} disabled={!data||busy}>Chọn thư mục mod <IconArrowUpRight size={16}/></button>{/if}</div>{/if}
  </section><section class="inspector" aria-label="Chi tiết mod">{#if active}<div class="cover">{#if active.thumbnail}<img src={convertFileSrc(active.thumbnail)} alt={active.name}/>{:else}<IconBox size={66} stroke={0.8}/><span>inZOI / {active.project}</span>{/if}</div><div class="inspector-body"><span class="eyebrow">{active.managed?'THƯ VIỆN RIÊNG':'ĐƯỢC PHÁT HIỆN TRONG GAME'}</span><h2>{active.name||active.project}</h2><p class="author">bởi {active.author||'Chưa rõ tác giả'}</p><p class="description">{active.description||'Mod chưa có mô tả.'}</p><dl><div><dt>Phiên bản</dt><dd>{active.version||'—'}</dd></div><div><dt>Trạng thái</dt><dd>{active.enabled?'Đang bật':'Đã tắt'}</dd></div><div><dt>Nguồn quản lý</dt><dd>{active.managed?'Better Mod Manager':'Bên ngoài ứng dụng'}</dd></div></dl>{#if active.managed}<button class="wide primary" disabled={busy} onclick={()=>toggle(active!)}>{active.enabled?'Tắt mod':'Bật trong game'}</button><div class="actions"><button onclick={()=>{editName=active!.name;editDescription=active!.description;editOpen=true}}><IconPencil size={16}/>Chỉnh sửa</button><button class="danger" disabled={busy||active.enabled} onclick={()=>remove(active!)}><IconTrash size={16}/>Gỡ</button></div>{:else}<div class="external-note">Mod này đã có sẵn. Quét thư viện không thay đổi file của game.</div><button class="wide" disabled={busy} onclick={()=>void run(async()=>{await invoke('import_folder',{path:active!.path});await refresh();message='Đã sao chép vào thư viện riêng. Bản cài hiện tại được giữ nguyên.'})}>Sao chép vào thư viện</button>{/if}<button class="wide ghost" onclick={()=>void run(async()=>{await openPath(active!.path)})}><IconFolder size={16}/>Mở thư mục</button>{#if active.source.startsWith('https://')}<button class="wide ghost" onclick={()=>void run(async()=>{await openUrl(active!.source)})}>Trang của tác giả <IconArrowUpRight size={15}/></button>{/if}</div>{:else}<div class="inspector-empty"><IconCommand size={28} stroke={1}/><p>Chọn một mod để xem chi tiết</p></div>{/if}</section></div>
  <footer><span><span class="game-dot"></span>{busy?'Đang xử lý…':'Thư viện cục bộ'}</span><span><kbd>↑</kbd><kbd>↓</kbd> Điều hướng <kbd>Ctrl K</kbd> Tìm kiếm</span></footer>
 </main>
</div>
{#if settingsOpen||editOpen||linkOpen}<div class="overlay"><div class="modal" role="dialog" aria-modal="true" aria-label={settingsOpen?'Cài đặt':editOpen?'Chỉnh sửa mod':'Liên kết Nexus'} tabindex="-1"><div class="modal-head"><h2>{settingsOpen?'Cài đặt thư viện':editOpen?'Chỉnh sửa mod':'Liên kết Nexus / Vortex'}</h2><button class="icon" onclick={()=>{settingsOpen=false;editOpen=false;linkOpen=false}} aria-label="Đóng"><IconX size={20}/></button></div>
 {#if settingsOpen&&draft}<p class="muted">Chọn đúng thư mục Mods để nhận diện các mod đã có.</p>{#each [{key:'mods_dir' as const,name:'Thư mục mod của game'},{key:'library_dir' as const,name:'Thư viện riêng'},{key:'vortex_dir' as const,name:'Dữ liệu Vortex'}] as f}<label>{f.name}<div class="path-input"><input bind:value={draft[f.key]}/><button title="Chọn thư mục" onclick={()=>void run(async()=>{const p=await open({directory:true});if(p&&draft)draft[f.key]=p})}><IconFolder size={18}/></button></div></label>{/each}<p class="muted">Cập nhật từ coolbirdzik/inzoi-better-mod-manager. Bản cập nhật phải có chữ ký hợp lệ.</p><button class="wide" onclick={()=>void run(async()=>{if(await confirm('Dùng app này để mở liên kết nxm thay ứng dụng đang được đăng ký?',{title:'Liên kết Nexus'})){await invoke('register_nxm');message='Đã đăng ký nhận liên kết nxm cho inZOI.'}})}>Đặt làm ứng dụng nhận liên kết Vortex</button><button class="primary wide" disabled={busy} onclick={()=>void run(async()=>{await invoke('save_settings',{settings:draft});await refresh();settingsOpen=false})}>Lưu cài đặt</button>
 {:else if editOpen}<label>Tên mod<input bind:value={editName}/></label><label>Mô tả<textarea bind:value={editDescription} rows="5"></textarea></label><button class="primary wide" disabled={busy} onclick={()=>void run(async()=>{await invoke('edit_mod',{id:active!.id,name:editName,description:editDescription});await refresh();editOpen=false})}>Lưu thay đổi</button>
 {:else if linkOpen}<p class="muted">Dán liên kết nxm://inzoi/… từ Nexus Mods. Các liên kết dành cho game khác sẽ bị từ chối.</p><label>Liên kết<input bind:value={link} placeholder="nxm://inzoi/mods/123/files/456"/></label><button class="primary wide" disabled={busy} onclick={()=>receiveLink(link)}>Đọc liên kết</button>{#if nexus}<div class="link-info">inZOI · Mod #{nexus.mod_id} · File #{nexus.file_id}</div><p class="muted">Mở trang Nexus để tải file. Tải tự động có xác thực Nexus đang được phát triển.</p><button class="wide" onclick={()=>void run(async()=>{await openUrl(nexus!.url)})}>Mở file trên Nexus <IconArrowUpRight size={16}/></button>{/if}{/if}
 {#if error}<p class="form-error" role="alert">{error}</p>{/if}
 </div></div>{/if}
