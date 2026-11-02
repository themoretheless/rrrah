import {scanFiles} from './scanFiles.mts';
import fs from 'node:fs/promises';import path from 'node:path';import assert from 'node:assert/strict';
const root=await fs.mkdtemp(new URL('./owned-',import.meta.url));
await fs.mkdir(path.join(root,'nested'));await fs.writeFile(path.join(root,'healthy.ts'),'x');await fs.writeFile(path.join(root,'nested','copy.ts'),'x');await fs.writeFile(path.join(root,'oversized.ts'),Buffer.alloc(256*1024+1));
const adapter={exists:async p=>fs.access(p).then(()=>true,()=>false),readFile:async p=>fs.readFile(p,'utf8'),stat:async p=>fs.stat(p),readDir:async p=>(await fs.readdir(p,{withFileTypes:true})).map(e=>({name:e.name,isDirectory:e.isDirectory(),isFile:e.isFile()}))};
const normal=await scanFiles(adapter,root);assert.deepEqual(normal.map(x=>x.path),['healthy.ts','nested/copy.ts']);
const missing=await scanFiles(adapter,path.join(root,'absent'));assert.deepEqual(missing,[]);
const partial=await scanFiles({...adapter,readDir:async p=>{if(p===path.join(root,'nested'))throw Error('owned injected read failure');return adapter.readDir(p)}},root);assert.deepEqual(partial.map(x=>x.path),['healthy.ts']);
console.log(JSON.stringify({runtime:process.version,controls_passed:3,normal,missing,partial,scope:'Original unchanged TypeScript through Node strip-types;owned real filesystem adapter;one injected child read error;not full upstream Vitest or app',findings:['256KiB+1 file omitted with no completeness signal','Missing root returns empty list','Child enumeration error silently yields partial list retaining healthy source'],root},null,2));
