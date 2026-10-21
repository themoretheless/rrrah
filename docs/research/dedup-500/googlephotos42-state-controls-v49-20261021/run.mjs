import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import assert from 'node:assert/strict';
const original = readFileSync('.dedup-work/googlephotos42-20261021/scripts/store-state.mjs','utf8');
const source = original.replace(/^import .* from 'node:[^']+'\n/gm,'');
function run(command, tag, reads) {
 const output=[], writes=[];
 runInNewContext(source,{
  process:{argv:['node','store-state.mjs',command,'cws',tag],env:{STATE_FILE:'synthetic.json'},exit(code){throw Error('exit '+code)}},
  execSync(cmd){ if (!(cmd in reads)) throw Error('unavailable'); return reads[cmd]; },
  writeFileSync(path,data){writes.push({path,data:JSON.parse(data)});},
  console:{log(x){output.push(x)},error(x){output.push(x)}}
 });
 return {output,writes};
}
const primary='git show origin/store-state:state.json', fallback='git show FETCH_HEAD:state.json';
assert.deepEqual(run('pending','v3.6.0',{[primary]:'{"cws":"v3.6.0"}'}).output,['up-to-date']);
console.log('PASS same tag is up-to-date');
assert.deepEqual(run('pending','v3.5.0',{[primary]:'{"cws":"v3.6.0"}'}).output,['pending']);
console.log('PASS older target is pending (exact tag equality, no monotonic version ordering)');
assert.equal(run('set','v3.5.0',{[primary]:'{"cws":"v3.6.0","edge":"v3.6.0"}'}).writes[0].data.cws,'v3.5.0');
console.log('PASS older target overwrites newer stored tag');
assert.deepEqual(run('set','v3.7.0',{}).writes[0].data,{cws:'v3.7.0'});
console.log('PASS unreadable state falls back to empty object before set');
assert.deepEqual(run('set','v3.7.0',{[primary]:'invalid JSON',[fallback]:'{"edge":"old"}'}).writes[0].data,{edge:'old',cws:'v3.7.0'});
console.log('PASS invalid primary uses FETCH_HEAD fallback');
assert.throws(()=>run('pending','v3.7.0',{[primary]:'null'}),/null/);
console.log('PASS valid JSON null lacks schema validation and throws');
console.log('TOTAL 6 passed; original CLI body with explicit fake process/git-show/file-writer adapters; no git/filesystem writes by original');
