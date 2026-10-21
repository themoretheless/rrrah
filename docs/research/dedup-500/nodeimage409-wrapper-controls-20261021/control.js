const fs=require('fs'),vm=require('vm'),crypto=require('crypto'),assert=require('assert');
const root='.dedup-work/nodeimage409-20261021';
const source=fs.readFileSync(root+'/src/calculateHash.js','utf8');
const errors=[];const moduleObject={exports:{}};
const forcedError=new Error('controlled decoder failure');
vm.runInNewContext(source,{module:moduleObject,Buffer,console:{error:(...args)=>errors.push(args.map(String))},require:(name)=>name==='imghash'?{hash:async()=>{throw forcedError},hexToBinary:h=>[...h].map(x=>parseInt(x,16).toString(2).padStart(4,'0')).join('')}:require(name)});
const hash=moduleObject.exports;const rows=[];
(async()=>{
 for(const bytes of [[0,65,127],[128,255],[0,128,255,10]]) {
  const b=Buffer.from(bytes),r=await hash(b),expected=crypto.createHash('sha256').update(b).digest('hex');
  const transformed=crypto.createHash('sha256').update(Buffer.from(b.toString('latin1'),'utf8')).digest('hex');
  assert.equal(r.type,'sha256');assert.equal(r.hash,transformed);assert.equal(r.hash===expected,bytes.every(v=>v<128));
  rows.push({bytes,observed:r.hash,raw_byte_sha256:expected,matches_raw_bytes:r.hash===expected});
 }
 await assert.rejects(()=>hash(Buffer.from([128]),64,'hex',{useSha256Fallback:false}),e=>e===forcedError);
 const b=Buffer.from([128,255]);const h=await hash(b,64,'hex'),base=await hash(b,64,'base64'),binary=await hash(b,64,'binary');
 assert.equal(Buffer.from(base.hash,'base64').toString('hex'),h.hash);assert.equal(binary.hash.length,256);
 fs.writeFileSync('docs/research/dedup-500/nodeimage409-wrapper-controls-20261021/result.json',JSON.stringify({rows,fallback_disabled_rethrows:true,base64_consistent_with_transformed_digest:true,binary_bits:256,scope:'Original calculateHash source in Node VM; imghash dependency replaced with explicit rejecting adapter to reach fallback, real Node crypto/fs/Buffer. Not real image decoder or original Jest suite.'},null,2)+'\n');
 console.log('3 byte controls, disabled fallback and encoding controls passed; non-ASCII byte digest mismatch confirmed');
})().catch(e=>{console.error(e);process.exitCode=1});
