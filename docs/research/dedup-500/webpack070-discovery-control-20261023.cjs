// Original discovery module; explicit dependency adapters, real owned filesystem.
const fs = require('fs'), path = require('path'), os = require('os'), vm = require('vm');
const crypto = require('crypto'), assert = require('assert/strict');
const {isDeepStrictEqual} = require('util');
const base = path.join(__dirname,'evidence/atlassian-labs--webpack-deduplication-plugin');
const source = fs.readFileSync(path.join(base,'04-utils.js'));
const pin = JSON.parse(fs.readFileSync(path.join(base,'tree.json'))).tree.find(e=>e.path==='src/utils.js');
assert.equal(crypto.createHash('sha1').update(Buffer.concat([Buffer.from(`blob ${source.length}\0`),source])).digest('hex'),pin.sha);
let packagePaths = [], discoveries = 0;
const adapters = {
 'app-root-path': {toString() {throw Error('rootPath required');}},
 'fast-glob': {sync() {discoveries++; return packagePaths;}},
 'lodash/flatten': list => list.flat(),
 'lodash/isEqual': isDeepStrictEqual,
 'lodash/pickBy': (obj, predicate) => Object.fromEntries(Object.entries(obj).filter(([k,v])=>predicate(v,k))),
 'lodash/transform': (obj, iterate, result) => {for(const [k,v] of Object.entries(obj)) iterate(result,v,k);return result;},
 'mkdirp': {sync: dir => fs.mkdirSync(dir,{recursive:true})},
};
const sandbox = {module:{exports:{}},require(name) {return ['crypto','fs','path'].includes(name)?require(name):adapters[name]??(()=>{throw Error(name);})();}};
vm.runInNewContext(source.toString(),sandbox);
const {getDuplicatedPackages} = sandbox.module.exports;
const tmp = fs.mkdtempSync(path.join(os.tmpdir(),'rrrah-webpack-discovery-'));
try {
 const roots = ['a','b'].map(x=>path.join(tmp,x)), cacheDir=path.join(tmp,'cache'), patchesPath=path.join(tmp,'patches');
 fs.mkdirSync(patchesPath);
 const paths = roots.map(root=>{
  fs.mkdirSync(root);fs.writeFileSync(path.join(root,'yarn.lock'),'identical graph');
  return ['parent-a','parent-b'].map((parent,i)=>{
   const dir=path.join(root,'node_modules',parent,'node_modules','pkg');fs.mkdirSync(dir,{recursive:true});
   fs.writeFileSync(path.join(dir,'package.json'),JSON.stringify({name:'pkg',version:'1.0.0',dependencies:{}}));
   fs.writeFileSync(path.join(dir,'index.js'),`module.exports=${i};`);
   return path.join(dir,'package.json');
  });
 });
 packagePaths=paths[0];
 const first=getDuplicatedPackages({rootPath:roots[0],cacheDir,patchesPath});
 assert.equal(first['pkg@1.0.0'].length,2);
 assert.notEqual(fs.readFileSync(path.join(first['pkg@1.0.0'][0],'index.js'),'utf8'),fs.readFileSync(path.join(first['pkg@1.0.0'][1],'index.js'),'utf8'));
 packagePaths=paths[1];
 const stale=getDuplicatedPackages({rootPath:roots[1],cacheDir,patchesPath});
 assert.equal(JSON.stringify(stale),JSON.stringify(first));assert.equal(discoveries,1);
 assert(stale['pkg@1.0.0'].every(p=>p.startsWith(roots[0]+path.sep)));
 const fresh=getDuplicatedPackages({rootPath:roots[1],patchesPath});
 assert.equal(discoveries,2);assert(fresh['pkg@1.0.0'].every(p=>p.startsWith(roots[1]+path.sep)));
 const cacheFile=path.join(cacheDir,fs.readdirSync(cacheDir)[0]);fs.writeFileSync(cacheFile,'{');
 assert.throws(()=>getDuplicatedPackages({rootPath:roots[0],cacheDir,patchesPath}),/JSON|position|property/);
 console.log(JSON.stringify({node:process.version,original_module_git_verified:true,shared_cache_returns_other_root_paths:true,unequal_source_bytes_grouped:true,without_cache_correct_root:true,corrupt_cache_throws_without_rebuild:true,webpack_build_run:false}));
} finally {fs.rmSync(tmp,{recursive:true,force:true});}
