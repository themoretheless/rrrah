import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const source=path.resolve('.dedup-work/googlephotos42-20261021/src/core');
const out=path.resolve('.dedup-work/googlephotos42-20261021/wait-runtime');
await fs.mkdir(out,{recursive:true});
for(const name of ['utils','run-occupancy','surface']){
 let code=stripTypeScriptTypes(await fs.readFile(path.join(source,name+'.ts'),'utf8'));
 code=code.replace("from './utils'","from './utils.mjs'");
 await fs.writeFile(path.join(out,name+'.mjs'),code);
}
const {waitUntilAbortable,StopRequested}=await import(pathToFileURL(path.join(out,'run-occupancy.mjs')));
let stopped=false;
const value=await waitUntilAbortable(async()=>{stopped=true;return 'ready'},10,1,()=>stopped,async()=>{});
assert.equal(stopped,true);assert.equal(value,'ready');
console.log('PASS 1: stop becomes true during condition, truthy result still returned');
await assert.rejects(waitUntilAbortable(()=>false,10,1,()=>true,async()=>{}),StopRequested);
console.log('PASS 2: preexisting stop correctly refuses condition polling');
const {isSupportedPhotosUrl,identifyPhotosView}=await import(pathToFileURL(path.join(out,'surface.mjs')));
for(const url of ['http://photos.google.com/','https://photos.google.com.example/','https://user@photos.google.com/','https://photos.google.com:8443/'])assert.equal(isSupportedPhotosUrl(url),false);
assert.deepEqual(identifyPhotosView('https://photos.google.com/u/2/album/abc%20def'),{kind:'album',path:'/album/abc%20def',account:'2',target:'abc def'});
console.log('PASS 3: exact URL admission rejects four mismatches and extracts account/view');
