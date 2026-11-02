import fs from 'node:fs';
import assert from 'node:assert/strict';
const source=fs.readFileSync('docs/research/dedup-500/evidence/0xuezhang985--wallet-convergence-alert/04-content.js','utf8');
const start=source.indexOf('  function mergeTrades(existing, incoming) {');assert(start>=0);const end=source.indexOf('\n  function ',start+1);assert(end>start);const body=source.slice(start,end).trim();
// Only the reviewed pure function;never evaluate browser initialization,network,or storage.
const merge=new Function(`${body}
;return mergeTrades;`)();
const a={wallet:'owned-wallet',token:'SAME',mint:'mint-A',chain:'sol',timeMs:100};const b={...a,mint:'mint-B'};
const collapsed=merge([a],[b]);assert.equal(collapsed.length,1);assert.equal(collapsed[0].mint,'mint-B');
const repeated=merge([{...a,signature:'sig-1'}],[{...a,signature:'sig-1'}]);assert.equal(repeated.length,1);
const capped=merge([],Array.from({length:51},(_,i)=>({...a,signature:`owned-${i}`,timeMs:i})));assert.equal(capped.length,50);assert.equal(capped.at(-1).timeMs,1);
const result={controls_passed:3,scope:'Original pure mergeTrades function only;synthetic records;no browser or service requests',findings:['Absent signature fallback merges different mints with same wallet/token/timeMs.','Repeated same signature merges as expected.','51 unique input trades intentionally truncated to newest50.'],runtime:process.version};fs.writeFileSync(new URL('results.json',import.meta.url),JSON.stringify(result,null,2)+'\n');console.log(result);
