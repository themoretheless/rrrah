import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
const source=fs.readFileSync('.dedup-work/reverse426-20261021/internal/web/app.js','utf8');
function harness() {
 const elements=new Map();
 const element=id=>{if(!elements.has(id))elements.set(id,{id,value:'',textContent:'',style:{},dataset:{},classList:{add(){},remove(){},contains(){return false}},addEventListener(){},appendChild(){},querySelectorAll(){return []}});return elements.get(id)};
 let resolve,reject;
 const pending=new Promise((a,b)=>{resolve=a;reject=b});
 const streams=[];
 const context=vm.createContext({document:{getElementById:element,querySelectorAll(){return []},addEventListener(){},createElement(){return element(Symbol())}},localStorage:{getItem(){return null},setItem(){}},FormData:class{append(){}},EventSource:class{constructor(url){this.url=url;this.closed=false;streams.push(this)}close(){this.closed=true}},fetch(){return pending},alert(){},confirm(){return false},Date,Math,JSON,parseInt,parseFloat,encodeURIComponent});
 vm.runInContext(source,context);
 vm.runInContext('selectedFile = {name:"synthetic.jpg"}; startSearch()',context);
 return {context,element,streams,resolve,reject};
}
const flush=()=>new Promise(resolve=>setImmediate(resolve));
const rows=[];
{
 const h=harness();vm.runInContext('stopSearch()',h.context);
 assert.equal(h.element('progressText').textContent,'Search complete');
 h.resolve({json:async()=>({searchId:'delayed'})});await flush();
 assert.equal(h.streams.length,1);assert.equal(h.streams[0].closed,false);
 rows.push({name:'stop_before_response',observed:'Stop shows complete, but delayed response opens a live results stream'});
}
{
 const h=harness();h.resolve({json:async()=>({searchId:'disconnect'})});await flush();h.streams[0].onerror();
 assert.equal(h.element('progressText').textContent,'Search complete');assert.equal(h.element('progressBar').style.width,'100%');
 rows.push({name:'stream_disconnect',observed:'Connection failure displayed as Search complete / 100%'});
}
{
 const h=harness();h.resolve({json:async()=>({error:'fixture failure'})});await flush();
 assert.equal(h.element('progressText').textContent,'Error: fixture failure');assert.equal(h.element('searchBtn').style.display,'none');assert.equal(h.element('stopBtn').style.display,'inline-block');
 rows.push({name:'server_rejects_search',observed:'Error text remains, search button stays hidden until Stop'});
}
{
 const h=harness();h.reject(new Error('fixture network failure'));await flush();
 assert.equal(h.element('progressText').textContent,'Search complete');
 rows.push({name:'request_rejected',observed:'Network error text immediately overwritten by Search complete'});
}
const result={repository:'dpippenger/reverse-imagesearch',commit:'617c17b544ff389d1de79a797e3ab05af3407bfe',source_sha256:crypto.createHash('sha256').update(source).digest('hex'),controls:rows,limits:['Original whole app.js evaluated with minimal DOM, deferred fetch and EventSource adapters.','No real browser, network, image decode, CSS layout or server worker exercised.','Assertions preserve observed faults rather than validate desired behavior.']};
fs.writeFileSync(new URL('result.json',import.meta.url),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(rows));
