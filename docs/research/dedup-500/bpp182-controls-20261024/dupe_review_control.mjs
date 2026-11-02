// Complete original module in isolated VM; API/toast/window/timer adapters.
import fs from 'node:fs';
import vm from 'node:vm';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const base=path.dirname(fileURLToPath(import.meta.url));
const source=path.join(base,'../evidence/Arkalogy--best-photo-picker/remaining-pinned/bpp/web/static/js/modules/dupe-review.mjs');
async function fixture(response,fail=false){
 const capture={opened:[],toasts:[],errors:[],requests:[]};
 const context=vm.createContext({window:{openCompareWithSibling:(...args)=>capture.opened.push(args)},setTimeout:()=>{throw new Error('unexpected timer')}});
 const api=new vm.SyntheticModule(['apiFetch'],function(){this.setExport('apiFetch',async url=>{capture.requests.push(url);if(fail)throw new Error('owned injected API error');return response;})},{context});
 const toast=new vm.SyntheticModule(['toast','toastError'],function(){this.setExport('toast',msg=>capture.toasts.push(msg));this.setExport('toastError',(...args)=>capture.errors.push(args.map(String)));},{context});
 const module=new vm.SourceTextModule(fs.readFileSync(source,'utf8'),{context,identifier:source});await module.link(name=>name==='./api-client.mjs'?api: name==='./toast.mjs'?toast:Promise.reject(new Error('Unexpected import '+name)));await module.evaluate();await module.namespace.startDupeReview();return {capture,module};
}
const results=[];
const f=await fixture({groups:[{photos:[{filepath:'a',aggregate_score:0.8},{filepath:'b',aggregate_score:0.4,similarity:0.125}]}]});
if(f.capture.opened.length!==1||f.capture.opened[0][1][0].similarity!==1)throw Error('counterexample not reproduced');
results.push({case:'all_group_members_assigned_similarity_one',input_similarity:0.125,observed_similarity:f.capture.opened[0][1][0].similarity,scope:'Original UI mapping with adapter API payload;not real backend payload/browser/rendering or mathematical similarity.'});
const empty=await fixture({groups:[]});if(empty.capture.opened.length||empty.module.namespace._getDupeGroups()!==null||empty.capture.toasts.length!==1)throw Error('empty group handling failed');results.push({case:'empty_group_list_does_not_open_compare',passed:true});
const failed=await fixture(null,true);if(failed.capture.opened.length||failed.capture.errors.length!==1)throw Error('API error handling failed');results.push({case:'API_failure_reports_error_without_opening_compare',passed:true});
const payload={scope:'Complete original ESM module under Node VM;explicit API/toast/window/timer adapters. No network,userfiles,UI actions,deletion or actual browser.',authored_controls_passed:results.length,results,node:process.version};fs.writeFileSync(path.join(base,'dupe-review-results.json'),JSON.stringify(payload,null,2)+'\n');console.log(JSON.stringify(payload,null,2));
