export type CheckLog={id:string;run_id:string;timestamp:number;mod_id:string;mod_name:string;project:string;provider:string;status:string;code:string;duration_ms:number;detail:string};
export function checkFailureCode(error:string){
 const text=error.toLowerCase();
 if(text.includes('apikeyrequired'))return 'missing_key';
 if(/command.*not found|unknown command|invalid args|missing required key/.test(text))return 'backend_mismatch';
 for(const status of ['401','403','404','429'])if(text.includes(`http ${status}`))return `http_${status}`;
 if(/timed out|timeout/.test(text))return 'timeout';
 if(/connect|tls|certificate/.test(text))return 'network';
 if(/no longer exists|not found/.test(text))return 'missing_mod';
 return 'other';
}
export function redactCheckLog(value:string,keys:string[]=[]){
 for(const key of keys.map(key=>key.trim()).filter(Boolean))value=value.split(key).join('[REDACTED]');
 return value.replace(/(?:https?|nxm):\/\/[^\s<>"']+/gi,raw=>{try{const url=new URL(raw);url.username='';url.password='';url.search='';url.hash='';return url.href}catch{return '[URL]'}});
}
export function formatCheckLog(entries:CheckLog[],keys:string[]=[]){
 return redactCheckLog(entries.map(entry=>`[${new Date(entry.timestamp).toISOString()}] ${entry.provider||'unknown'} | ${entry.mod_name} | ${entry.project}\n${entry.status} | ${entry.code} | ${entry.duration_ms} ms\n${entry.detail}`).join('\n\n'),keys);
}

export function validCheckLogs(value:unknown):CheckLog[]{
 if(!Array.isArray(value))return[];
 return value.filter((entry):entry is CheckLog=>entry&&typeof entry.id==='string'&&typeof entry.run_id==='string'&&Number.isFinite(entry.timestamp)&&entry.timestamp>0&&Number.isFinite(entry.duration_ms)&&['mod_id','mod_name','project','provider','status','code','detail'].every(key=>typeof entry[key]==='string')).slice(-500);
}
export function savedCheckLogs():CheckLog[]{try{return validCheckLogs(JSON.parse(localStorage.getItem('inzoimm.modCheckLog')||'[]'))}catch{return[]}}
