import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import assert from 'node:assert/strict';
const input=process.argv[2];
const original=readFileSync(input,'utf8');
// Remove only imports. Pure exported functions below retain their original bodies.
const isolated=original.replace(/^import .*;\r?$/gm,'');
const js=stripTypeScriptTypes(isolated);
const m=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
const report=(name,actual,expected)=>{assert.deepEqual(actual,expected);console.log(JSON.stringify({name,actual,passed:true}));};
const item=(mediaKey,timestamp=0)=>({mediaKey,dedupKey:mediaKey,thumb:'unused',timestamp,creationTimestamp:0});
const vec=(angle)=>new Float32Array([Math.cos(angle),Math.sin(angle)]);
report('adjacency-misses-separated-identical-vectors',m.communityDetection([vec(0),vec(Math.PI/2),vec(0)],0.99,[0,1,2]),[]);
report('adjacent-chain-is-not-a-clique',m.communityDetection([vec(0),vec(0.1),vec(0.2)],0.99,[0,1,2]),[[0,1,2]]);
const items=['a','b','c'].map(x=>item(x));
report('union-find-chain-is-not-a-clique',m.withinGroupDuplicates(items,new Map(items.map((x,i)=>[x.mediaKey,vec(i*0.1)])),0.99,0)[0].mediaKeys,['a','b','c']);
report('window-boundary-misses-nearby-timestamps',m.groupByTimestamp([item('a',999),item('b',1000)],1000),[]);
report('dedup-key-keeps-first-even-if-later-quality-is-better',m.dedupeByDedupKey([{...item('a'),dedupKey:'same',isOriginalQuality:false},{...item('b'),dedupKey:'same',isOriginalQuality:true}]).map(x=>x.mediaKey),['a']);
report('shorter-later-vector-silently-fails-comparison',m.communityDetection([new Float32Array([1,0]),new Float32Array([1])],0.99),[]);
for(const k of [1,3,50,51,80]){
 const a=Float32Array.from({length:80},(_,i)=>Math.sin(i*17));
 report('finite-top-k-'+k,m.topK(a,k).values,[...a].sort((a,b)=>b-a).slice(0,k));
}
