const fs=require('fs'),path=require('path'),assert=require('assert');
const ts=require(path.resolve('.dedup-work/lulu425-20261021/node_modules/typescript'));
const postcss=require(path.resolve('.dedup-work/lulu425-20261021/node_modules/postcss'));
const root='.dedup-work/dhash440-20261021';const locks=[];
for(const file of ['bun.lock','deno.lock','deno.test.lock','docs/deno.lock']){
 const parsed=ts.parseConfigFileTextToJson(file,fs.readFileSync(root+'/'+file,'utf8'));assert(!parsed.error);const d=parsed.config;let nodes=0,integrity=0;
 function walk(v){nodes++;if(Array.isArray(v)){for(const a of v)walk(a);}else if(v&&typeof v==='object'){for(const [k,a]of Object.entries(v)){if(k==='integrity'){assert.equal(typeof a,'string');if(a.startsWith('sha512-'))assert.equal(Buffer.from(a.slice(7),'base64').length,64);else assert(/^[a-f0-9]{64}$/.test(a));integrity++;}walk(a);}}}
 walk(d);locks.push({path:file,root_keys:Object.keys(d),packages:Object.keys(d.packages||d.npm||{}).length,nodes,integrity_fields:integrity});
}
const css=postcss.parse(fs.readFileSync(root+'/docs/styles.css','utf8'));const counts={};let imports=0,urls=[];
css.walk(n=>{counts[n.type]=(counts[n.type]||0)+1;if(n.type==='atrule'&&n.name==='import')imports++;if(n.type==='decl'&&n.value.includes('url('))urls.push(n.value);});assert.equal(imports,0);
fs.writeFileSync('docs/research/dedup-500/dhash440-metadata-controls-20261021/result.json',JSON.stringify({locks,css:{node_types:counts,unresolved_imports:imports,url_values:urls},scope:'All lock JSON/JSONC nodes structurally traversed and integrity fields checked; Deno frozen tests already executed test/demo locks. Bun install not run. Entire generated Tailwind CSS AST parsed and counted; not manual stylesheet source reading or browser layout qualification.'},null,2)+'\n');console.log(JSON.stringify({locks,css:counts}));
