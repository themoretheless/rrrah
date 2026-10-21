import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const root=path.resolve('.dedup-work/googlephotos42-20261021');
const out=path.join(root,'presets-runtime');await fs.mkdir(out,{recursive:true});
await fs.copyFile(path.join(root,'src/selector-packs/pack-v1.json'),path.join(out,'pack.json'));
for(const name of ['selector-pack','diagnostics','selectors','photo-filter','surface','presets']){
 let code=stripTypeScriptTypes(await fs.readFile(path.join(root,'src/core',name+'.ts'),'utf8'));
 code=code.replace(/from '\.\/([^']+)'/g,(_,dep)=>"from './"+dep+".mjs'");
 code=code.replace("from '../selector-packs/pack-v1.json'","from './pack.json' with { type: 'json' }");
 await fs.writeFile(path.join(out,name+'.mjs'),code);
}
const {buildFilterFromControls}=await import(pathToFileURL(path.join(out,'photo-filter.mjs')));
const {sanitizePresets,createPresetManager}=await import(pathToFileURL(path.join(out,'presets.mjs')));
const controls={type:'phoot',dateMode:'off',dateA:'',dateB:''};
assert.deepEqual(buildFilterFromControls(controls.type,'off','',''),{ok:true,filter:{kind:'all'}});
assert.equal(sanitizePresets([{id:'p',name:'broken',...controls}])[0].type,'all');
console.log('PASS 1: unknown type accepted and normalized into all by controls and preset sanitation');
const valid={...controls,type:'photo'};let stored=[{id:'old',name:'old',...valid}];
const manager=createPresetManager({read:async()=>{throw Error('temporary')},write:async list=>{stored=list;return true}},()=>true);
assert.equal((await manager.save('new',valid)).ok,true);assert.equal(stored.length,1);assert.equal(stored[0].name,'new');
console.log('PASS 2: read failure treated as empty, successful write replaces previous in-memory preset');
let concurrent=[];
const m=createPresetManager({read:async()=>[...concurrent],write:async list=>{concurrent=list;return true}},()=>true);
const result=await Promise.all([m.save('first',valid),m.save('second',valid)]);
assert.ok(result.every(x=>x.ok));assert.equal(concurrent.length,1);
console.log('PASS 3: concurrent saves both report success, but only one preset remains');
