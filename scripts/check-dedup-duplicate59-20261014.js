const fs=require('fs'), vm=require('vm'), assert=require('assert');
const source=fs.readFileSync(process.argv[2],'utf8');
function run(n,failRead=-1,failDuplicate=-1){
 const actions=[],copies=[],moves=[],alerts=[];let didThrow=false;
 const docs=Array.from({length:n},(_,i)=>{let layer={name:'layer-'+i};return {id:i,get activeLayer(){if(i===failRead&&!didThrow){didThrow=true;throw Error('synthetic missing layer');}return layer;},set activeLayer(v){layer=v;}};});
 if(n)docs[0].activeLayer={name:'source',duplicate(target){if(target.id===failDuplicate)throw Error('synthetic duplicate failure');copies.push(target.id);target.activeLayer={name:'copy',move(layer){moves.push([target.id,layer.name]);}};}};
 const context={app:{documents:docs,activeDocument:docs[0]},ElementPlacement:{PLACEATBEGINNING:1,PLACEBEFORE:2},DialogModes:{NO:0},ActionDescriptor:function(){this.putReference=()=>{};},ActionReference:function(){this.putEnumerated=()=>{};this.putClass=()=>{};},charIDToTypeID:s=>s,stringIDToTypeID:s=>s,executeAction:id=>actions.push(id),alert:s=>alerts.push(s)};
 let error=null;try{vm.runInNewContext(source,context,{timeout:1000});}catch(e){error=e.message;}
 return {n,failRead,failDuplicate,actions,copies,moves,alerts,error,restored:context.app.activeDocument===docs[0]};
}
const rows=[];let r;
r=run(0);assert.deepStrictEqual(r.actions,[]);assert.deepStrictEqual(r.alerts,[]);rows.push(r);
r=run(1);assert.deepStrictEqual(r.copies,[]);assert.deepStrictEqual(r.actions,['Mk  ','ungroupLayersEvent']);assert.strictEqual(r.alerts.length,1);rows.push(r);
r=run(4);assert.deepStrictEqual(r.copies,[1,2,3]);assert.deepStrictEqual(r.moves,[[1,'layer-1'],[2,'layer-2'],[3,'layer-3']]);assert.strictEqual(r.actions.filter(x=>x==='ungroupLayersEvent').length,4);assert(r.restored);rows.push(r);
r=run(3,1);assert.deepStrictEqual(r.moves,[[2,'layer-2']]);assert.strictEqual(r.error,null);rows.push(r);
r=run(3,2);assert.deepStrictEqual(r.moves,[[1,'layer-1'],[2,'layer-1']]);assert.strictEqual(r.error,null);rows.push(r);
r=run(3,-1,2);assert.strictEqual(r.error,'synthetic duplicate failure');assert.deepStrictEqual(r.copies,[1]);assert.strictEqual(r.alerts.length,0);assert.strictEqual(r.actions.filter(x=>x==='ungroupLayersEvent').length,1);rows.push(r);
process.stdout.write(JSON.stringify({status:'verified_six_controlled_photoshop_scenarios',rows,scope:'Original43line source unmodified in Node vm with explicit synthetic Photoshop adapter. No actual Photoshop execution, UI edits, image corpus, detection or file identity qualification.'},null,2)+'\n');
