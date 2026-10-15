import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
const root=process.cwd(),source=path.join(root,'.dedup-work/tada51-20261015/source');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const rows=JSON.parse(fs.readFileSync(path.join(root,'docs/research/dedup-500/tada51-full-file-pins-20261015.json'),'utf8'));
for(const r of rows) assert.equal(fs.readFileSync(path.join(source,r.path),'utf8'),r.content);
function env(modern=false){
 const timers=new Map(),events=[],observations=[],unobservations=[];let next=0;
 const window={innerHeight:100,innerWidth:100,addEventListener:(...x)=>events.push(x),getComputedStyle:e=>({display:e.display??'block'})};
 class Observer{constructor(callback,options){this.callback=callback;this.options=options;}observe(e){observations.push(e);}unobserve(e){unobservations.push(e);}}
 if(modern){window.IntersectionObserver=Observer;window.IntersectionObserverEntry=function(){};window.IntersectionObserverEntry.prototype.isIntersecting=true;}
 const context=vm.createContext({window,document:{documentElement:{clientHeight:100,clientWidth:100},querySelectorAll:()=>[]},IntersectionObserver:Observer,setTimeout:(f,d)=>{timers.set(++next,{f,d});return next;},clearTimeout:id=>timers.delete(id),module:{exports:{}},exports:{}});
 return {context,window,events,observations,unobservations,timers,flush(){const pending=[...timers.values()];timers.clear();for(const x of pending)x.f();}};
}
async function esm(e){const cache=new Map();async function load(file){file=path.resolve(file);if(cache.has(file))return cache.get(file);const m=new vm.SourceTextModule(fs.readFileSync(file,'utf8'),{context:e.context,identifier:file});cache.set(file,m);await m.link(async (id,parent)=>{assert.ok(id.startsWith('.'));let resolved=path.resolve(path.dirname(parent.identifier),id);if(fs.existsSync(resolved)&&fs.statSync(resolved).isDirectory())resolved=path.join(resolved,'index.js');else if(!resolved.endsWith('.js'))resolved+='.js';assert.ok(resolved.startsWith(source+path.sep));return load(resolved);});return m;}
 const main=await load(path.join(source,'src/index.js'));await main.evaluate();return {api:main.namespace.default,load};}
