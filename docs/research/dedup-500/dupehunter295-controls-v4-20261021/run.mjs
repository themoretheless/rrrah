import '../dupehunter295-controls-v2-20261021/loader.mjs';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {spawnSync} from 'node:child_process';
const root=path.resolve('.dedup-work/dupehunter295-20261021');
const {parseCliArgs}=await import(pathToFileURL(path.join(root,'src/cli-args.ts')));
assert.equal(parseCliArgs(['--concurrency','2garbage']).concurrency,2);
assert.equal(parseCliArgs(['--concurrency','1.9']).concurrency,1);
assert.throws(()=>parseCliArgs(['--concurrency','NaN']));
console.log('PASS concurrency parser accepts numeric prefixes, rejects NaN text');
assert.equal(parseCliArgs(['--min-size','9'.repeat(400)]).minSizeBytes,Infinity);
console.log('PASS oversized min-size accepted as Infinity');
const temp=await mkdtemp(path.join(tmpdir(),'rrrah-dupehunter295-cli-'));
try{
 const result=spawnSync(process.execPath,['--experimental-strip-types','--import',path.resolve('docs/research/dedup-500/dupehunter295-controls-v2-20261021/loader.mjs'),path.join(root,'src/cli.ts'),path.join(temp,'missing'),'--json'],{encoding:'utf8'});
 assert.equal(result.status,0,result.stderr);
 const report=JSON.parse(result.stdout);assert.equal(report.filesScanned,0);assert.equal(report.warnings.length,1);
 console.log('PASS nonexistent scan root returns CLI exit0 with JSON warning');
}finally{await rm(temp,{recursive:true,force:true});}
