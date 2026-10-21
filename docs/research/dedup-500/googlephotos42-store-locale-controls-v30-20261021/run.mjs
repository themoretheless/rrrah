import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const root=path.resolve('.dedup-work/googlephotos42-20261021');
const runtime=path.join(root,'store-locale-runtime');await fs.mkdir(runtime,{recursive:true});
await fs.writeFile(path.join(runtime,'locales.mjs'),stripTypeScriptTypes(await fs.readFile(path.join(root,'scripts/lib/locales.ts'),'utf8')));
const {localeCodes,readMessages,messageProblems,MESSAGE_KEYS}=await import(pathToFileURL(path.join(runtime,'locales.mjs')));
const codes=localeCodes(root);assert.equal(codes.length,10);
for(const code of codes){const msgs=readMessages(code,root);assert.deepEqual(messageProblems(code,msgs),[]);assert.deepEqual(Object.keys(msgs).sort(),[...MESSAGE_KEYS].sort());}
console.log('PASS: original validator accepts all10pinned locale catalogs,5requiredkeys each; no browser/store validation');
