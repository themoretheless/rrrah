import fs from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
const source=await fs.readFile('.dedup-work/googlephotos42-20261021/src/core/page-runner.ts','utf8');
const stripped=stripTypeScriptTypes(source);
const start=stripped.indexOf('export class PageRunner');assert.ok(start>=0);
const originalClass=stripped.slice(start).replace('export class PageRunner','class PageRunner');
let created=0;let pending=[];
class FakeEngine {
 isStopped=false;isPaused=false;log={ratePerMinute:()=>0};
 constructor(){created++}
 run(){return new Promise(resolve=>pending.push(()=>resolve({status:'done',deleted:0,selected:0,startedAt:1})))}
 getDryRunLabels(){return ['Photo - Mar 1, 2024']}getDateReport(){return null}
}
const context={DeleteEngine:FakeEngine,admitConcurrentStart:occupied=>({ok:!occupied}),admitDestructiveRun:()=>({ok:true}),readLocalAcknowledgement:()=>({readable:true,acknowledged:true}),CONSENT_KEY:'c',EMPTY_TRASH_ACK_KEY:'e',filterRequiresPro:f=>f.kind==='date',shouldNavigateToEmptyTrash:()=>false,classifyLabel:()=> 'unknown',console,Set,Date,RunInProgressError:class extends Error{},ProRequiredError:class extends Error{}};
const Runner=vm.runInNewContext(originalClass+';PageRunner',context);
const make=()=>new Runner({dom:{},baton:{},runEmptyTrash:async()=>{throw Error('unexpected')}});
const options={maxCount:5,dryRun:true,emptyTrashAfter:false,filter:{kind:'date',range:{mode:'before',date:'2020-01-01'}}};
const r=make();let allow=[];r.isPro=()=>new Promise(resolve=>allow.push(resolve));
const a=r.start(options),b=r.start(options);assert.equal(allow.length,2);allow.forEach(resolve=>resolve(true));
await new Promise(resolve=>setImmediate(resolve));assert.equal(created,2);assert.equal(pending.length,2);
pending.splice(0).forEach(done=>done());await Promise.all([a,b]);
console.log('PASS 1: two concurrent date starts pass async license boundary and instantiate two fake engines');
let blobTouched=0;context.Blob=class{constructor(){blobTouched++}};
r.exportCsv();assert.equal(blobTouched,0);assert.equal(r.getSummary().total,1);
console.log('PASS 2: after settlement summary exists, engine cleared, exportCsv returns before Blob');
const broken=make();let calls=0;broken.onUpdate(()=>{if(++calls>1)throw Error('listener')});
await assert.rejects(broken.start({...options,filter:{kind:'all'}}),/listener/);
assert.equal(broken.getStatus().running,true);
console.log('PASS 3: throwing status listener before try/finally leaves running=true');
