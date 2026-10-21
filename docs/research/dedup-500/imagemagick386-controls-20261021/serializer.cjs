const fs=require('fs'),vm=require('vm'),assert=require('assert'),crypto=require('crypto');
const {EventEmitter}=require('events');
const {Readable,Writable}=require('stream');
const source=fs.readFileSync('.dedup-work/imagemagick386-20261021/lib/phash.js','utf8');
const compare=require('../evidence/science-periodicals--phash-imagemagick/02-compare.js');
function generate(value){
 const child=new EventEmitter(),rl=new EventEmitter();child.stdin=new Writable({write(c,e,cb){cb();}});child.stdout=new Readable({read(){}});rl.close=()=>{};
 const context={module:{exports:{}},process:{env:{},stdout:process.stdout},require(name){if(name==='child_process')return {spawn:()=>child};if(name==='readline')return {createInterface:()=>rl};if(name==='fs')return fs;throw Error(name);}};
 vm.runInNewContext(source,context);
 let output;
 context.module.exports(Readable.from([]),(err,data)=>{assert.ifError(err);output=data;});
 rl.emit('line','  Type: TrueColor');rl.emit('line','  Units: PixelsPerCentimeter');rl.emit('line','  Resolution: 100x100');rl.emit('line','  Channel perceptual hash:');
 for(let i=0;i<7;i++)rl.emit('line',`    PH${i+1}: ${Array(6).fill(value).join(',')}`);
 child.emit('close',0);return output;
}
const zero=generate(0),large=generate(100),negative=generate(-10);
assert.strictEqual(zero.pHash,large.pHash);
assert.strictEqual(compare.compare(zero,large),0);
assert.strictEqual(zero.pHash.length,168);assert.strictEqual(negative.pHash.length,210);
assert.strictEqual(zero.ppi.width,39.3701);
const result={source_sha256:crypto.createHash('sha256').update(source).digest('hex'),original_phash_js_executed_unchanged:true,adapter:'VM stubs child_process/readline with synthetic identify stdout; no ImageMagick execution',controls:{coefficient_0_vs_100_identical_serialization:true,comparison_distance:0,zero_hash_length:168,minus10_hash_length:210,expected_fixed_width:168,ppi_from_100_pixels_per_cm:zero.ppi.width,correct_pixels_per_inch:254},limits:['Synthetic moment values test serializer boundary; no claim these values occur in the tracked fixture corpus.','No backend hash generation, native pHash or ASAN execution.']};
fs.writeFileSync(__dirname+'/serializer-result.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
