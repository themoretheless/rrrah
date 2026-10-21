import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import assert from 'node:assert/strict';
const scratch = mkdtempSync(join(tmpdir(), 'gpdt-prompt-'));
try {
  for (const name of ['photo-filter', 'pro-moments', 'post-run-prompt', 'selectors', 'selector-pack', 'diagnostics']) {
    const source = readFileSync(resolve('.dedup-work/googlephotos42-20261021/src/core', name + '.ts'), 'utf8');
    writeFileSync(join(scratch, name + '.mjs'), stripTypeScriptTypes(source).replace(/from '(\.\/[^']+)'/g, "from '$1.mjs'").replace("from '../selector-packs/pack-v1.json'", "from './pack-v1.json' with { type: 'json' }"));
  }
  writeFileSync(join(scratch, 'pack-v1.json'), readFileSync('.dedup-work/googlephotos42-20261021/src/selector-packs/pack-v1.json'));
  const { claimPostRunPrompt } = await import(pathToFileURL(join(scratch, 'post-run-prompt.mjs')));
  const result = { dryRun: false, stopped: false, status: 'done', deleted: 1, filterKind: 'ids' };
  let shown = false, writes = 0;
  const storage = { isShown: async () => shown, markShown: async () => { shown = true; writes++; } };
  const pair = await Promise.all([claimPostRunPrompt(result, storage, 'chrome', false), claimPostRunPrompt(result, storage, 'chrome', false)]);
  assert.equal(pair.filter(Boolean).length, 2); assert.equal(writes, 2);
  console.log('PASS concurrent claims both return prompts and write flag (counterexample to once-per-install under concurrency)');
  assert.equal(await claimPostRunPrompt(result, storage, 'chrome', false), null);
  console.log('PASS subsequent sequential claim is suppressed');
  const broken = { isShown: async () => false, markShown: async () => { throw Error('write failure'); } };
  assert.equal(await claimPostRunPrompt(result, broken, 'chrome', false), null);
  console.log('PASS write failure suppresses prompt');
} finally { rmSync(scratch, { recursive: true, force: true }); }
