// Original whole results IIFE, unchanged; minimal DOM/OC/fetch boundary adapters.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const source = fs.readFileSync(process.argv[2], 'utf8');
class Element {
  constructor(tag) { this.tag=tag; this.children=[]; this.attributes={}; this.style={}; this.handlers={}; }
  appendChild(child) { this.children.push(child); }
  setAttribute(key,value) { this.attributes[key]=value; }
  addEventListener(key,fn) { this.handlers[key]=fn; }
  removeEventListener(key) { delete this.handlers[key]; }
}
const settle = () => new Promise(resolve => setImmediate(resolve));
function group(id,size,path='/admin/files/a') {
 return {id,files:[1,2].map(n=>({id:id*10+n,path:path+n,size,mimetype:'text/plain',nodeId:id*10+n,fileHash:'authored'}))};
}
async function run(pages) {
 const nodes=Object.fromEntries(['container','loader-container','loader-btn','title'].map(id=>[id,new Element('div')]));
 const errors=[]; const requests=[];
 const context={document:{getElementById:id=>nodes[id]||null,createElement:tag=>new Element(tag)},URLSearchParams,
  OC:{generateUrl:x=>x,Util:{humanFileSize:x=>String(x)},MimeType:{getIconUrl:()=>'/icon'},dirname:x=>x},
  console:{error:(...args)=>errors.push(args)},fetch:async url=>{requests.push(url);const page=pages.shift();if(page instanceof Error)throw page;return {json:async()=>page};}};
 vm.createContext(context);vm.runInContext(source,context,{filename:process.argv[2]});await settle();
 return {nodes,errors,requests};
}
function page(groups,last=true,key=0) { return {data:{entities:groups,isLastFetched:last,pageKey:key}}; }
(async()=>{
 const sorted=await run([page([group(1,10),group(2,30),group(3,20)])]);
 const ids=sorted.nodes.container.children.map(x=>x.attributes.id);
 assert.deepEqual(ids,['grp-1','grp-2','grp-3']);assert.notDeepEqual(ids,['grp-2','grp-3','grp-1']);
 const failed=await run([page([group(1,10)],false,1),new Error('authored page2 failure')]);
 assert.equal(failed.nodes['loader-btn'].style.display,'inherit');
 await failed.nodes['loader-btn'].handlers.click();await settle();
 assert.equal(failed.nodes['loader-btn'].style.display,'none');assert.equal(failed.errors.length,1);
 assert(failed.nodes.container.children.some(x=>x.innerHTML==='Failed to load duplicates'));
 assert.equal(failed.requests[1],'/apps/duplicatefinder/api/v1/Duplicates?offset=1');
 const malformed=await run([page([group(1,10,'/unexpected/path')])]);
 assert.equal(malformed.errors.length,1);assert.equal(malformed.nodes.container.children.length,1);
 assert.equal(malformed.nodes.container.children[0].innerHTML,'Failed to load duplicates');
 console.log(JSON.stringify({source_sha256:crypto.createHash('sha256').update(source).digest('hex'),controls:3,passed:3,sort_group_ids:ids,page2_failure_hides_retry:true,unexpected_path_aborts_render:true,scope:'Unchanged full original results script under native Node VM with authored responses and minimal DOM/OC adapters. No real browser layout, Nextcloud server, file client or deletion.'},null,2));
})().catch(e=>{console.error(e);process.exitCode=1;});
