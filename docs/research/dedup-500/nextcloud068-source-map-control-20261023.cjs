// Compare original saga source bodies; only imports/exports and effects adapted.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const original = fs.readFileSync(process.argv[2], 'utf8');
const mapBytes = fs.readFileSync(process.argv[3]);
const map = JSON.parse(mapBytes);
const index = map.sources.findIndex(s => s.endsWith('/js-src/settings/sagas/settingsSagas.js'));
assert(index >= 0);
const embedded = map.sourcesContent[index];
function inspect(source) {
 const imports = [...source.matchAll(/import[\s\S]*?from '[^']+'\n/g)];
 assert(imports.length === 3 || imports.length === 4);
 let adapted = source.replace(/import[\s\S]*?from '[^']+'\n/g, '');
 adapted = adapted.replace(/export default \[\s*watchForView,\s*watchForAction\s*\]/, '');
 adapted += '\nthis.controls = {saveSetting,loadSettings};';
 const context = {window:{},config:{appID:'duplicatefinder'},
 createReducerConstants:()=>({app:{f:{}}}),
 API:{patch:(...args)=>({effect:'patch',args}),get:(...args)=>({effect:'get',args})},
 setStep:(...args)=>({effect:'step',args}),gettext:x=>x,
 };
 vm.createContext(context);vm.runInContext(adapted,context);
 const save = context.controls.saveSetting('ignore_mounted_files',true,false);
 const effect = save.next().value;
 assert.equal(effect.effect,'patch');
 context.controls.loadSettings().next();
 return {source_sha256:crypto.createHash('sha256').update(source).digest('hex'),
   request_body:JSON.parse(JSON.stringify(effect.args[3])),
   ambient_app_id:context.window.appID || null};
}
const current=inspect(original), built=inspect(embedded);
assert.deepEqual(current.request_body,{newConfig:{ignore_mounted_files:true}});
assert.deepEqual(built.request_body,{config:{ignore_mounted_files:true}});
assert.equal(current.ambient_app_id,null);
assert.equal(built.ambient_app_id,'duplicatefinder');
console.log(JSON.stringify({map_sha256:crypto.createHash('sha256').update(mapBytes).digest('hex'),source_index:index,current,embedded:built,controls:2,passed:2,scope:'Complete original current and map-embedded saga module bodies with effect/import adapters. Request payload and first generator step only; no fetch, server, JSX, Redux scheduler or compiled bundle execution.'},null,2));
