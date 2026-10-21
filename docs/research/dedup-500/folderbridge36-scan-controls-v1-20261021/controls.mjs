import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import assert from 'node:assert/strict';
const root = process.argv[2];
const url = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const filterSource = readFileSync(`${root}/src/mountFileFilter.ts`, 'utf8').replace(/^import .*\n/gm, '');
const filterUrl = url(stripTypeScriptTypes(filterSource));
const source = readFileSync(`${root}/src/mountScan.ts`, 'utf8').replace(/^import .*\n/gm, '');
const scanner = await import(url(`import {isVisibleFileInMount} from '${filterUrl}';const normalizePath = x => x;\n${stripTypeScriptTypes(source)}`));
const mount = {virtualPath: 'mount', enabled: true};
const files = n => Array.from({length:n}, (_, i) => `mount/${i}.md`);
async function run(n, fail = false) {
 let active = 0, peak = 0, errors = 0;
 const created = [];
 const result = await scanner.replayMountContentsToVault(mount, {
  list: async () => ({folders: [], files: files(n)}),
  stat: async file => { active++; peak = Math.max(peak, active); await Promise.resolve(); active--; if (fail && file === 'mount/0.md') throw new Error('synthetic stat failure'); return null; },
  hasAbstractFile: () => false, isIgnored: () => false,
  onFolderCreated: async () => {}, onFileCreated: async file => {created.push(file);},
  onError: () => {errors++;}, yieldToEventLoop: async () => {},
 });
 return {result, peak, errors, created};
}
let a = await run(24);
assert.equal(a.peak, 8); assert.deepEqual(a.created, files(24));
console.log(JSON.stringify({name:'local-concurrency8-ordered-notification', passed:true, peak:a.peak}));
a = await run(16, true);
assert.equal(a.errors,1); assert.equal(a.result.fileCount,0); assert.equal(a.result.scanLimitHit,false); assert.deepEqual(a.created,[]);
console.log(JSON.stringify({name:'first-stat-failure-abandons-folder-returning-partial-counts', passed:true, result:a.result}));
a = await run(1001);
assert.equal(a.result.fileCount,1001); assert.equal(a.result.isHuge,false);
console.log(JSON.stringify({name:'flat-1001-file-mount-not-marked-huge', passed:true, result:a.result}));
