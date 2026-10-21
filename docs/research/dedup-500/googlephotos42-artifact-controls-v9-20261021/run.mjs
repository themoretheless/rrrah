import fs from 'node:fs/promises';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {stripTypeScriptTypes} from 'node:module';
const source=await fs.readFile('.dedup-work/googlephotos42-20261021/scripts/verify.ts','utf8');
const code=stripTypeScriptTypes(source);
const table=code.match(/const RAW_CHROME_MAX\s*=\s*(\[[\s\S]*?\n\])/);
assert.ok(table);
const rows=vm.runInNewContext(table[1]);
assert.equal(rows.length,13);
const expression=code.match(/const count = (\(code\.match\(re\) \?\? \[\]\)\.length)/);
assert.ok(expression);
for(const [re,label,max] of rows){
 const call=re.source.replaceAll('\\','');
 const repeated=(call+'0);').repeat(3);
 const observed=vm.runInNewContext(expression[1],{code:repeated,re});
 const independent=[...repeated.matchAll(new RegExp(re.source,'g'))].length;
 assert.equal(independent,3,label);assert.equal(observed,1,label);assert.ok(observed<=max,label);
}
console.log('PASS: all13original artifact regex counters report1for3calls and accept max1; independent global count3');
