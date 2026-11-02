// Execute unchanged pinned readRange helpers only; no shell commands execute.
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { stripTypeScriptTypes } from 'node:module';
import assert from 'node:assert/strict';
const source = new URL('../evidence/Davidcreador--pi-dcp/03-overlap-dedup.ts', import.meta.url);
const original = readFileSync(source, 'utf8');
// Original contiguous declarations and helpers; external message/config imports
// and applyOverlapDedup body deliberately excluded, not replaced by mock logic.
const section = original.slice(original.indexOf('export interface OverlapDedupResult'), original.indexOf('export function applyOverlapDedup'));
assert(section.includes('export function readRange'));
const javascript = stripTypeScriptTypes('import {basename,resolve} from "node:path";\n'+section);
const {readRange} = await import('data:text/javascript;base64,'+Buffer.from(javascript).toString('base64'));
const observations=[];
function record(name, input, check) {
  const actual=readRange(input);
  assert(check(actual),name);
  observations.push({name,input,actual: actual ? {...actual,start:String(actual.start),end:String(actual.end)} : null});
}
record('ordinary bounded read', {name:'read',arguments:{path:'/owned/file.txt',offset:5,limit:3}}, r=>r.path==='/owned/file.txt' && r.start===5 && r.end===7);
record('pipeline rejected', {name:'bash',arguments:{command:'cat /owned/file.txt | sort'}}, r=>r===undefined);
record('shell variable retained as literal path', {name:'bash',arguments:{command:'cat $HOME/owned.txt'}}, r=>r.path===resolve('$HOME/owned.txt'));
record('cat option misclassified as file', {name:'bash',arguments:{command:'cat --help'}}, r=>r.path===resolve('--help'));
record('nonpositive read limit accepted as reversed interval', {name:'read',arguments:{path:'/owned/file.txt',offset:5,limit:0}}, r=>r.start===5 && r.end===4);
record('NaN offset accepted', {name:'read',arguments:{path:'/owned/file.txt',offset:NaN,limit:3}}, r=>Number.isNaN(r.start) && Number.isNaN(r.end));
const result={scope:'Original contiguous readRange and helper declarations, native Node type stripping and node:path; no strategy pipeline, message handling, shell, file reads requested by tool calls, or original upstream tests executed',source_sha256:createHash('sha256').update(original).digest('hex'),authored_controls_run:observations.length,observations};
writeFileSync(new URL('read-range-results.json',import.meta.url),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
