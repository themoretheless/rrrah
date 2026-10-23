// Original resolver body with explicit deterministic resolver and memoize adapters.
const fs=require('fs'),path=require('path'),os=require('os'),vm=require('vm'),crypto=require('crypto'),assert=require('assert/strict');
const base=path.join(__dirname,'evidence/atlassian-labs--webpack-deduplication-plugin');
const source=fs.readFileSync(path.join(base,'03-resolver.js'));
const pin=JSON.parse(fs.readFileSync(path.join(base,'tree.json'))).tree.find(x=>x.path==='src/resolver.js');
assert.equal(crypto.createHash('sha1').update(Buffer.concat([Buffer.from(`blob ${source.length}\0`),source])).digest('hex'),pin.sha);
let calls=0;
let resolveSync=(_options,context,request)=>{calls++;return JSON.stringify([request,context]);};
function memoize(fn,keyFor) {const entries=new Map();return (...args)=>{const key=keyFor(...args);if(!entries.has(key))entries.set(key,fn(...args));return entries.get(key);};}
const sandbox={Error,module:{exports:{}},require(name){
 if(name==='fs')return fs;
 if(name==='lodash/memoize')return memoize;
 if(name==='enhanced-resolve')return {CachedInputFileSystem:class {},ResolverFactory:{createResolver(){return {resolveSync:(...args)=>resolveSync(...args)};}}};
 throw Error(name);
}};
vm.runInNewContext(source.toString(),sandbox);
const create=sandbox.module.exports.createMemoisedResolver;
const stack=Error.captureStackTrace,limit=Error.stackTraceLimit;
const checkGlobals=()=>{assert.equal(Error.captureStackTrace,stack);assert.equal(Error.stackTraceLimit,limit);};
const resolver=create(['main']);
const one=['a _____ /root/b','/root/c'],two=['a','/root/b _____ /root/c'];
assert.notEqual(JSON.stringify(one),JSON.stringify(two));
assert.equal(resolver(...one),JSON.stringify(one));checkGlobals();
assert.equal(resolver(...two),JSON.stringify(one));assert.equal(calls,1);checkGlobals();
// No collision on ordinary distinct keys.
assert.equal(resolver('different','/root/c'),JSON.stringify(['different','/root/c']));assert.equal(calls,2);
const tmp=fs.mkdtempSync(path.join(os.tmpdir(),'rrrah-resolver-retry-'));
try {
 resolveSync=(_options,context,request)=>{calls++;const target=path.join(context,request);fs.accessSync(target);return target;};
 const cached=create(['main']);const before=calls;
 assert.equal(cached('new.js',tmp),'new.js');checkGlobals();
 fs.writeFileSync(path.join(tmp,'new.js'),'module.exports=1');
 assert.equal(cached('new.js',tmp),'new.js');assert.equal(calls,before+1);checkGlobals();
 assert.equal(create(['main'])('new.js',tmp),path.join(tmp,'new.js'));checkGlobals();
 console.log(JSON.stringify({node:process.version,original_module_git_verified:true,distinct_pairs_share_cached_result:true,failed_lookup_survives_file_creation:true,new_resolver_sees_file:true,error_globals_restored:true,enhanced_resolve_and_lodash_real_dependencies_run:false}));
} finally {fs.rmSync(tmp,{recursive:true,force:true});checkGlobals();}
