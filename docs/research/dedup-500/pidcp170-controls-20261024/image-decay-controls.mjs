import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {applySizeAgeDecay} from './runtime/lib/strategies/size-age-decay.ts';
import {createSessionState} from './runtime/lib/state.ts';
import {lenientConfig} from './runtime/test/_helpers.ts';
const image={type:'image',data:'AAAA',mimeType:'image/png'};
const body=Array.from({length:400},(_,i)=>'owned line '+i+' '+ 'x'.repeat(80)).join('\n');
const config=lenientConfig();
function aged(content){return [{role:'toolResult',toolCallId:'mixed',toolName:'read',content,isError:false,timestamp:0},...Array.from({length:12},()=>({role:'assistant',content:[{type:'text',text:'later owned step'}],timestamp:0}))];}
const mixed=aged([{type:'text',text:body},image]);
const first=applySizeAgeDecay(mixed,config,createSessionState());assert.equal(first.decayedCount,1);assert.equal(mixed[0].content.length,1);assert.equal(mixed[0].content[0].type,'text');assert(!mixed[0].content[0].text.includes('[image omitted]'));
const imageOnly=aged(Array.from({length:8},()=>({...image})));
const second=applySizeAgeDecay(imageOnly,config,createSessionState());assert.equal(second.decayedCount,0);assert.equal(imageOnly[0].content.length,8);
const result={scope:'Original unchanged size-age-decay, messages and config modules; owned text and synthetic image blocks only. No image decoder, schema validation, provider, recall or Pi host execution.',authored_controls_run:2,observations:[{case:'mixed long text and image becomes text-only excerpt without explicit image omission note',decayed:first.decayedCount,remainingBlocks:mixed[0].content.length},{case:'image-only output remains unchanged',decayed:second.decayedCount,remainingBlocks:imageOnly[0].content.length}]};
writeFileSync(new URL('image-decay-results.json',import.meta.url),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
