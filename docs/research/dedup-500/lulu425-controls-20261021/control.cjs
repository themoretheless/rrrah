const fs=require('fs'),vm=require('vm'),path=require('path'),assert=require('assert');
const root=path.resolve('.dedup-work/lulu425-20261021');const ts=require(root+'/node_modules/typescript');
function load(p){const module={exports:{}};const js=ts.transpileModule(fs.readFileSync(root+'/'+p,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2021}}).outputText;vm.runInNewContext(js,{module,exports:module.exports,require,Buffer});return module.exports;}
const {generateHash}=load('src/hash.ts');const {distance}=load('src/distance.ts');
const data=Buffer.alloc(4*4*4);for(let y=0;y<4;y++)for(let x=0;x<4;x++){const i=(y*4+x)*4;data[i]=30*x;data[i+3]=255;}
// Every 4-value band is [0,30,60,90], independent median=(30+60)/2=45.
// Above-median bits are 0011 per band, hexadecimal3333.
const fast=generateHash({width:4,height:4,data},false,4),precise=generateHash({width:4,height:4,data},true,4);assert.equal(fast,'1111');assert.equal(precise,fast);assert.notEqual(fast,'3333');
const odd=Buffer.alloc(5*5*4);for(let i=0;i<25;i++)odd[i*4+3]=255;const changed=Buffer.from(odd);changed[96]=changed[97]=changed[98]=255;
const before=generateHash({width:5,height:5,data:odd},false,4),after=generateHash({width:5,height:5,data:changed},false,4);assert.equal(before,after);
const edit=distance('0','f');assert.equal(edit,1);assert.notEqual(edit,4);
fs.writeFileSync('docs/research/dedup-500/lulu425-controls-20261021/result.json',JSON.stringify({median:{input_bands:[0,30,60,90],independent_median:45,expected_hash:'3333',observed_fast:fast,observed_precise:precise},discarded_edge:{width:5,height:5,bits:4,changed_pixel:[4,4],before,after},distance:{a:'0',b:'f',observed_edit_distance:edit,independent_bit_hamming:4},scope:'Original complete TS functions transpiled unchanged with installed TypeScript, executed in VM. Pure decoded RGBA controls, no PNG/JPEG decoder or full package adapter.'},null,2)+'\n');console.log('Confirmed wrong even median, fast edge omission, and string edit versus bit distance');
