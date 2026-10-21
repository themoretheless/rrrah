import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const original=readFileSync('.dedup-work/googlephotos42-20261021/scripts/cws-screenshots.mjs','utf8');
const source=original.replace(/^import .* from '[^']+'\n/gm,'').replaceAll('import.meta.url',"'file:///synthetic/scripts/cws-screenshots.mjs'");
async function scenario(pro){
 const events=[];let dry=false;
 const locator={waitFor:async()=>{},innerText:async()=>'',isEnabled:async()=>pro};
 const page={
  locator:()=>locator,
  setViewportSize:async()=>{},evaluate:async()=>{},
  check:async(id)=>{if(id==='#gpdt-dryrun')dry=true;events.push(['check',id]);},
  uncheck:async(id)=>{if(id==='#gpdt-dryrun')dry=false;events.push(['uncheck',id]);},
  click:async(id)=>events.push(['click',id,dry]),
  waitForFunction:async()=>{},waitForTimeout:async()=>{},screenshot:async()=>{},
  selectOption:async()=>{}
 };
 await runInNewContext('(async()=>{'+source+'})()',{
  process:{argv:['node','synthetic'],exit(c){throw Error('exit'+c)}},
  readFile:async()=> '// synthetic inert userscript',
  mkdir:async()=>{},stat:async()=>({}),
  resolve,fileURLToPath,URL,connect:async()=>({close:async()=>{}}),
  newTab:async()=>page,requireUrl:async()=>true,
  console:{log(){},error(){},warn(){}}
 });
 return events.filter(e=>e[0]==='click'&&e[1]==='#gpdt-start').map(e=>e[2]);
}
assert.deepEqual(await scenario(false),[true]);
console.log('PASS non-Pro default invocation only requests dry run');
assert.deepEqual(await scenario(true),[true,false]);
console.log('PASS counterexample: Pro default invocation requests second start with dry-run unchecked despite absent allow-destructive flag');
console.log('TOTAL2passed; original top-level script body with complete fake browser/filesystem adapters; no browser, userscript evaluation, account, clicks or deletion executed');
