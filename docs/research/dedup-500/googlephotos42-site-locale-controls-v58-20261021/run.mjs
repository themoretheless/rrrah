import {readFileSync,mkdtempSync,rmSync,mkdirSync,copyFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
const base=resolve('.dedup-work/googlephotos42-20261021');
const {build}=await import(pathToFileURL(join(base,'scripts/site-build.mjs')));
const out=mkdtempSync(join(tmpdir(),'gpdt-site-l10n-'));
try{
 const locales=join(out,'locales');mkdirSync(locales);
 for(const code of ['fr','it'])copyFileSync(join(base,'site-l10n',code+'.json'),join(locales,code+'.json'));
 const result=build({siteDir:join(base,'site'),l10nDir:locales,outDir:join(out,'built')});
 assert.deepEqual(result.slugs,['fr','it']);
 const keys=[];
 for(const code of result.slugs){
  const loc=JSON.parse(readFileSync(join(base,'site-l10n',code+'.json'),'utf8'));
  keys.push(Object.keys(loc.strings).sort());
  assert.ok(Object.values(loc.strings).every(s=>typeof s==='string'&&s.trim()));
  const html=readFileSync(join(out,'built',code,'index.html'),'utf8');
  assert.ok(html.includes('<html lang="'+code+'">'));
  assert.ok(html.includes('<main'));
  assert.ok(html.includes('href="../style.css"'));
  assert.ok(html.includes(loc.strings['Find duplicates, shown on a test page with generated images.']));
  console.log('PASS '+code+' '+keys.at(-1).length+' nonempty keys; original build language/rootasset/generated-demo-caption checks');
 }
 assert.deepEqual(keys[0],keys[1]);
 console.log('TOTAL2localecontrols passed; original builder run for only two cached locales. No browser rendering, deployment or full asset completeness assertion');
}finally{rmSync(out,{recursive:true,force:true});}
