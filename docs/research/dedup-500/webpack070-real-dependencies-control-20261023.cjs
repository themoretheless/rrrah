// Original modules loaded using real package dependencies in an isolated cache.
const fs=require('fs'),path=require('path'),os=require('os'),crypto=require('crypto'),assert=require('assert/strict');
const {createRequire}=require('module');
const root=path.resolve(__dirname,'../../..');
const runtime=path.join(root,'.dedup-work/webpack070-runtime-20261023');
const evidence=path.join(__dirname,'evidence/atlassian-labs--webpack-deduplication-plugin');
const tree=JSON.parse(fs.readFileSync(path.join(evidence,'tree.json'))).tree;
for(const [name,local] of [['utils','04-utils.js'],['resolver','03-resolver.js'],['index','02-index.js']]) {
 const bytes=fs.readFileSync(path.join(evidence,local));
 assert.equal(crypto.createHash('sha1').update(Buffer.concat([Buffer.from(`blob ${bytes.length}\0`),bytes])).digest('hex'),tree.find(e=>e.path===`src/${name}.js`).sha);
 fs.writeFileSync(path.join(runtime,name+'.js'),bytes);
}
const load=createRequire(path.join(runtime,'control.cjs'));
const {getDuplicatedPackages}=load('./utils.js');
const {createMemoisedResolver}=load('./resolver.js');
const {deduplicate}=load('./index.js');
const versions=Object.fromEntries(['app-root-path','enhanced-resolve','fast-glob','find-package-json','lodash','mkdirp'].map(n=>[n,load(n+'/package.json').version]));
const tmp=fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(),'rrrah-webpack-real-')));
const originalCapture=Error.captureStackTrace,originalLimit=Error.stackTraceLimit;
try {
 const roots=['a','b'].map(x=>path.join(tmp,x));const cacheDir=path.join(tmp,'cache'),patchesPath=path.join(tmp,'patches');fs.mkdirSync(patchesPath);
 for(const r of roots) {
  fs.mkdirSync(r);fs.writeFileSync(path.join(r,'yarn.lock'),'same graph');
  for(const [i,parent] of ['parent-a','parent-b'].entries()) {
   const pkg=path.join(r,'node_modules',parent,'node_modules','pkg');fs.mkdirSync(pkg,{recursive:true});
   fs.writeFileSync(path.join(pkg,'package.json'),JSON.stringify({name:'pkg',version:'1.0.0',main:'index.js',dependencies:{}}));
   fs.writeFileSync(path.join(pkg,'index.js'),`module.exports=${i};`);
  }
 }
 const first=getDuplicatedPackages({rootPath:roots[0],cacheDir,patchesPath});
 assert.equal(first['pkg@1.0.0'].length,2);
 const second=getDuplicatedPackages({rootPath:roots[1],cacheDir,patchesPath});assert.deepEqual(second,first);
 const fresh=getDuplicatedPackages({rootPath:roots[1],patchesPath});assert(fresh['pkg@1.0.0'].every(p=>p.startsWith(roots[1]+path.sep)));
 const [target,other]=fresh['pkg@1.0.0'];assert.notEqual(fs.readFileSync(path.join(target,'index.js'),'utf8'),fs.readFileSync(path.join(other,'index.js'),'utf8'));
 const input={request:'./index.js',context:other};deduplicate(input,[fresh['pkg@1.0.0']],createMemoisedResolver(['main']));assert.equal(input.request,path.join(target,'index.js'));
 const memo=createMemoisedResolver(['main']);
 const one=['missing _____ /root/b','/root/c'],two=['missing','/root/b _____ /root/c'];
 assert.equal(memo(...one),one[0]);assert.equal(memo(...two),one[0]);assert.equal(createMemoisedResolver(['main'])(...two),two[0]);
 const retry=createMemoisedResolver(['main']);assert.equal(retry('./new.js',tmp),'./new.js');fs.writeFileSync(path.join(tmp,'new.js'),'1;');assert.equal(retry('./new.js',tmp),'./new.js');assert.equal(createMemoisedResolver(['main'])('./new.js',tmp),path.join(tmp,'new.js'));
 fs.writeFileSync(path.join(cacheDir,fs.readdirSync(cacheDir)[0]),'{');assert.throws(()=>getDuplicatedPackages({rootPath:roots[0],cacheDir,patchesPath}),SyntaxError);
 assert.equal(Error.captureStackTrace,originalCapture);assert.equal(Error.stackTraceLimit,originalLimit);
 console.log(JSON.stringify({node:process.version,versions,shared_cache_wrong_root:true,different_source_bytes_redirected:true,memo_key_collision:true,negative_result_stale:true,fresh_instance_sees_file:true,corrupt_cache_throws:true,error_globals_restored:true,webpack_compiler_run:false,original_jest_suite_run:false},null,2));
} finally {fs.rmSync(tmp,{recursive:true,force:true});}