function el(tag='IMG',url='image.png',rect={top:0,bottom:10,left:0,right:10}){const attrs=new Map([['data-src',url],['src','placeholder.png']]);return {tagName:tag,style:{},display:'block',getAttribute:k=>attrs.has(k)?attrs.get(k):null,setAttribute:(k,v)=>attrs.set(k,String(v)),removeAttribute:k=>attrs.delete(k),getBoundingClientRect:()=>rect,attrs};}
const cases=[];const check=(name,fn)=>{fn();cases.push(name);console.log('PASS '+name);};
for(const flavor of ['source-esm','tracked-umd']){
 for(const modern of [false,true]){
  const e=env(modern);let api;
  if(flavor==='source-esm')api=(await esm(e)).api;else {vm.runInContext(fs.readFileSync(path.join(source,'build/tada.js'),'utf8'),e.context);api=e.context.module.exports.default;}
  const prefix=flavor+'/'+(modern?'observer':'legacy')+'/';
  const custom={attribute:'data-custom',delay:0,threshold:'0px',callback:()=>{throw Error('callback should be ignored by bug');}};
  check(prefix+'public_setup_nests_options_instead_of_overriding_defaults',()=>{api.setup(custom);assert.equal(api.config.attribute,'data-src');assert.equal(api.config.delay,50);assert.equal(api.config.threshold,'20%');assert.equal(api.config.callback,undefined);assert.equal(api.config.config,custom);});
  check(prefix+'backend_detection_matches_explicit_environment',()=>assert.equal(api.useIntersectionObserver,modern));
  const a=el();
  check(prefix+'actual_load_updates_src_and_removes_marker',()=>{api.add(a);if(modern)api.instance.execute({isIntersecting:true,target:a});else e.flush();assert.equal(a.getAttribute('src'),'image.png');assert.equal(a.getAttribute('data-src'),null);});
  check(prefix+'repeated_setup_replaces_instance',()=>{const old=api.instance;api.setup();assert.notEqual(api.instance,old);});
 }
}
const e=env();const {load}=await esm(e);const legacy=await load(path.join(source,'src/legacy/index.js'));const Legacy=legacy.namespace.default;
const defaultConfig=()=>({attribute:'data-src',delay:0,threshold:'0px'});
check('legacy_identity_dedup_does_not_compare_image_content',()=>{const t=new Legacy(defaultConfig()),a=el(),b=el();t.addElement(a);t.addElement(a);t.addElement(b);assert.equal(t.elements.length,2);});
check('legacy_viewport_spanning_element_falsely_not_visible',()=>{const t=new Legacy({...defaultConfig(),threshold:'0%'});t.init();assert.equal(t.valid(el('IMG','x',{top:-50,bottom:150,left:-50,right:150})),false);});
check('legacy_pixel_threshold_string_concatenation_expands_positive_boundary',()=>{const t=new Legacy(defaultConfig());t.init();assert.equal(t.thresholdHeight,'0');assert.equal(t.valid(el('IMG','x',{top:150,bottom:160,left:150,right:160})),true);});
check('legacy_hidden_own_display_is_filtered',()=>{const t=new Legacy(defaultConfig());t.init();const a=el();a.display='none';assert.equal(t.valid(a),false);});
check('legacy_missing_data_attribute_becomes_literal_null_src',()=>{const t=new Legacy(defaultConfig());const a=el();a.removeAttribute('data-src');t.add(a);assert.equal(a.getAttribute('src'),'null');});
check('legacy_case_tag_contract_is_exact_IMG',()=>{const t=new Legacy(defaultConfig()),a=el('img');t.show(a);assert.equal(a.style.backgroundImage,'url(image.png)');assert.equal(a.getAttribute('src'),'placeholder.png');});
check('legacy_failed_initialization_latches_initialized',()=>{const t=new Legacy({...defaultConfig(),threshold:'invalid'});assert.throws(()=>t.init());assert.equal(t.initialized,true);t.config.threshold='0px';t.init();assert.equal(t.thresholdHeight,undefined);});
check('legacy_resize_recalculates_percentage_threshold',()=>{const t=new Legacy({...defaultConfig(),threshold:'20%'});t.init();e.window.innerHeight=200;t.onResize();assert.equal(t.thresholdHeight,40);e.window.innerHeight=100;});
const d=await load(path.join(source,'src/utils/debounce.js'));const debounce=d.namespace.default;
check('debounce_zero_delay_preserves_original_callable',()=>{const method=()=>3;assert.equal(debounce(method,0),method);});
check('debounce_coalesces_calls_and_discards_arguments',()=>{e.timers.clear();let calls=0,args;const f=debounce((...a)=>{calls++;args=a;},50);f(1);f(2);assert.equal(e.timers.size,1);e.flush();assert.equal(calls,1);assert.deepEqual(args,[]);});
const modern=env(true);const mods=await esm(modern);const observerModule=await mods.load(path.join(source,'src/intersection-observer/index.js'));const Observer=observerModule.namespace.default;
check('observer_nonintersecting_entry_does_not_load',()=>{const t=new Observer(defaultConfig()),a=el();t.execute({isIntersecting:false,target:a});assert.equal(a.getAttribute('src'),'placeholder.png');});
check('observer_callback_runs_immediately_after_assignment',()=>{const a=el();let count=0;const t=new Observer({...defaultConfig(),callback:x=>{assert.equal(x,a);assert.equal(x.getAttribute('src'),'image.png');count++;}});t.execute({isIntersecting:true,target:a});assert.equal(count,1);assert.equal(modern.unobservations.at(-1),a);});
check('observer_replayed_queued_entry_overwrites_src_with_null',()=>{const t=new Observer(defaultConfig()),a=el();t.execute({isIntersecting:true,target:a});t.execute({isIntersecting:true,target:a});assert.equal(a.getAttribute('src'),'null');});
console.log(JSON.stringify({status:'passed',controls:cases.length,cases,node:process.version,source_pins:Object.fromEntries(rows.map(r=>[r.path,sha(fs.readFileSync(path.join(source,r.path)))])),adapters:'OriginalESM modules loaded unchanged through nativevm.SourceTextModule; trackedUMD loaded unchanged through nativevm. Explicit syntheticDOM/IntersectionObserver/timer/windowenvironment, controlledqueuedcallback replay; no realbrowser/image/networkload.',limits:'No original tests tracked; no fullpackage install/Webpack rebuild/nativebrowser/network/visualgroundtruth or libraryperformance qualification. No source adopted.'}));
