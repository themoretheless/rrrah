import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=process.argv[2];
const source=readFileSync(`${root}/main.ts`,'utf8');
// Exact complete method ranges, bounded by the following method declaration.
const part=(start,end)=>source.slice(source.indexOf(start),source.indexOf(end,source.indexOf(start)));
const methods=[part('\tprivate getManagedTocSourcePath()', '\tresolveAndCacheManagedTocSource()'),part('\tprivate getManagedTocDraftMounts()', '\tprivate getManagedTocIndex('),part('\tprivate async writeManagedTocMounts(', '\tasync bindManagedTocSource('),part('\tasync bindManagedTocSource(', '\tasync createManagedTocFromCurrentMounts(')].join('\n');
const dataUrl=s=>`data:text/javascript;base64,${Buffer.from(s).toString('base64')}`;
const normalizePath=p=>p.replaceAll('\\','/').replace(/^\/+|\/+$/g,'');
const tocSource=readFileSync(`${root}/src/TocConfig.ts`,'utf8').replace(/^import .*\n/gm,'');
const {serializeTocConfig}=await import(dataUrl(`const normalizePath=${normalizePath.toString()};\n${stripTypeScriptTypes(tocSource)}`));
const makeClass=new Function('fs','path','serializeTocConfig','Notice','return '+stripTypeScriptTypes(`class Bound {${methods}}`));
async function scenario(oldMounts,existingDestination,failWrite=false) {
 const memory=new Map([['/destination.json',serializeTocConfig(existingDestination)]]);
 const events=[];
 const fs={promises:{mkdir:async()=>{},writeFile:async(p,text)=>{events.push('write');if(failWrite)throw Error('synthetic write failure');memory.set(p,text);}}};
 const Bound=makeClass(fs,path,serializeTocConfig,class Notice{});
 const plugin=new Bound();
 Object.assign(plugin,{manifest:{name:'test'},settings:{managedTocSource:'/old.json',tocSources:['/destination.json']},resolvedManagedTocSource:'/old.json',managedTocRevision:0,managedTocMountPoints:oldMounts});
 plugin.isCloudMount=m=>['s3','sftp','webdav'].includes(m.mountType);
 plugin.resolveManagedTocSourcePath=p=>p.trim();
 plugin.refreshTocMountSources=async(_warn,p)=>{events.push('read');plugin.settings.mountPoints=JSON.parse(memory.get(p)).mounts;};
 plugin.saveSettings=async()=>{};
 const success=await plugin.bindManagedTocSource('/destination.json');
 return {success,memory,events,plugin};
}
const mount=(id,virtualPath)=>({id,virtualPath,realPath:`/source/${id}`,enabled:true,readOnly:false,mountType:'local'});
let s=await scenario([], [mount('dest','Destination')]);
assert.equal(s.success,true);assert.deepEqual(s.events,['write','read']);assert.deepEqual(JSON.parse(s.memory.get('/destination.json')).mounts,[]);
console.log(JSON.stringify({name:'binding-empty-current-draft-overwrites-existing-target-before-read',passed:true}));
s=await scenario([mount('old','Old')],[mount('dest','Destination')]);
assert.equal(s.success,true);assert.deepEqual(JSON.parse(s.memory.get('/destination.json')).mounts.map(m=>m.virtualPath),['Old']);
console.log(JSON.stringify({name:'binding-copies-old-draft-over-target-without-merging-destination',passed:true}));
s=await scenario([], [mount('dest','Destination')],true);
assert.equal(s.success,false);assert.equal(s.plugin.settings.managedTocSource,'/old.json');assert.deepEqual(s.plugin.settings.tocSources,[]);
console.log(JSON.stringify({name:'failed-bind-restores-managed-source-but-not-external-toc-source-list',passed:true}));
