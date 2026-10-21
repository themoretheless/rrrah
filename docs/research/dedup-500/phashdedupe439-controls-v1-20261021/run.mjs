import{readFileSync,writeFileSync,mkdtempSync,rmSync}from'node:fs';
import{tmpdir}from'node:os';
import{join,resolve}from'node:path';
import{pathToFileURL}from'node:url';
import{stripTypeScriptTypes}from'node:module';
import assert from'node:assert/strict';
const sharpPath='/Users/themoretheless/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp/dist/index.mjs';
const sharp=(await import(pathToFileURL(sharpPath))).default;
const tmp=mkdtempSync(join(tmpdir(),'phash439-'));
try{
 const base=resolve('.dedup-work/phashdedupe439-20261021/src');
 for(const name of ['hash','cluster']){
  let src=stripTypeScriptTypes(readFileSync(join(base,name+'.ts'),'utf8'));
  src=src.replace('from "sharp"','from '+JSON.stringify(pathToFileURL(sharpPath).href)).replaceAll('"./hash.js"','"./hash.mjs"');
  writeFileSync(join(tmp,name+'.mjs'),src);
 }
 const{hashImage,hammingDistance}=await import(pathToFileURL(join(tmp,'hash.mjs')));
 const{findDuplicatePairs,clusterDuplicates}=await import(pathToFileURL(join(tmp,'cluster.mjs')));
 const pairs=findDuplicatePairs(new Map([['A',0n],['B',1n],['C',3n]]),1);
 assert.equal(pairs.length,2);assert.equal(clusterDuplicates(pairs)[0].length,3);assert.equal(hammingDistance(0n,3n),2);
 console.log('PASS original transitive policy groups A/B/C although A/C distance2 exceeds1 (documented policy, not contract violation)');
 assert.equal(hammingDistance(-1n,0n),0);
 console.log('PASS negative bigint input returns distance0 without rejection');
 assert.deepEqual(findDuplicatePairs(new Map([['A',0n],['B',0n]]),NaN),[]);
 console.log('PASS NaN threshold silently omits identical pair');
 for(const[v,name]of[[0,'black'],[255,'white']])await sharp({create:{width:32,height:32,channels:3,background:{r:v,g:v,b:v}}}).png().toFile(join(tmp,name+'.png'));
 assert.equal(await hashImage(join(tmp,'black.png')),0n);assert.equal(await hashImage(join(tmp,'white.png')),0n);
 console.log('PASS actualSharp solid black/white bothhash0 atstrictthreshold0');
 const raw=Buffer.alloc(9*8*4);
 for(let i=0;i<72;i++){raw[i*4]=raw[i*4+1]=raw[i*4+2]=128;raw[i*4+3]=32+(i%9)*24;}
 const file=join(tmp,'alpha.png');await sharp(raw,{raw:{width:9,height:8,channels:4}}).png().toFile(file);
 const decoded=await sharp(file).grayscale().resize(9,8,{fit:'fill'}).raw().toBuffer({resolveWithObject:true});
 assert.equal(decoded.info.channels,1);assert.equal(decoded.data.length,72);
 const actual=await hashImage(file);let oracle=0n;
 for(let y=0;y<8;y++)for(let x=0;x<8;x++){oracle<<=1n;if(decoded.data[(y*9+x)*decoded.info.channels]>decoded.data[(y*9+x+1)*decoded.info.channels])oracle|=1n;}
 assert.equal(actual,oracle);
 console.log('PASS alpha-interleaving hypothesis REJECTED: actualSharp emits onechannel and original hash matches channelawareoracle');
 console.log('Sharp version '+sharp.versions.sharp+'; TOTAL5passed, actualbundledSharp version maydifferfrompinnedlock; no CLI/deletion');
}finally{rmSync(tmp,{recursive:true,force:true});}
