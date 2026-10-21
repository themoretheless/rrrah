import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const root=path.resolve('.dedup-work/googlephotos42-20261021');
const out=path.join(root,'canary-runtime');await fs.mkdir(out,{recursive:true});
const modules={'selector-pack':'src/core/selector-pack.ts',diagnostics:'src/core/diagnostics.ts',selectors:'src/core/selectors.ts',phash:'src/core/dedup/phash.ts','browser-grid':'src/core/dedup/browser-grid.ts',evaluate:'scripts/canary/evaluate.ts'};
await fs.copyFile(path.join(root,'src/selector-packs/pack-v1.json'),path.join(out,'pack.json'));
for(const [name,file] of Object.entries(modules)) {
 let code=stripTypeScriptTypes(await fs.readFile(path.join(root,file),'utf8'));
 code=code.replace(/from ['"](?:\.\.\/|\.\/)+(selector-pack|selectors|diagnostics|phash)['"]/g,(_,dep)=>"from './"+dep+".mjs'");
 code=code.replace("from '../../src/core/selector-pack'","from './selector-pack.mjs'");
 code=code.replace("from '../selector-packs/pack-v1.json'","from './pack.json' with { type: 'json' }");
 await fs.writeFile(path.join(out,name+'.mjs'),code);
}
const {mediaIdFromHref}=await import(pathToFileURL(path.join(out,'browser-grid.mjs')));
const {buildChecks,evaluateDrift}=await import(pathToFileURL(path.join(out,'evaluate.mjs')));
const html=await fs.readFile(path.join(root,'tests/fixtures/canary-grid-healthy.html'),'utf8');
const hrefs=[...html.matchAll(/href="([^"\n]+)"/g)].map(m=>m[1]);
assert.equal(hrefs.length,2);assert.deepEqual(hrefs.map(mediaIdFromHref),[null,null]);
console.log('PASS 1: both media links in healthy canary fixture rejected by original id parser');
const observations=Object.fromEntries(buildChecks().filter(x=>!x.skipReason).map(x=>[x.id,{primary:1,fallbacks:[]} ]));
const summary=evaluateDrift(observations);assert.equal(summary.verdict,'ok');assert.equal(summary.checks.find(x=>x.id==='dialog').verdict,'skipped');
console.log('PASS 2: count-only synthetic observations pass with dialog skipped; no usable-id validation');
observations.mediaLink={primary:0,fallbacks:[1]};assert.equal(evaluateDrift(observations).verdict,'ok');
console.log('PASS 3: fallback-only media match passes; missing photo candidates cannot be inferred absent video fallback');
