'use strict'
const assert = require('node:assert/strict')
const path = require('node:path')
const u = require(path.join(__dirname, 'evidence/mcollina--async-cache-dedupe/04-util.js'))
const sets = Array.from({length:64},(_,mask)=>Array.from({length:6},(_,i)=>i).filter(i=>mask&(1<<i)))
let searchCases=0,matchingCases=0
const differences=[]
for(const a of sets){
 for(let n=-1;n<=6;n++){assert.equal(u.bsearchIndex(a,n),a.indexOf(n));searchCases++}
 for(const b of sets){
  assert.deepEqual(u.findMatchingIndexes(a,b),b.map((x,i)=>a.includes(x)?i:-1).filter(i=>i>=0));matchingCases++
  const actual=u.findNotMatching(a,b),expected=b.filter(x=>!a.includes(x))
  if(JSON.stringify(actual)!==JSON.stringify(expected))differences.push({a,b,actual,expected})
 }
}
assert(differences.length>0)
console.log(JSON.stringify({binary_search_cases_passed:searchCases,matching_index_cases_passed:matchingCases,set_difference_cases:4096,set_difference_mismatches:differences.length,first_counterexamples:differences.slice(0,5),scope:'Whole original stdlib-only util module. Sorted unique integer arrays, six-element finite universe; no cache/server/Redis or performance qualification.'},null,2))
