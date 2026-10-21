import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const text=readFileSync(process.argv[2],'utf8');
const start=text.indexOf('    const dedupKeys: string[] = []',text.indexOf('  const handleTrash = useCallback'));
const end=text.indexOf('    if (dedupKeys.length === 0) return',start);
assert.ok(start>0&&end>start);
// Execute the exact candidate-selection statements, without confirmation,
// command dispatch, account access or deletion.
const select=new Function('state','selectedGroupIds','getKept',stripTypeScriptTypes(text.slice(start,end))+'return {dedupKeys,mediaKeysToTrash};');
const group={id:'g',mediaKeys:['a','b','c'],originalMediaKey:'a'};
const state={groups:[group],mediaItems:Object.fromEntries(group.mediaKeys.map(k=>[k,{dedupKey:'d-'+k}]))};
for(const [name,kept,expected] of [['valid-kept-member',['a'],['b','c']],['absent-only-override',['old'],['a','b','c']],['empty-override',[],['a','b','c']]]){
 const actual=select(state,new Set(['g']),()=>new Set(kept)).mediaKeysToTrash;
 assert.deepEqual(actual,expected);console.log(JSON.stringify({name,actual,passed:true}));
}
