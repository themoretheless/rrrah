// Authored harness: original complete saga module body, import/effect adapters only.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const sourcePath = process.argv[2];
const original = fs.readFileSync(sourcePath, 'utf8');
let imports = 0;
let adapted = original.replace(/import[\s\S]*?from '[^']+'\n/g, () => { imports++; return ''; });
assert.equal(imports, 3);
adapted = adapted.replace(/export default \[\s*watchForView,\s*watchForAction\s*\]/, '');
assert(!adapted.includes('export default'));
adapted += '\nthis.controls = {handleFilter, saveSetting, changeFilter, loadSettings};';
let serial = 0;
const context = {
 uniqueId: p => p + (++serial),
 createReducerConstants: () => ({app:{f:{ACTION_REQUESTED:'action',VIEW_LOADED:'loaded'}}}),
 takeLatest: (...args) => ({effect:'takeLatest', args}), takeEvery: (...args) => ({effect:'takeEvery',args}),
 select: fn => ({effect:'select',fn}), setViewData: (...args) => ({effect:'setViewData',args}),
 API:{patch:(...args)=>({effect:'patch',args}),get:(...args)=>({effect:'get',args})},
 showErrorToast: text => ({effect:'error',text}), showSuccessToast: text => ({effect:'success',text}),
 setStep: (...args) => ({effect:'step',args}),removeStep:(...args)=>({effect:'removeStep',args}),gettext:x=>x
};
vm.createContext(context);vm.runInContext(adapted,context,{filename:sourcePath});
const c=context.controls;
const oldFilter=[{id:'old',conditions:[{attribute:'filename',operator:'=',value:'a'}]}];
const editedFilter=[{id:'new',conditions:[{attribute:'filename',operator:'=',value:'b'}]}];
const view={settings:{ignored_files:[[{attribute:'filename',operator:'=',value:'a'}]]},filter:editedFilter};
const it=c.saveSetting('ignored_files',[[{attribute:'filename',operator:'=',value:'b'}]],oldFilter);
assert.equal(it.next().value.effect,'patch');
assert.equal(it.next({status:500}).value.effect,'error');
assert.equal(it.next().value.effect,'select');
const nested=it.next(view).value;assert.equal(typeof nested.next,'function');
assert.equal(nested.next().value.effect,'setViewData');assert(nested.next().done);assert(it.next().done);
assert.equal(view.filter,editedFilter);assert.equal(view.settings.ignored_files,oldFilter);
const success=c.saveSetting('ignored_files',[],oldFilter);
assert.equal(success.next().value.effect,'patch');
assert.equal(success.next({status:200,json:()=>({success:false})}).value.effect,'success');
assert(success.next().done);
const add=c.handleFilter({action:'filter_builder_add_or_condtion'},[]);
assert.equal(add.length,1);assert.deepEqual(Object.keys(add[0].conditions[0]),['id']);
let missingGroupThrew=false;
try {c.handleFilter({action:'filter_builder_save_condtion',payload:{group:'missing',id:'missing'}},[]);} catch(e) {missingGroupThrew=e.name==='TypeError';}
assert(missingGroupThrew);
console.log(JSON.stringify({source_sha256:crypto.createHash('sha256').update(original).digest('hex'),controls:4,passed:4,failed_save_editor_filter_retained:true,failed_save_settings_received_editor_group_shape:true,http200_error_body_not_inspected:true,new_group_condition_has_only_id:true,missing_group_throws_typeerror:true,scope:'Original full saga body with effect/import adapters; manually driven generator failure/success paths. No Redux scheduler, React render, HTTP or Nextcloud runtime.'},null,2));
