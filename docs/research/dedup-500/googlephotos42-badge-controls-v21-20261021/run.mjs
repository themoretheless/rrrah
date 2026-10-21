import fs from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
let source=stripTypeScriptTypes(await fs.readFile('.dedup-work/googlephotos42-20261021/src/extension/background.ts','utf8'));
for(const line of ["import { setBadgeBackgroundColor, setBadgeText } from './api'", "import { registerLifecycle } from './lifecycle'"]){assert.ok(source.includes(line));source=source.replace(line,'');}
let listener;let now=1000;let timerId=0;const timers=new Map();const writes=[];
const context={console,Date:{now:()=>now},setTimeout:fn=>{const id=++timerId;timers.set(id,fn);return id},clearTimeout:id=>timers.delete(id),setBadgeText:async v=>{writes.push(v.text)},setBadgeBackgroundColor:async()=>{},registerLifecycle:()=>{},chrome:{runtime:{onMessage:{addListener:fn=>{listener=fn}}},tabs:{onRemoved:{addListener:()=>{}}}}};
vm.runInNewContext(source,context);
const progress=(status,deleted)=>listener({type:'progress',data:{status,deleted}},{tab:{id:1}});
progress('selecting',1);now=1050;progress('selecting',5);assert.equal(timers.size,1);
now=1200;progress('done',9);assert.equal(writes.at(-1),'');
for(const fn of [...timers.values()])fn();
assert.deepEqual(writes,['1','','5']);
console.log('PASS: pending old progress overwrites newer done badge when new message runs before due timer');
