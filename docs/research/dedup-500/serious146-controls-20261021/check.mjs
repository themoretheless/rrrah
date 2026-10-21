import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
const root=path.resolve('.dedup-work/serious146-20261021');
const {default:dedupe}=await import(pathToFileURL(path.join(root,'index.ts')));
let passed=0;
globalThis.describe=(_name,fn)=>fn();
globalThis.it=(_name,fn)=>{fn();passed++;};
globalThis.expect=value=>({toEqual:expected=>assert.deepStrictEqual(value,expected)});
// Only resolve the extensionless test import to original index.ts; test bodies unchanged.
const tests=fs.readFileSync(path.join(root,'test.ts'),'utf8').replace("import dedupe from './index'",`import dedupe from ${JSON.stringify(pathToFileURL(path.join(root,'index.ts')).href)}`);
const adapted=path.join(root,'review-test-adapter.mjs');fs.writeFileSync(adapted,tests);await import(pathToFileURL(adapted));assert.equal(passed,7);
const inherited=dedupe(['constructor','toString','__proto__','safe'],v=>v);assert.deepEqual(inherited,['safe']);
const nullFamily=dedupe([null,NaN,Infinity,-Infinity]);assert.equal(nullFamily.length,1);
const undefinedFamily=dedupe([undefined,()=>0,Symbol('x')]);assert.equal(undefinedFamily.length,1);
const orderedObjects=dedupe([{a:1,b:2},{b:2,a:1}]);assert.equal(orderedObjects.length,2);
let cyclic,largeInteger;const x={};x.self=x;
try{dedupe([x]);}catch(e){cyclic=e.name;}
try{dedupe([1n]);}catch(e){largeInteger=e.name;}
assert.equal(cyclic,'TypeError');assert.equal(largeInteger,'TypeError');
const sha=p=>crypto.createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex');
const result={method:'Original index.ts executed unchanged with Node experimental type stripping. Seven original test bodies executed with tiny describe/it/expect adapter and extension-only import resolution; original Jest/build/package outputs not qualified.',node:process.version,source_sha256:sha('index.ts'),test_sha256:sha('test.ts'),original_test_bodies_passed:passed,identity_custom_hasher:{input:['constructor','toString','__proto__','safe'],output:inherited},default_json_equivalence:{null_nan_infinities_survivors:nullFamily.length,undefined_function_symbol_survivors:undefinedFamily.length,reordered_object_survivors:orderedObjects.length},unsupported_inputs:{cyclic,largeInteger},limits:'Array-key equivalence controls, not file identity or image retrieval; JSON normalization limits are not claimed violations of documented string-based equivalence.'};
fs.writeFileSync('docs/research/dedup-500/serious146-controls-20261021/result.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
