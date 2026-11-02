import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {applyDeduplication} from './runtime/lib/strategies/deduplication.ts';
import {applyOverlapDedup} from './runtime/lib/strategies/overlap-dedup.ts';
import {createSessionState} from './runtime/lib/state.ts';
import {lenientConfig} from './runtime/test/_helpers.ts';
const call=(id,args)=>({role:'assistant',content:[{type:'toolCall',id,name:'read',arguments:args}],timestamp:0});
const result=(id,text,isError=false)=>({role:'toolResult',toolCallId:id,toolName:'read',content:[{type:'text',text}],isError,timestamp:0});
const config=lenientConfig();
const observations=[];
{
 const messages=[call('old',{path:'/owned/a'}),result('old','historical version A'),call('new',{path:'/owned/a'}),result('new','changed version B')];
 const observed=applyDeduplication(messages,config,createSessionState());
 assert.equal(observed.prunedCount,1);assert(messages[1].content[0].text.startsWith('[pruned by pi-dcp:'));assert.equal(messages[3].content[0].text,'changed version B');
 observations.push({case:'identical arguments with differing outputs prunes historical output',observed,oldAfter:messages[1].content[0].text});
}
{
 const messages=[call('good',{path:'/owned/a'}),result('good','valid old data'),call('error',{path:'/owned/a'}),result('error','owned read failure',true)];
 const observed=applyDeduplication(messages,config,createSessionState());
 assert.equal(observed.prunedCount,1);assert(messages[1].content[0].text.startsWith('[pruned by pi-dcp:'));assert.equal(messages[3].isError,true);
 observations.push({case:'latest errored call supersedes earlier successful exact-signature result',observed});
}
{
 const messages=[call('slice',{path:'/owned/a',offset:2,limit:1}),result('slice','historical middle line'),call('whole',{path:'/owned/a'}),result('whole','changed whole-file output')];
 const observed=applyOverlapDedup(messages,config,createSessionState());
 assert.equal(observed.prunedCount,1);assert(messages[1].content[0].text.startsWith('[pruned by pi-dcp:'));
 observations.push({case:'requested range containment removes differing historical content',observed});
}
{
 const messages=[call('a',{path:'/owned/a'}),result('a','OK'),call('b',{path:'/owned/a'}),result('b','OK')];
 const before=messages[1].content[0].text.length;
 const observed=applyDeduplication(messages,config,createSessionState());
 const after=messages[1].content[0].text.length;
 assert(after>before);assert.equal(observed.tokensSaved,0);
 observations.push({case:'short duplicate expands placeholder while savings floor hides growth',beforeChars:before,afterChars:after,observed});
}
const payload={scope:'Complete original strategy/messages/config/state/token modules and original test helper; real tokenizer0.0.4 dependency installed, though these paths use original character-based estimates. No config loading, host pipeline, real files, providers or shell actions.',authored_controls_run:observations.length,observations};
writeFileSync(new URL('strategy-results.json',import.meta.url),JSON.stringify(payload,null,2)+'\n');
console.log(JSON.stringify(payload,null,2));
