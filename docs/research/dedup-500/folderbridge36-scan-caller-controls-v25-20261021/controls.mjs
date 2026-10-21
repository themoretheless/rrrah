import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const root=process.argv[2];
const url=s=>`data:text/javascript;base64,${Buffer.from(s).toString('base64')}`;
const normalizePath=p=>p.replaceAll('\\','/').replace(/^\/+|\/+$/g,'');
const original=p=>readFileSync(`${root}/${p}`,'utf8');
const filter=stripTypeScriptTypes(original('src/mountFileFilter.ts').replace(/^import .*\n/gm,''));
const scanner=stripTypeScriptTypes(original('src/mountScan.ts').replace(/^import .*\n/gm,''));
const {replayMountContentsToVault}=await import(url(`const normalizePath=${normalizePath.toString()};\n${filter}\n${scanner}`));
const main=original('main.ts');
const start=main.indexOf('\tasync notifyVaultMountAdded(');
const end=main.indexOf('\n\t/**\n\t * Remove a virtual mount folder',start);
assert(start>0&&end>start);
const build=(Notice,logger)=>new Function('Notice','logger','normalizePath','replayMountContentsToVault','performance','setTimeout', 'return '+stripTypeScriptTypes(`class ScanCaller {${main.slice(start,end)}}`))(Notice,logger,normalizePath,replayMountContentsToVault,{now:()=>0},()=>0);
async function scenario(failingStat,maxFiles=0) {
 const messages=[],errors=[],created=[],watchers=[];
 class Notice {constructor(m){messages.push(m)}setMessage(m){messages.push(m)}hide(){}}
 const Caller=build(Notice,{debug:(...args)=>errors.push(args.map(String))});
 const plugin=new Caller();
 const mount={id:'one',virtualPath:'Docs',realPath:'/synthetic',mountType:'local',enabled:true,maxFiles};
 const files=Array.from({length:10},(_,i)=>`Docs/${i}.md`);
 plugin.mountInjections=new Map();plugin.withSafeDeviceOverride=m=>m;plugin.resolveMountPath=async()=>{};plugin.isNameIgnored=()=>false;
 plugin.fileWatcher={stopWatching:()=>{},startWatching:()=>watchers.push('started')};
 plugin.scheduleExplorerWork=()=>{};
 plugin.app={vault:{getAbstractFileByPath:()=>null,onChange:async(event,p)=>{if(event==='file-created')created.push(p)},adapter:{list:async()=>({files,folders:[]}),stat:async p=>{if(p===failingStat)throw Error('synthetic stat failure');return {type:'file',ctime:0,mtime:0,size:0}}}}};
 await plugin.notifyVaultMountAdded(mount);
 return {messages,errors,created,watchers};
}
let s=await scenario(null);
assert.equal(s.created.length,10);assert(s.messages.some(m=>m.includes('Mounted 0 folders and 10 files')));assert.equal(s.errors.length,0);
console.log(JSON.stringify({name:'healthy-scan-announces-all-ten-files',passed:true}));
s=await scenario('Docs/2.md');
assert.equal(s.created.length,2);assert.equal(s.errors.length,1);assert.deepEqual(s.watchers,['started']);assert(s.messages.some(m=>m.includes('Mounted 0 folders and 2 files')));assert(!s.messages.some(m=>/limit|fail|partial|incomplete/i.test(m)));
console.log(JSON.stringify({name:'failed-stat-leaves-two-of-ten-and-success-style-mounted-message-without-incomplete-warning',passed:true}));
s=await scenario(null,2);
assert.equal(s.created.length,2);assert(s.messages.some(m=>m.includes('Scan limit (2 items) reached')));
console.log(JSON.stringify({name:'explicit-scan-cap-does-have-a-separate-limit-warning',passed:true}));
