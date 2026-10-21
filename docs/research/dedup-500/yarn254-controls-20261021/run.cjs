// Original plugin loaded unchanged; explicit shell adapter never executes commands.
const assert = require('node:assert/strict');
const plugin = require('../../../../.dedup-work/yarn254-20261021/index.js');
const originalArgv = process.argv.slice();
const originalEnv = process.env.DEDUPED;
async function run(name, env, args, returns, expected) {
  if (env === undefined) delete process.env.DEDUPED;
  else process.env.DEDUPED = env;
  process.argv = ['node', 'control', ...args];
  const calls = [];
  const hook = plugin.factory(name => {
    assert.equal(name, '@yarnpkg/shell');
    return {execute: async command => {calls.push(command); return returns.shift();}};
  }).hooks.afterAllInstalled;
  await hook();
  assert.deepEqual(calls, expected);
  console.log('PASS', name, JSON.stringify(calls));
  return hook;
}
(async () => {
  try {
    await run('no duplicates', undefined, [], [0], ['yarn dedupe --check']);
    await run('duplicates', undefined, [], [1, 0], ['yarn dedupe --check', 'yarn dedupe']);
    await run('check generic failure also invokes dedupe', undefined, [], [2, 0], ['yarn dedupe --check', 'yarn dedupe']);
    await run('dedupe failure return ignored', undefined, [], [1, 2], ['yarn dedupe --check', 'yarn dedupe']);
    await run('environment guard', 'yes', [], [], []);
    await run('argv guard', undefined, ['dedupe'], [], []);
  } finally {
    process.argv = originalArgv;
    if (originalEnv === undefined) delete process.env.DEDUPED;
    else process.env.DEDUPED = originalEnv;
  }
})().catch(error => {console.error(error); process.exitCode=1;});
