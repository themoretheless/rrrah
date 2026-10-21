import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const source=readFileSync(process.argv[2],'utf8');
for(const [name,pages,args,expected] of [
 ['date-range-not-applied',[{items:[{mediaKey:'outside',creationTimestamp:100,timestamp:100}]}],{dateRange:{from:'2025-01-01',to:'2025-12-31'}},['outside']],
 ['new-key-at-watermark-excluded',[{items:[{mediaKey:'new-same-time',creationTimestamp:100}]}],{sinceTimestamp:100},[]],
 ['null-page-after-first-page-reported-as-success',[{items:[{mediaKey:'first',creationTimestamp:200}],nextPageId:'p2'},null],{},['first']]
]){
 const messages=[];let calls=0;
 const window={location:{pathname:'/u/2/'},addEventListener(){},postMessage:m=>messages.push(m),gptkApi:{async getItemsByUploadedDate(){return pages[calls++]}}};
 const ctx=vm.createContext({window,setTimeout,clearTimeout,console:{log(){},warn(){},error(){}}});
 vm.runInContext(source+'\nglobalThis.readLibrary=getAllMediaItems;',ctx);
 await ctx.readLibrary('r',args);
 const actual=messages.filter(m=>m.action==='gptkResultChunk').flatMap(m=>m.data).map(i=>i.mediaKey);
 assert.deepEqual(actual,expected);assert.ok(!messages.some(m=>m.success===false));
 console.log(JSON.stringify({name,actual,apiCalls:calls,passed:true}));
}
