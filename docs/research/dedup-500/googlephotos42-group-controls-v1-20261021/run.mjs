// Read-only source qualification: strip TypeScript types, change one local
// import suffix, then call original pure grouping functions on synthetic hashes.
import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { stripTypeScriptTypes } from 'node:module';
import { pathToFileURL } from 'node:url';

const root = process.cwd();
const source = path.join(root, '.dedup-work/googlephotos42-20261021/src/core/dedup');
const runtime = path.join(root, '.dedup-work/googlephotos42-20261021/controls-runtime');
await fs.mkdir(runtime, { recursive: true });
for (const name of ['phash', 'group']) {
  let code = stripTypeScriptTypes(await fs.readFile(path.join(source, `${name}.ts`), 'utf8'));
  if (name === 'group') {
    assert.equal(code.split("from './phash'").length - 1, 1);
    code = code.replace("from './phash'", "from './phash.mjs'");
  }
  await fs.writeFile(path.join(runtime, `${name}.mjs`), code);
}
const { groupDuplicates, GroupingAborted } = await import(pathToFileURL(path.join(runtime, 'group.mjs')));
const { hammingDistance } = await import(pathToFileURL(path.join(runtime, 'phash.mjs')));
const opts = { now: () => 0, sliceMs: 1e9 };
const item = (id, lo, hi = 0) => ({ id, hash: { lo, hi } });

const allBits = [item('zero', 0), item('complement', 0xffffffff, 0xffffffff)];
assert.equal(hammingDistance(allBits[0].hash, allBits[1].hash), 64);
const universal = await groupDuplicates(allBits, { ...opts, threshold: 0 });
assert.deepEqual(universal, []);
console.log('PASS 1: threshold0 permits distance64, but complementary pair omitted by64band index');

const a = item('a', 0), b = item('b', 1), c = item('c', 2);
const first = await groupDuplicates([a, b, c], { ...opts, threshold: 63 / 64 });
const reordered = await groupDuplicates([b, a, c], { ...opts, threshold: 63 / 64 });
assert.deepEqual(first.map(g => g.itemIds), [['a', 'b', 'c']]);
assert.deepEqual(reordered.map(g => g.itemIds), [['b', 'a']]);
assert.equal(hammingDistance(b.hash, c.hash), 2);
console.log('PASS 2: seed grouping joins mutually out-of-threshold members and changes with input order');

let reads = 0;
const preCancelled = new AbortController();
preCancelled.abort();
const monitored = [0, 1].map(i => ({ id: String(i), get hash() { reads++; return { lo: 0, hi: 0 }; } }));
await assert.rejects(groupDuplicates(monitored, { ...opts, signal: preCancelled.signal }), GroupingAborted);
assert.equal(reads, 8);
console.log('PASS 3: pre-aborted operation reads all2items across4bands before cancellation refusal');

const late = new AbortController();
const final = await groupDuplicates([a, item('copy', 0)], {
  ...opts, signal: late.signal,
  onProgress: (done, total) => { if (done === total) late.abort(); },
});
assert.equal(late.signal.aborted, true);
assert.deepEqual(final.map(g => g.itemIds), [['a', 'copy']]);
console.log('PASS 4: cancellation in final progress callback still publishes a group');
