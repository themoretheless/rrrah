import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import vm from 'node:vm';
const source=readFileSync('.dedup-work/synology126-20261021/src/main.rs','utf8');
const start=source.indexOf('.then(paths => {')+'.then(paths => {'.length;
const end=source.indexOf('  }).catch(() => {});',start);
const body=source.slice(start,end);
function restore(paths) {
 const cards=['a','b'].map((path,i)=>({dataset:{path},classes:new Set([i?'del':'keep']),badge:{textContent:i?'DELETE':'KEEP'}}));
 for(const c of cards){c.classList={remove:x=>c.classes.delete(x),add:x=>c.classes.add(x)};c.querySelector=()=>c.badge;}
 let updates=0;
 const context={paths,document:{querySelectorAll:()=>cards},updateStats:()=>updates++};
 vm.runInNewContext('(function(paths){'+body+'})(paths)',context);
 return {cards,updates};
}
const empty=restore([]);
assert(empty.cards[1].classes.has('del'));assert.equal(empty.updates,0);
console.log('PASS original empty-draft callback leaves default deletion selection; saved keep-all intent is not restored');
const nonempty=restore(['a']);
assert(nonempty.cards[0].classes.has('del'));assert(nonempty.cards[1].classes.has('keep'));assert.equal(nonempty.updates,1);
console.log('PASS original nonempty-draft callback restores saved selection');
console.log('RESULT 2 passed; original callback in minimal DOM adapter, no browser/server/network/deletion');
