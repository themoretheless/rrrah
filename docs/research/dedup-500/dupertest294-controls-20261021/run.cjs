// Authored controls against unchanged original source; not original Jasmine suite.
const assert=require('node:assert/strict');
const request=require('../../../../.dedup-work/dupertest294-20261021/lib/dupertest');
let response;
request((req,res)=>res.send(req.params.id+1)).params({id:2}).end(x=>response=x);
assert.equal(response,3);console.log('PASS original action receives parameters and sends response');
let routed=0;
request((req,res,next)=>next()).next(()=>routed++).end();
assert.equal(routed,1);console.log('PASS next callback independently observed');
const error=new Error('synthetic');let observed;
request((req,res,next)=>next(error)).errNext(e=>observed=e).end();
assert.equal(observed,error);console.log('PASS error-next callback independently observed');
assert.throws(()=>request().end(),/No action defined/);
console.log('PASS missing action explicitly rejected');
console.log('4 authored controls passed; no duplicate-content algorithm exists here');
