import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const source=readFileSync(process.argv[2],'utf8');
let handler;const messages=[];
const context=vm.createContext({Float32Array,ArrayBuffer,Map,Set,Math,setTimeout,self:{addEventListener:(type,fn)=>{assert.equal(type,'message');handler=fn},postMessage:m=>messages.push(m)}});
vm.runInContext(stripTypeScriptTypes(source.replace(/^import .*;\r?$/gm,'')),context);
async function detect(name,rows,threshold,expected,type='detect',buckets){
 messages.length=0;const dim=rows[0].length;const flat=new Float32Array(rows.length*dim);rows.forEach((r,i)=>flat.set(r,i*dim));
 await handler({data:{type,data:{flatEmbeddings:flat,n:rows.length,dim,threshold,timestamps:rows.map((_,i)=>i),buckets}}});
 const actual=JSON.parse(JSON.stringify(messages.find(m=>m.type==='detectionResults').groups));assert.deepEqual(actual,expected);console.log(JSON.stringify({name,actual,passed:true}));
}
const v=a=>new Float32Array([Math.cos(a),Math.sin(a)]);
await detect('full-worker-finds-separated-identical', [v(0),v(Math.PI/2),v(0)],0.99,[[0,2]]);
await detect('full-worker-star-not-clique',[v(0),v(.1),v(.2)],.99,[[0,1,2]]);
await detect('full-worker-exact-threshold-caps-sixty-identical-at-fifty',Array.from({length:60},()=>new Float32Array([1,0])),1,[Array.from({length:50},(_,i)=>i)]);
await detect('smart-worker-chain-not-clique',[v(0),v(.1),v(.2)],.99,[[0,1,2]],'detectSmart',[[0,1,2]]);
