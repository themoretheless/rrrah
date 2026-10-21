// Unchanged original resolver plugin; authored synchronous resolver callbacks.
const assert = require('node:assert/strict');
const Plugin = require('../../../../.dedup-work/webpack216-20261021');
function hook(options) {
  let handler;
  new Plugin(options).apply({plugin(name, callback) {
    assert.equal(name, 'before-described-relative'); handler=callback;
  }});
  return request => {let n=0; handler(request,()=>n++);assert.equal(n,1);return request.path;};
}
const req=(name,version,path,relativePath='.')=>({descriptionFileData:{name,version},path,relativePath});
let h=hook({modules:{}});
assert.equal(h(req('a','1','/one')), '/one');
assert.equal(h(req('a','1','/two')), '/one');
console.log('PASS same name/version reuses first path without byte inspection');
assert.equal(h(req('a','2','/three')), '/three');
console.log('PASS distinct version preserved');
assert.equal(h(req('a','1','/nested','./file')), '/nested');
console.log('PASS non-root relative path skipped');
h=hook({modules:{},exclude:()=>true});assert.equal(h(req('a','1','/excluded')), '/excluded');
console.log('PASS exclude guard');
const first=hook();const second=hook();
assert.equal(first(req('shared-fixture','1','/build-A')), '/build-A');
assert.equal(second(req('shared-fixture','1','/build-B')), '/build-A');
console.log('PASS default cache leaks across separate plugin instances');
h=hook({modules:{}});
h(req(undefined,undefined,'/unnamed-A'));
assert.equal(h(req(undefined,undefined,'/unnamed-B')), '/unnamed-A');
console.log('PASS missing name/version collapse to same key');
