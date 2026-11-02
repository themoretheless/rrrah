// Full original classic script; isolated Node VM and recording browser adapters.
const fs=require('node:fs'),vm=require('node:vm'),path=require('node:path'),assert=require('node:assert/strict');
const source=path.join(__dirname,'../evidence/nosoop--tab-deduper/02-background.js');
function fixture(tabs,delayed=false){
 const events={},calls={query:[],move:[],update:[],remove:[]};
 const listener=n=>({addListener:f=>events[n]=f});
 const browser={tabs:{TAB_ID_NONE:-1,onUpdated:listener('updated'),onCreated:listener('created'),onRemoved:listener('removed'),query:q=>{calls.query.push(q);return Promise.resolve(tabs)},move:(...a)=>{calls.move.push(a);return delayed?new Promise(()=>{}):Promise.resolve()},update:(...a)=>{calls.update.push(a);return Promise.resolve()},remove:ids=>{calls.remove.push(Array.from(ids));return Promise.resolve()}},notifications:{create:()=>Promise.resolve('owned-notification'),clear:()=>Promise.resolve()},i18n:{getMessage:x=>x},storage:{onChanged:listener('storage'),sync:{get:(_key,cb)=>{cb?.({});return Promise.resolve({})}}}};
 const context=vm.createContext({browser,URL,setTimeout:()=>0});vm.runInContext(fs.readFileSync(source,'utf8'),context,{filename:source});return {calls,events,run:async tab=>{context.ownedInput=tab;await vm.runInContext('checkDuplicateTabs(ownedInput)',context)}};
}
const tab=(id,url='https://example.invalid/a',cookieStoreId='normal',active=true)=>({id,url,cookieStoreId,active,index:id,windowId:1});
(async()=>{const out=[];
 let f=fixture([tab(1),tab(2)]);await f.run(tab(2));assert.deepEqual(f.calls.remove,[[2]]);assert.equal(f.calls.move[0][0],1);out.push({case:'same_url_cookie_keeps_lowest_id',recorded_removal:f.calls.remove});
 f=fixture([tab(1,undefined,'other'),tab(2)]);await f.run(tab(2));assert.equal(f.calls.remove.length,0);out.push({case:'different_cookie_stores_remain',passed:true});
 f=fixture([tab(1,'https://example.invalid/a#x'),tab(2)]);await f.run(tab(2));assert.equal(f.calls.remove.length,0);out.push({case:'different_fragment_not_equal_url',passed:true});
 f=fixture([tab(1),tab(2)]);f.events.created({...tab(2),openerTabId:1,status:'loading'});await f.run(tab(2));assert.equal(f.calls.query.length,0);out.push({case:'explicit_opened_loading_tab_protected',passed:true});
 f=fixture([tab(1),tab(2,undefined,'normal',false)]);await f.run(tab(2,undefined,'normal',false));assert.equal(f.calls.update.length,0);assert.deepEqual(f.calls.remove,[[2]]);out.push({case:'background_duplicate_does_not_focus',passed:true});
 f=fixture([tab(1),tab(2)]);f.events.storage({checkMultiWindow:{newValue:true}},'sync');await f.run(tab(2));assert.equal(f.calls.query[0].currentWindow,null);out.push({case:'multi_window_query_shape',currentWindow:f.calls.query[0].currentWindow,qualification:'Recording adapter does not validate Firefox API schema or actual window visibility.'});
 f=fixture([tab(1),tab(2)],true);await f.run(tab(2));assert.deepEqual(f.calls.remove,[[2]]);assert.equal(f.calls.move.length,1);out.push({case:'remove_called_while_move_promise_unresolved',recorded_removal:f.calls.remove,qualification:'Ordering confirmed under pending adapter;no real browser move failure or tab removal.'});
 f=fixture([]);f.events.updated(2,{url:'about:blank'},tab(2,'about:blank'));assert.equal(f.calls.query.length,0);out.push({case:'blank_navigation_event_ignored',passed:true});
 const p={scope:'Complete unchanged original background.js;realNodeVM,explicit browser/query/storage/notification/timer adapters. No real browser,extensions,tabs,network or deletion. Query adapter returns supplied records,not actual Firefox match-pattern execution.',authored_controls_passed:out.length,results:out,node:process.version};fs.writeFileSync(path.join(__dirname,'results.json'),JSON.stringify(p,null,2)+'\n');console.log(JSON.stringify(p,null,2));})();
