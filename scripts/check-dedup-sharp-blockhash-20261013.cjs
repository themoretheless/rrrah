// Execute pinned wrapper unchanged with explicit fake Sharp/blockhash providers.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const {EventEmitter} = require('node:events');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const source = '/Users/themoretheless/.codex/tmp/rrrah-sharp-blockhash-source-20261013/index.js';
let calls = [];
function Sharp(input, options) {
  EventEmitter.call(this);
  this.options = Object.assign({streamIn:false, streamOut:false}, options);
}
require('node:util').inherits(Sharp, EventEmitter);
Sharp.prototype._read = function() {};
const backend = {
  pipelineAsync: async options => { calls.push({kind:'pipeline'}); return Buffer.alloc(12); },
  metadataAsync: async options => ({width:100,height:100}),
};
class FakeBluebird extends Promise {}
FakeBluebird.promisifyAll = value => value;
const blockhash = {blockhashData: (data,bits,method) => {
  calls.push({kind:'blockhash',width:data.width,height:data.height,bytes:data.data.length,bits,method});
  return 'mock-hash';
}};
const moduleObject = {exports:{}};
const dependencies = {'sharp':Sharp, 'bluebird':FakeBluebird, 'blockhash':blockhash,
  'util':require('node:util'), 'sharp/build/Release/sharp':backend};
vm.runInNewContext(fs.readFileSync(source,'utf8'), {module:moduleObject,require:n=>{
  assert.ok(n in dependencies); return dependencies[n];
},Buffer}, {filename:source});
const shash = moduleObject.exports;
const rows = [];
async function check(name, run) { calls=[]; await run(); rows.push({name,passed:true,calls:JSON.parse(JSON.stringify(calls))}); }
(async()=>{
  await check('no_input_constructor_leaves_options_absent', async()=>{
    assert.throws(()=>shash().toBlockhash(),error=>error.name==='TypeError' && /blockhashOut/.test(error.message));
  });
  await check('constructor_blockhash_options_reset_to_defaults', async()=>{
    assert.equal(await shash('mock-input',{blockhashBits:16}).toBlockhash(),'mock-hash');
    assert.equal(calls.at(-1).bits,8);
  });
  await check('callback_only_overload_discards_callback', async()=>{
    let count=0; await shash('mock-input').toBlockhash(()=>count++); assert.equal(count,0);
  });
  await check('explicit_callback_for_file_input_is_never_called', async()=>{
    let count=0; await shash('mock-input').toBlockhash({},()=>count++); assert.equal(count,0);
  });
  await check('stream_callback_and_promise_schedule_two_inferences', async()=>{
    let count=0; const image=shash('mock-input',{streamIn:true});
    const result=image.toBlockhash({},()=>count++); image.emit('finish'); await result;
    await new Promise(resolve=>setImmediate(resolve));
    assert.equal(count,1); assert.equal(calls.filter(x=>x.kind==='pipeline').length,2);
  });
  await check('raw_pipeline_payload_forwarded_with_input_metadata_dimensions', async()=>{
    await shash('mock-input').toBlockhash({blockhashBits:16});
    const row=calls.at(-1); assert.equal(row.bits,16);
    assert.deepEqual([row.width,row.height,row.bytes],[100,100,12]);
  });
  const sha=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
  const output=path.join(root,'docs/research/dedup-500/sharp-blockhash-controls-20261013.json');
  assert.ok(!fs.existsSync(output));
  fs.writeFileSync(output,JSON.stringify({status:'passed_wrapper_controls_with_explicit_mocks',
    controls:rows,node_version:process.version,upstream_tests_executed:0,
    scope:'Unmodified pinned wrapper executed in VM. Sharp, Bluebird adapter and blockhash are mocks. No real decoding, perceptual hashing, upstream Mocha suite or fixture-quality claim.',
    pins:{[source]:sha(source),[__filename]:sha(__filename)}},null,2)+'\n');
  process.stdout.write('6 wrapper controls passed\n');
})().catch(error=>{console.error(error);process.exitCode=1;});
