import {readFileSync} from 'node:fs';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const root=process.argv[2],url=s=>`data:text/javascript;base64,${Buffer.from(s).toString('base64')}`;
const source=readFileSync(`${root}/src/OSHelpers.ts`,'utf8').replace(/^import .*\n/gm,'').replace(/^const (fs|path):.*\n/gm,'');
const h=await import(url(`import fs from 'node:fs';import path from 'node:path';const Platform={isMobile:false};\n${stripTypeScriptTypes(source)}`));
assert.equal(h.normalizeForComparison('C:\\Notes\\File.md'),h.normalizeForComparison('c:/notes/file.md'));
console.log(JSON.stringify({name:'windows-dialect-comparison-folds-case',passed:true}));
assert.notEqual(h.normalizeForComparison('/Notes/File.md'),h.normalizeForComparison('/notes/file.md'));
console.log(JSON.stringify({name:'posix-comparison-retains-case',passed:true}));
const temp=await mkdtemp(path.join(tmpdir(),'fb-os-control-'));
try {
 const file=path.join(temp,'ordinary.md');await writeFile(file,'ordinary local text');
 assert.equal(await h.isCloudPlaceholder(file),true);
 console.log(JSON.stringify({name:'ordinary-existing-file-also-classified-placeholder',passed:true}));
 assert.equal(await h.areDifferentDevices(file,path.join(temp,'missing')),false);
 console.log(JSON.stringify({name:'missing-device-stat-collapses-to-false',passed:true}));
} finally {await rm(temp,{recursive:true,force:true});}
