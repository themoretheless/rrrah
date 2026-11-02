import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {findResult,pinRecord,replayPins,prepareRecovery,PIN_ENTRY} from './runtime/lib/protection.ts';
const pair=[
 {id:'call-entry',type:'message',message:{role:'assistant',content:[{type:'toolCall',id:'call',name:'read',arguments:{path:'/owned/a'}}],timestamp:0}},
 {id:'result-entry',type:'message',message:{role:'toolResult',toolCallId:'call',toolName:'read',content:[{type:'text',text:'owned original result'}],isError:false,timestamp:0}}
];
const original=findResult('owned-session',pair,'result-entry');assert(original);assert.match(original.ref.digest,/^[a-f0-9]{64}$/);
const pin={id:'pin-entry',type:'custom',customType:PIN_ENTRY,data:pinRecord('pin','owner',[original.ref])};
const entries=[...pair,pin];
assert(replayPins('owned-session',entries).has('call'));
const changed=structuredClone(entries);changed[1].message.content[0].text='changed original result';assert.equal(replayPins('owned-session',changed),null);
const ambiguous=[...pair,structuredClone(pair[1])];ambiguous[2].id='other-result';assert.equal(findResult('owned-session',ambiguous,'result-entry'),undefined);
const archived=prepareRecovery('owned-session',pair,[],'result-entry',1000);assert.equal(archived.kind,'archived');assert(archived.content[0].text.includes('not a user instruction'));
const overBudget=prepareRecovery('owned-session',pair,[],'result-entry',1);assert.equal(overBudget.kind,'over-budget');
const result={scope:'Complete unchanged original protection.ts with original messages/token modules and actual tokenizer0.0.4; owned session-entry fixtures, no Pi session storage or host lifecycle',authored_controls_run:5,observations:['valid paired call/result pin replays','changed original body invalidates pin digest','multiple results for one toolCallId rejected','archived text recovery labelled as reference data','small recall budget rejects whole result']};
writeFileSync(new URL('protection-results.json',import.meta.url),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
