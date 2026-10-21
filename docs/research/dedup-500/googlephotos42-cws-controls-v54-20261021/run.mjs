import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
const {publishV2}=await import(pathToFileURL(resolve('.dedup-work/googlephotos42-20261021/scripts/cws-v2.mjs')));
async function flow(after, staged=false){
 const queue=[{}, {uploadState:'SUCCEEDED',crxVersion:'3.6.1'}, {}, after];let calls=0;
 const result=await publishV2({token:'synthetic',publisherId:'P',itemId:'E',zip:Buffer.from('synthetic'),...(staged?{publishType:'STAGED_PUBLISH'}:{}),sleep:async()=>{},fetchImpl:async()=>{calls++;assert.ok(queue.length);return new Response(JSON.stringify(queue.shift()));}});
 assert.equal(calls,4);return result;
}
const wrong={submittedItemRevisionStatus:{state:'PENDING_REVIEW',distributionChannels:[{crxVersion:'0.0.1'}]}};
assert.equal((await flow(wrong)).outcome,'published');
console.log('PASS normal publish accepts submitted state with different readback version');
assert.equal((await flow(wrong,true)).outcome,'published');
console.log('PASS staged publish also accepts mismatched submitted version');
await assert.rejects(()=>flow({submittedItemRevisionStatus:{state:'REJECTED'}}),/readback/);
console.log('PASS rejected readback state refused');
let calls=0;
const blocked=await publishV2({token:'synthetic',publisherId:'P',itemId:'E',zip:Buffer.from('synthetic'),fetchImpl:async()=>{calls++;return new Response(JSON.stringify({submittedItemRevisionStatus:{state:'PENDING_REVIEW'}}));}});
assert.equal(blocked.outcome,'blocked');assert.equal(calls,1);
console.log('PASS existing pending review blocks before upload');
console.log('TOTAL4passed; unmodified original export with fake fetch only; no authentication/signing/network/account action');
