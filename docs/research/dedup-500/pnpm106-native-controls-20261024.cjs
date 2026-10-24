const vm=require('node:vm'),fs=require('node:fs/promises'),os=require('node:os'),path=require('node:path'),assert=require('node:assert/strict');
const semver=require('../webpack070-runtime-20261023/node_modules/semver');
(async()=>{const reports=[];const tmp=await fs.mkdtemp(path.join(os.tmpdir(),'pnpm106-owned-'));try{
const original='{"name":"owned","version":"1.0.0"}\n';const mp=path.join(tmp,'package.json');
async function execute(mode,fail){await fs.writeFile(mp,original);let calls=0;const logs=[];
const ctx=vm.createContext({console:{log:(...s)=>logs.push(s.join(' '))}});
const source=await fs.readFile(path.join(__dirname,'index.mjs'),'utf8');
const leaf=()=>({dependencies:[]});
const hosts={
'@pnpm/lockfile-file':{readWantedLockfile:async()=>({importers:{'.':{specifiers:{pkg:'^1.0.0'}}}})},
'@pnpm/lockfile-utils':{nameVerFromPkgSnapshot:(_p,s)=>({name:'pkg',version:s.version})},
'@pnpm/lockfile-walker':{lockfileWalker:()=>({step:{dependencies:['1.0.0','1.1.0'].map(version=>({depPath:'/pkg/'+version,pkgSnapshot:{version},next:leaf}))}})},
'@pnpm/manifest-utils':{getAllDependenciesFromManifest:()=>({})},
'@pnpm/read-project-manifest':{safeReadProjectManifestOnly:async()=>({}),readProjectManifest:async()=>({fileName:'package.json',manifest:JSON.parse(await fs.readFile(mp,'utf8')),writeProjectManifest:async m=>fs.writeFile(mp,JSON.stringify(m))})},
'child_process':{execSync:()=>{calls++;if(fail&&calls===2)throw Error('owned-install-failure')}},
'dependency-path':{depPathToFilename:p=>p.replaceAll('/','+')},
'fs/promises':{default:fs},path:{default:path},semver:{default:semver}};
const mod=new vm.SourceTextModule(source,{context:ctx});await mod.link(async spec=>{const h=hosts[spec];assert(h,spec);return new vm.SyntheticModule(Object.keys(h),function(){for(const[k,v]of Object.entries(h))this.setExport(k,v)},{context:ctx})});await mod.evaluate();let error=null;try{await mod.namespace[mode](tmp)}catch(e){error=e.message};return{calls,error,manifest:await fs.readFile(mp,'utf8'),logs};}
let r=await execute('listDuplicates',false);assert.equal(r.calls,1);assert.equal(r.manifest,original);reports.push({control:'list-dispatches-install',install_calls:r.calls,scope:'Recording execSync host, no real package manager or network'});
r=await execute('fixDuplicates',true);assert.equal(r.error,'owned-install-failure');assert.equal(r.calls,2);assert.equal(JSON.parse(r.manifest).pnpm.overrides['pkg@^1.0.0'],'1.1.0');reports.push({control:'failed-second-install-leaves-overrides',install_calls:r.calls,overrides:JSON.parse(r.manifest).pnpm.overrides});
r=await execute('fixDuplicates',false);assert.equal(r.calls,3);assert.equal(r.manifest,original);reports.push({control:'healthy-path-restores-original-bytes',install_calls:r.calls,restored:true});
process.stdout.write(JSON.stringify({passed:3,author_controls:reports,scope:'Original index.ts transpiled by esbuild0.28.0, Node22.23.4 vm modules, real filesystem and semver '+require('../webpack070-runtime-20261023/node_modules/semver/package.json').version+', recording pnpm/dependency metadata hosts. Not original full dependency cohort or upstream tests.'},null,2)+'\n');
}finally{await fs.rm(tmp,{recursive:true,force:true})}})().catch(e=>{console.error(e);process.exitCode=1});
