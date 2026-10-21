import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const {hashFile}=await import(pathToFileURL(path.resolve('.dedup-work/dupehunter295-20261021/src/hasher.ts')));
const root=await mkdtemp(path.join(tmpdir(),'rrrah-dupehunter295-'));
try {
 for(const [name,data,expected] of [
 ['empty',Buffer.alloc(0),'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'],
 ['abc',Buffer.from('abc'),'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'],
 ['binary',Buffer.from(Array.from({length:262151},(_,i)=>i%256)),null]]){
 const f=path.join(root,name);await writeFile(f,data);assert.equal(await hashFile(f),expected??createHash('sha256').update(data).digest('hex'));console.log('PASS original streaming hash',name,data.length);
 }
 await assert.rejects(hashFile(path.join(root,'missing')),{code:'ENOENT'});console.log('PASS missing file rejects ENOENT');
} finally {await rm(root,{recursive:true,force:true});}
