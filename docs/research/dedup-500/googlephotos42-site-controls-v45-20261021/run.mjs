import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';
const { build } = await import(pathToFileURL(resolve('.dedup-work/googlephotos42-20261021/scripts/site-build.mjs')));
const scratch = mkdtempSync(join(tmpdir(), 'gpdt-site-control-'));
try {
 const siteDir = join(scratch, 'source'), l10nDir = join(scratch, 'locales'), outDir = join(scratch, 'output');
 mkdirSync(siteDir); mkdirSync(l10nDir);
 writeFileSync(join(siteDir, 'sitemap.xml'), '<urlset></urlset>');
 const locale = { name:'French', code:'FR', pickerLabel:'Language', hreflang:'fr', htmlLang:'fr', ogLocale:'fr_FR', strings:{'Hello world':'Bonjour monde'} };
 writeFileSync(join(l10nDir,'fr.json'),JSON.stringify(locale));
 const source = body => '<html lang="en"><head></head><body>'+body+'</body></html>';
 writeFileSync(join(siteDir,'index.html'),source('<p>Hello world</p>'));
 build({siteDir,l10nDir,outDir});
 assert.ok(readFileSync(join(outDir,'fr/index.html'),'utf8').includes('<p>Bonjour monde</p>'));
 console.log('PASS complete key translates covered text');
 writeFileSync(join(siteDir,'index.html'),source('<p>Uncovered</p><p>Hello world</p>'));
 assert.throws(()=>build({siteDir,l10nDir,outDir}),/English text with no translation key/);
 console.log('PASS unrelated uncovered text rejected');
 writeFileSync(join(siteDir,'index.html'),source('<p>Hello</p><p>Hello world</p>'));
 build({siteDir,l10nDir,outDir});
 const html = readFileSync(join(outDir,'fr/index.html'),'utf8');
 assert.ok(html.includes('<p>Hello</p>')); assert.ok(html.includes('<p>Bonjour monde</p>'));
 console.log('PASS counterexample: substring-covered standalone English remains untranslated while build succeeds');
} finally { rmSync(scratch,{recursive:true,force:true}); }
