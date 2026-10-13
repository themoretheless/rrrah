// Original module in a VM with explicit dependency adapters; no Sharp execution.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const doc = path.join(root, 'docs/research/dedup-500');
const manifest = path.join(doc, 'perceptual-hashes-benchmark-full-source-20261012.json');
const metadata = JSON.parse(fs.readFileSync(manifest));
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
for (const row of metadata.files) assert.equal(sha(path.join(metadata.source, row.path)), row.sha256);
const source = path.join(metadata.source, 'index.js');
const output = path.join(doc, 'perceptual-hashes-benchmark-runtime-controls-20261012.json');
assert(!fs.existsSync(output));
const caught = [];
const chain = {resize() {return this;}, grayscale() {return this;},
  then(callback) {return Promise.resolve([0, 255, 0, 255]).then(callback);}};
const context = {module: {exports: {}}, console: {log(error) {caught.push({name: error.name, message: error.message});}},
  require(name) {
    if (name === 'sharp') return () => chain;
    if (name === '@stdlib/array/to-iterator') return values => values;
    if (name === '@stdlib/stats/iter/mean') return values => values.reduce((a, b) => a + b) / values.length;
    throw Error(name);
  }};
vm.runInNewContext(fs.readFileSync(source, 'utf8'), context, {filename: source});
const rows = [];
for (const name of ['pHash', 'PQDHash', 'blockMeanHash']) {
  assert.equal(context.module.exports[name]('unused-image'), undefined);
  rows.push({control: name + '_placeholder_returns_undefined'});
}
assert.equal(context.module.exports.aHash('unused-image'), undefined);
rows.push({control: 'aHash_returns_neither_hash_nor_promise'});
setImmediate(() => {
  assert.equal(context.bitStr, 'falsetruefalsetrue');
  rows.push({control: 'undeclared_global_contains_boolean_words', bitStr: context.bitStr});
  assert.equal(caught.length, 1);
  assert.equal(caught[0].name, 'ReferenceError');
  assert.equal(caught[0].message, 'bitSt is not defined');
  rows.push({control: 'callback_typo_caught_and_only_logged', error: caught[0]});
  assert.equal(rows.length, 6);
  const pins = Object.fromEntries([manifest, source, __filename, process.execPath].map(file => [file, sha(file)]));
  fs.writeFileSync(output, JSON.stringify({status: 'complete_original_module6_mocked_dependency_controls',
    controls: rows, authored_controls: 6, upstream_tests: 0, node_version: process.version, pins,
    scope: 'Unchanged pinned JS module, explicit mocked Sharp/mean/iterator adapters. Six observed language/control-flow behaviors; no actual image decode, Sharp API, dependency or benchmark qualification. No adoption.'}, null, 2) + '\n', {flag: 'wx'});
  console.log('6 original-module mocked-dependency controls passed');
});
