import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const root=path.resolve('.dedup-work/googlephotos42-20261021');
const out=path.join(root,'popup-catalog-runtime');await fs.mkdir(out,{recursive:true});
const flatten=(value,prefix='',result={})=>{for(const [k,v] of Object.entries(value)){if(typeof v==='string')result[prefix+k]=v;else flatten(v,prefix+k+'.',result)}return result};
const catalogs={};
for(const code of ['en','de','es','fr','it']){
 await fs.writeFile(path.join(out,code+'.mjs'),stripTypeScriptTypes(await fs.readFile(path.join(root,'src/extension/popup/i18n/locales',code+'.ts'),'utf8')));
 catalogs[code]=flatten((await import(pathToFileURL(path.join(out,code+'.mjs')))).default);
}
const keys=Object.keys(catalogs.en).sort();
for(const code of ['de','es','fr','it']){
 assert.deepEqual(Object.keys(catalogs[code]).sort(),keys);
 for(const key of keys){assert.ok(catalogs[code][key].length>0);assert.deepEqual((catalogs[code][key].match(/\{\w+\}/g)??[]).sort(),(catalogs.en[key].match(/\{\w+\}/g)??[]).sort(),code+':'+key);}
 console.log('PASS '+code+': '+keys.length+' nonempty keys and placeholder multisets match English');
}
