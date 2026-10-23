'use strict'
const assert=require('node:assert/strict'),path=require('node:path')
const root=path.resolve(__dirname,'../../../.dedup-work/async086-20261024')
const {createCache}=require(root)
const Memory=require(path.join(root,'src/storage/memory'))
const sets=Array.from({length:7},(_,m)=>['a','b','c'].filter((_,i)=>(m+1)&(1<<i)))
const mismatches=[];let cases=0
for(const old of sets)for(const current of sets)for(const ref of ['a','b','c']){
 const s=new Memory({invalidation:true});s.set('key','old',60,old);s.set('key','new',60,current)
 s.invalidate(ref);const actual=s.get('key'),expected=current.includes(ref)?undefined:'new'
 if(actual!==expected)mismatches.push({old,current,ref,actual:actual??null,expected:expected??null})
 cases++
}
;(async()=>{
 const cache=createCache({ttl:60});let finish,started;const gate=new Promise(r=>{finish=r});const ready=new Promise(r=>{started=r});let calls=0
 cache.define('work',async()=>{calls++;started();await gate;return 'old-generation'})
 const a=cache.work('key'),b=cache.work('key');assert.equal(a,b);await ready;assert.equal(calls,1)
 await cache.clear('work','key');finish();assert.equal(await a,'old-generation')
 const after=await cache.work('key');assert.equal(after,'old-generation');assert.equal(calls,1)
 console.log(JSON.stringify({reference_update_cases:cases,reference_update_mismatches:mismatches.length,first_counterexamples:mismatches.slice(0,6),concurrent_calls_shared_one_promise:true,clear_during_work_allows_completed_value_to_repopulate:true,scope:'Whole original package and memory storage with actual dependencies, owned in-process fixtures. No Redis connection, browser runtime, cancellation guarantee or performance proof.'},null,2))
})().catch(e=>{console.error(e);process.exitCode=1})
