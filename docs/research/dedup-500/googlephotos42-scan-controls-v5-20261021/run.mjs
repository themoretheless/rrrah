import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const src=path.resolve('.dedup-work/googlephotos42-20261021/src/core/dedup');
const out=path.resolve('.dedup-work/googlephotos42-20261021/scan-runtime');
await fs.mkdir(out,{recursive:true});
for(const name of ['phash','scan']) {
 let code=stripTypeScriptTypes(await fs.readFile(path.join(src,name+'.ts'),'utf8'));
 if(name==='scan') {assert.equal(code.split("from './phash'").length-1,1);code=code.replace("from './phash'","from './phash.mjs'");}
 await fs.writeFile(path.join(out,name+'.mjs'),code);
}
const {scanView}=await import(pathToFileURL(path.join(out,'scan.mjs')));
const tile=(id,url=id)=>({id,label:null,thumbUrl:url});
const zero=new Uint8Array(8);
let ticks=0;
const target={scrollTop:0,scrollHeight:100,clientHeight:100,scrollTo(){},scrollBy(){}};
const late=await scanView({findScrollTarget:()=>target,harvest:()=>ticks>=5?[tile('early'),tile('late')]:[tile('early')],sleep:async()=>{ticks++},hashThumb:async()=>zero},{pollMs:1,scrollSettleMs:1,endOfListAttempts:3});
assert.equal(ticks,4);assert.equal(late.cancelled,false);assert.deepEqual(late.items.map(i=>i.id),['early']);
console.log('PASS 1: finite idle budget ends at tick4; tile available from tick5 is omitted with cancelled=false');
let harvests=0;const attempts=[];
const refreshed=await scanView({findScrollTarget:()=>target,harvest:()=>[tile('same',++harvests===1?'expired':'fresh')],sleep:async()=>{},hashThumb:async url=>{attempts.push(url);return url==='fresh'?zero:null;}},{pollMs:1,scrollSettleMs:1,endOfListAttempts:3});
assert.deepEqual(attempts,['expired']);assert.equal(refreshed.failed,1);assert.equal(refreshed.items.length,0);assert.ok(harvests>1);
console.log('PASS 2: refreshed thumbnail URL for previously failed id is ignored; no retry');
const ctrl=new AbortController();
const final=await scanView({findScrollTarget:()=>null,harvest:()=>[tile('a')],sleep:async()=>{},hashThumb:async()=>zero},{signal:ctrl.signal,onProgress:p=>{if(p.phase==='done')ctrl.abort();}});
assert.equal(ctrl.signal.aborted,true);assert.equal(final.cancelled,false);assert.equal(final.items.length,1);
console.log('PASS 3: final done callback aborts signal but returned cancelled remains false');
const pre=new AbortController();pre.abort();let reads=0;
const cancelled=await scanView({findScrollTarget:()=>null,harvest:()=>{reads++;return [tile('a')]},sleep:async()=>{},hashThumb:async()=>{throw Error('must not hash')}},{signal:pre.signal});
assert.equal(reads,1);assert.equal(cancelled.cancelled,true);assert.equal(cancelled.found,1);assert.equal(cancelled.items.length,0);
console.log('PASS 4: pre-cancelled scan still harvests once but starts no hash work');
