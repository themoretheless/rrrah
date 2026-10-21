const fs=require('fs'),vm=require('vm'),path=require('path'),assert=require('assert');
const ts=require(path.resolve('.dedup-work/lulu425-20261021/node_modules/typescript'));
const source=fs.readFileSync('docs/research/dedup-500/evidence/ClaudiuCeia--dhash/02-dhash.ts','utf8');
const moduleObject={exports:{}};const code=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2021,esModuleInterop:true}}).outputText;
vm.runInNewContext(code,{module:moduleObject,exports:moduleObject.exports,require:n=>n==='sharp'?()=>{throw Error('sharp not exercised in pure helper controls')}:require(n)});
const {compare,invertHash,toAscii}=moduleObject.exports;
for(let i=0;i<64;i++){
 const value=1n<<BigInt(i),hex=value.toString(16).padStart(16,'0');
 assert.equal(compare('0000000000000000',hex),1);assert.equal(compare(hex,'0000000000000000'),1);assert.equal(compare(hex,invertHash(hex)),64);assert.equal(invertHash(invertHash(hex)),hex);
 const ascii=toAscii(hex,['.','#']).replaceAll('\n','');assert.equal(ascii.length,64);assert.equal(ascii.indexOf('#'),63-i);assert.equal(ascii.split('#').length-1,1);
}
assert.equal(compare('f','0'),4);assert.equal(compare('A','a'),0);
let representationError;try{compare('0','00')}catch(e){representationError=e.message}assert(representationError);
fs.writeFileSync('docs/research/dedup-500/dhash440-helper-controls-20261021/result.json',JSON.stringify({one_hot_positions_checked:64,assertions_per_position:7,short_hex_bit_distance:4,uppercase_supported:true,zero_padding_same_value_rejected:representationError,scope:'Original entire TS module transpiled unchanged with TypeScript and executed in Node VM; sharp import replaced with throwing stub. Only pure helpers compare/invertHash/toAscii called, no image pipeline or original Bun/Deno tests.'},null,2)+'\n');console.log('64 one-hot bit/ASCII/inversion controls passed; zero-padding representation mismatch rejected');
