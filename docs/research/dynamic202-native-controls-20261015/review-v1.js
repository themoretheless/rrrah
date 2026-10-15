'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const Module = require('node:module');
const dedupe = require('./index.js');
const originalExtension = require.extensions['.js'];
const originalLoad = Module._load;
let tests = 0, assertions = 0, ended = 0;
// Explicit synchronous adapter for the original six tap callbacks only.
Module._load = function (name, parent, isMain) {
  if (name === 'tap') return {test(name, callback) {
    tests++;
    callback({equal(a,b) {assert.equal(a,b);assertions++;}, end(){ended++;}});
  }};
  return originalLoad.apply(this, arguments);
};
try {require('./test/dedupe.js');} finally {
  Module._load = originalLoad; dedupe.deactivate(); dedupe.reset();
}
assert.equal(tests,6);assert.equal(assertions,18);assert.equal(ended,6);
console.log('original_callbacks=6 original_assertions=18 tap_adapter=true');
const root = fs.mkdtempSync(path.join(os.tmpdir(),'dynamic202-review-'));
let authored = 0;
function fixture(relative, source) {
  const filename=path.join(root,relative);
  fs.mkdirSync(path.dirname(filename),{recursive:true});fs.writeFileSync(filename,source);return filename;
}
function check(name, fn) {
  dedupe.deactivate();dedupe.reset();
  try {fn();authored++;console.log('PASS '+name);} finally {require.extensions['.js']=originalExtension;dedupe.reset();}
}
try {
  check('different_dependencies_share_first_exports',()=>{
    const a=fixture('dependency/a/common/dep/foo.js',"module.exports=require('./helper');");
    const b=fixture('dependency/b/common/dep/foo.js',"module.exports=require('./helper');");
    fixture('dependency/a/common/dep/helper.js',"module.exports={value:'first'};");
    fixture('dependency/b/common/dep/helper.js',"module.exports={value:'second'};");
    dedupe.activate();const first=require(a),second=require(b);
    assert.equal(first.value,'first');assert.equal(second,first);
  });
  check('directory_component_concatenation_collision',()=>{
    const a=fixture('components/root1/a/bc/foo.js','module.exports={directory:__dirname};');
    const b=fixture('components/root2/ab/c/foo.js','module.exports={directory:__dirname};');
    dedupe.activate('.js',2);const first=require(a),second=require(b);
    assert.notEqual(path.dirname(a),path.dirname(b));assert.equal(second,first);assert.equal(second.directory,path.dirname(a));
  });
  check('deactivate_discards_later_third_party_hook',()=>{
    const hook=function(m,f){return originalExtension(m,f);};require.extensions['.js']=hook;
    dedupe.activate();dedupe.deactivate();assert.equal(require.extensions['.js'],originalExtension);assert.notEqual(require.extensions['.js'],hook);
  });
  check('reset_leaves_node_require_cache_intact',()=>{
    const a=fixture('reset/a/common/dep/foo.js','module.exports={value:1};');
    const b=fixture('reset/b/common/dep/foo.js','module.exports={value:1};');
    dedupe.activate();const first=require(a);assert.equal(require(b),first);dedupe.reset();assert.equal(require(b),first);
  });
  check('failed_module_load_can_retry',()=>{
    const a=fixture('retry/common/dep/foo.js',"throw new Error('synthetic');");dedupe.activate();assert.throws(()=>require(a),/synthetic/);
    fs.writeFileSync(a,'module.exports={value:2};');assert.equal(require(a).value,2);
  });
} finally {require.extensions['.js']=originalExtension;Module._load=originalLoad;fs.rmSync(root,{recursive:true,force:true});}
assert.equal(authored,5);console.log('authored_controls=5');
