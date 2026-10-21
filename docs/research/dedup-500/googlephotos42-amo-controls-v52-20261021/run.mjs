import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const original=readFileSync('.dedup-work/googlephotos42-20261021/scripts/amo-create.mjs','utf8');
const source=original.replace(/^import .* from '[^']+'\n/gm,'').replaceAll('import.meta.url',"'file:///synthetic/scripts/amo-create.mjs'").replace('main().catch(', 'await main().catch(');
async function scenario(dry, responses){
 const logs=[],calls=[];let exit=0;
 const listing={amo:{categories:['photos-music-videos'],summary:'synthetic',license:'MIT',description:['synthetic description']}};
 await runInNewContext('(async()=>{'+source+'})()',{
  process:{argv:['node','synthetic','--zip','synthetic.zip',...(dry?['--dry-run']:[])],env:{FIREFOX_JWT_ISSUER:'synthetic',FIREFOX_JWT_SECRET:'synthetic'},exit(c){exit=c;throw Error('exit'+c)}},
  readFile:async(p)=>p.endsWith('listing.json')?JSON.stringify(listing):new Uint8Array([0]),
  resolve,fileURLToPath,URL,FormData,Blob,AbortSignal,
  signAmoJwt:()=> 'synthetic-token',
  fetch:async(url,options)=>{calls.push([url,options.method]);assert.ok(responses.length);const data=responses.shift();return {ok:true,status:200,text:async()=>JSON.stringify(data)};},
  console:{log(...x){logs.push(x.join(' '))},error(...x){logs.push(x.join(' '))}},
  setTimeout(fn){fn()}
 });
 return {logs,calls,exit};
}
const dry=await scenario(true,[]);assert.equal(dry.calls.length,0);assert.equal(dry.exit,0);
console.log('PASS dry-run makes no API calls with synthetic bytes');
const wrong=await scenario(false,[{uuid:'u'},{valid:true},{id:123,slug:'expected'},{},{slug:'different-addon',current_version:{version:'0.0.1'}}]);
assert.equal(wrong.calls.length,5);assert.equal(wrong.exit,0);assert.ok(wrong.logs.some(x=>x.includes('LIVE listing:')&&x.includes('different-addon')));
console.log('PASS mismatched readback slug/version accepted and logged LIVE without identity/version comparison');
const missing=await scenario(false,[{uuid:'u'},{valid:true},{id:123,slug:'expected'},{},{slug:'expected'}]);
assert.equal(missing.exit,0);assert.ok(missing.logs.some(x=>x.includes('version undefined submitted')));
console.log('PASS missing current_version accepted and logged submitted');
console.log('TOTAL3passed; original script with fully synthetic filesystem/API/signing adapters; no account, credentials, actual signing or network');
