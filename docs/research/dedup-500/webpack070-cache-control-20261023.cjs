// Selected original helper in VM; dependency adapters, no Webpack build.
const fs = require('fs');
const path = require('path');
const os = require('os');
const vm = require('vm');
const crypto = require('crypto');
const assert = require('assert/strict');
const base = path.join(__dirname, 'evidence/atlassian-labs--webpack-deduplication-plugin');
const tree = JSON.parse(fs.readFileSync(path.join(base, 'tree.json'))).tree;
const text = fs.readFileSync(path.join(base, '04-utils.js'));
assert.equal(crypto.createHash('sha1').update(Buffer.concat([Buffer.from(`blob ${text.length}\0`), text])).digest('hex'), tree.find(x => x.path === 'src/utils.js').sha);
const sandbox = { module: { exports: {} }, require(name) {
    if (['crypto','fs','path'].includes(name)) return require(name);
    // Imports unused by getCacheKey; refuse any accidental dependency call.
    return new Proxy(function () { throw Error(`unexpected call: ${name}`); }, {get() { throw Error(`unexpected access: ${name}`); }});
}};
vm.runInNewContext(text.toString() + '\nmodule.exports.controlGetCacheKey = getCacheKey;', sandbox);
const getCacheKey = sandbox.module.exports.controlGetCacheKey;
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'rrrah-webpack-key-'));
try {
    const roots = ['a','b'].map(x => path.join(tmp,x));
    for (const root of roots) { fs.mkdirSync(root); fs.writeFileSync(path.join(root,'yarn.lock'), 'same locked package graph\n'); }
    const key = rootPath => getCacheKey({rootPath, patchedPackages: []});
    assert.equal(key(roots[0]), key(roots[1]));
    const before = key(roots[0]);
    fs.mkdirSync(path.join(roots[0],'node_modules'));
    fs.writeFileSync(path.join(roots[0],'node_modules','changed-package.js'), 'different installed bytes');
    assert.equal(before, key(roots[0]));
    fs.writeFileSync(path.join(roots[0],'yarn.lock'), 'changed lockfile\n');
    assert.notEqual(before, key(roots[0]));
    console.log(JSON.stringify({node:process.version,pinned_helper_verified:true,different_roots_same_key:true,installed_bytes_change_same_key:true,lockfile_change_different_key:true,webpack_runtime_run:false}));
} finally { fs.rmSync(tmp,{recursive:true,force:true}); }
