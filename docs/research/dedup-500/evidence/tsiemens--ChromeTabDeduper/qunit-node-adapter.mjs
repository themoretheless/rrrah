import assert from 'node:assert/strict';
import fs from 'node:fs';
const rows=[];
export const QUnit={test(name,callback){let pending=0,assertions=0;callback({ok(v,msg){assert.ok(v,msg);assertions++;},equal(a,b,msg){assert.equal(a,b,msg);assertions++;},deepEqual(a,b,msg){assert.deepEqual(a,b,msg);assertions++;},async(){pending++;return()=>{pending--;};}});assert.equal(pending,0,'callback completion');rows.push({name,assertions,passed:true});}};
export const test=QUnit.test;export const module=()=>{};
process.on('exit',()=>{fs.writeFileSync(new URL('./utils-original-results.json',import.meta.url),JSON.stringify({tests:rows,scope:'Four original callback bodies unchanged;Node assertion/QUnit registration adapter,actual original ES module;no browser or real QUnit runner.'},null,2)+'\n');console.log(JSON.stringify(rows));});
