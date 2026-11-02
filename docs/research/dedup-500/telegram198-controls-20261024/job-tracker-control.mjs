import fs from 'node:fs';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
const source='docs/research/dedup-500/evidence/botnick--telegram-media-downloader/remaining-pinned/src/core/job-tracker.js';
const raw=fs.readFileSync(source);
const {createJobTracker,flattenStatus}=await import('data:text/javascript;base64,'+raw.toString('base64'));
const tick=()=>new Promise(r=>setImmediate(r));
const cases=[];
{
 const t=createJobTracker({kind:'control'});let finish;let sig;
 t.tryStart(async ({signal,onProgress})=>{sig=signal;onProgress({running:true,processed:1,total:3});await new Promise(r=>finish=r);return {processed:1};});
 assert.equal(t.tryStart(()=>{}).code,'ALREADY_RUNNING');assert.equal(t.cancel(),true);assert.equal(sig.aborted,true);assert.equal(t.isRunning(),true);finish();await tick();
 const s=t.getStatus();assert.equal(s.stage,'done');assert.equal(s.successes,1);assert.equal(s.error,null);assert.equal(Object.hasOwn(s,'cancelled'),false);assert.equal(flattenStatus(s).running,false);
 cases.push({name:'cooperative cancel returning normally counts success; single flight held until settle',passed:true});
}
{
 const t=createJobTracker({kind:'control'});t.tryStart(async()=>({old:1}));await tick();t.tryStart(async()=>{throw Error('injected');});await tick();const s=t.getStatus();assert.equal(s.stage,'error');assert.equal(s.error,'injected');assert.deepEqual(s.result,{old:1});assert.equal(s.failures,1);assert.equal(t.tryStart(async()=>({new:2})).started,true);await tick();assert.deepEqual(t.getStatus().result,{new:2});
 cases.push({name:'error retains previous result; next run succeeds',passed:true});
}
{
 const t=createJobTracker({kind:'control'});let late;
 t.tryStart(async({onProgress})=>{late=onProgress;return {};});await tick();let release;t.tryStart(async()=>{await new Promise(r=>release=r);return {};});late({processed:999,stage:'old-run'});assert.equal(t.getStatus().progress.processed,999);release();await tick();
 cases.push({name:'retained old progress callback can update a later running job',passed:true,scope:'authored late-callback fixture; actual caller scheduling not established'});
}
const result={source,sha256:crypto.createHash('sha256').update(raw).digest('hex'),cases,passed:cases.length,scope:'Actual unchanged dependency-free module; owned synthetic workers, no app/DB/server.'};
fs.writeFileSync('docs/research/dedup-500/telegram198-controls-20261024/job-tracker-control-results.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
