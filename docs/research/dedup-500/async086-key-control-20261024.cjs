'use strict'
const assert=require('node:assert/strict'),path=require('node:path')
const {createCache}=require(path.resolve(__dirname,'../../../.dedup-work/async086-20261024'))
;(async()=>{
 const c=createCache();let calls=0
 c.define('echo',async value=>{calls++;return {value,type:typeof value}})
 const result=await Promise.all([c.echo(42),c.echo('42')])
 assert.equal(calls,1);assert.deepEqual(result,[{value:42,type:'number'},{value:42,type:'number'}])
 const cleared=[]
 for(const zero of [0,false,'']){
  const cache=createCache({ttl:60});const seen=new Map()
  cache.define('read',async k=>{seen.set(k,(seen.get(k)||0)+1);return k})
  await cache.read(zero);await cache.read('unrelated');assert.equal(seen.get('unrelated'),1)
  await cache.clear('read',zero);await cache.read('unrelated');assert.equal(seen.get('unrelated'),2)
  cleared.push({key:zero,unrelated_recomputed:true})
 }
 console.log(JSON.stringify({numeric_and_string_same_text_share_inflight_result:result,underlying_calls:calls,falsey_clear_cases:cleared,scope:'Original whole package with actual memory dependencies. No Redis, browser or original test suite; observed finite argument identity and clear behavior only.'},null,2))
})().catch(e=>{console.error(e);process.exitCode=1})
