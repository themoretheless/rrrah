import {extractImports} from './parseFile.mts';import assert from 'node:assert/strict';
const ts=extractImports(`const example = 'import "./phantom"';`,'typescript');assert.deepEqual(ts.map(x=>x.specifier),['./phantom']);
const py=extractImports('"""\nimport ghost\n"""','python');assert.deepEqual(py.map(x=>x.specifier),['ghost']);
const go=extractImports('package p\n/* import "phantom" */','go');assert.deepEqual(go.map(x=>x.specifier),['phantom']);
console.log(JSON.stringify({runtime:process.version,controls_passed:3,findings:['TS import syntax inside string literal counted','Python import inside docstring counted','Go import inside blockcomment counted'],scope:'Original regex parser+originalresult/errors modules,onlyruntimeimportlocationschanged;ownedstrings;no compiler or wholemanifestexecution'},null,2));
