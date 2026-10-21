import './loader.mjs';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,stat,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const rootURL=pathToFileURL(path.resolve('.dedup-work/dupehunter295-20261021')+'/').href;
const {findDuplicates}=await import(rootURL+'src/dedupe.ts');
const {planActions,deleteDuplicates,hardlinkDuplicates}=await import(rootURL+'src/actions.ts');
const {runWithConcurrencyLimit}=await import(rootURL+'src/concurrency.ts');
for(const limit of [NaN]){
 let calls=0;const result=await runWithConcurrencyLimit([1,2],limit,async x=>{calls++;return x;});
 assert.equal(calls,0);assert.equal(result.length,2);assert.equal(Object.keys(result).length,0);
 console.log('PASS invalid concurrency silently returns holes without work',String(limit));
}
for(const mode of ['delete','hardlink']){
 const root=await mkdtemp(path.join(tmpdir(),'rrrah-dupehunter295-actions-'));
 try{
 const a=path.join(root,'a'),b=path.join(root,'b');await writeFile(a,'same');await writeFile(b,'same');
 const report=await findDuplicates({rootDir:root,minSizeBytes:0,followSymlinks:false,excludeGlobs:[],concurrency:1});assert.equal(report.groups.length,1);
 const actions=planActions(report.groups,'first');await writeFile(b,'DIFF');
 const outcomes=await (mode==='delete'?deleteDuplicates:hardlinkDuplicates)(actions);assert.ok(outcomes.every(x=>!x.error));
 if(mode==='delete')await assert.rejects(stat(b),{code:'ENOENT'});
 else {assert.equal(await readFile(b,'utf8'),'same');assert.equal((await stat(a)).ino,(await stat(b)).ino);}
 console.log('PASS original scan/action accepts same-length post-scan rewrite',mode);
 }finally{await rm(root,{recursive:true,force:true});}
}
