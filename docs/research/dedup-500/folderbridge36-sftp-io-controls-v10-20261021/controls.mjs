import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const root=process.argv[2],url=s=>`data:text/javascript;base64,${Buffer.from(s).toString('base64')}`;
const mock=url(stripTypeScriptTypes(readFileSync(`${root}/tests/__mocks__/obsidian.ts`,'utf8'),{mode:'transform'}));
const source=readFileSync(`${root}/src/SFTPAdapter.ts`,'utf8').replace(/^import .*\n/gm,'');
const {SFTPAdapter}=await import(url(`import {normalizePath} from '${mock}';const loadOptionalNodeModule=()=>null;const loadSessionCredential=()=>null;const decryptCredential=()=>null;const logger={error(){},warn(){}};\n${stripTypeScriptTypes(source)}`));
// Only exercise post-connection I/O with fake client. No protocol or credentials.
const make=sftp=>Object.assign(Object.create(SFTPAdapter.prototype),{sftp,connect:async()=>{}});
let a=make({list:async()=>[{name:'.',type:'d'},{name:'..',type:'d'},{name:'dir',type:'d'},{name:'link',type:'l'}]});
assert.deepEqual(await a.list('/root','Remote',{}),{files:['Remote/link'],folders:['Remote/dir']});
console.log(JSON.stringify({name:'dot-entries-skipped-symlink-listed-as-file',passed:true}));
a=make({list:async()=>{throw new Error('synthetic listing failure');}});assert.deepEqual(await a.list('/root','Remote',{}),{files:[],folders:[]});
console.log(JSON.stringify({name:'post-connect-list-error-returns-empty-list',passed:true}));
a.connect=async()=>{throw new Error('synthetic disconnected');};await assert.rejects(()=>a.list('/root','Remote',{}),/synthetic disconnected/);
console.log(JSON.stringify({name:'connect-error-propagates-before-list-catch',passed:true}));
const backing=Buffer.from([9,1,2,8]);a=make({get:async()=>backing.subarray(1,3)});assert.deepEqual(Array.from(new Uint8Array(await a.readBinary('/file'))),[1,2]);
console.log(JSON.stringify({name:'binary-read-slices-buffer-view-correctly',passed:true}));
