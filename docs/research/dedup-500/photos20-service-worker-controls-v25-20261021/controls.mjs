import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const text=readFileSync(process.argv[2],'utf8').replace(/^import \{ APP_ID \} from "\.\.\/lib\/types"\r?$/m,'').replace(/^import type \{[\s\S]*?\} from "\.\.\/lib\/types"\r?$/m,'');
for(const order of [[0,1,2],[2,0,1]]){
 const sent=[];
 const chrome={runtime:{onMessage:{addListener(){}},getURL:p=>p},tabs:{async query(){return [{id:2,active:true}]},async sendMessage(id,m){sent.push({id,m})},onRemoved:{addListener(){}}},action:{onClicked:{addListener(){}}}};
 const ctx=vm.createContext({chrome,console:{log(){},error(){}},APP_ID:'GPD'});
 vm.runInContext(stripTypeScriptTypes(text)+'\nglobalThis.api={handleGptkCommand,handleGptkResultChunk};',ctx);
 await ctx.api.handleGptkCommand({app:'GPD',action:'gptkCommand',command:'getAllMediaItems',requestId:'r'},{tab:{id:1}});
 for(const index of order)ctx.api.handleGptkResultChunk({app:'GPD',action:'gptkResultChunk',command:'getAllMediaItems',requestId:'r',chunkIndex:index,totalChunks:3,data:[index]},{});
 const actual=sent.filter(x=>x.id===1&&x.m.action==='gptkResultChunk').map(x=>x.m.chunkIndex);
 assert.deepEqual(actual,order[0]===2?[2]:[0,1,2]);
 console.log(JSON.stringify({order,relayed:actual,passed:true}));
}
