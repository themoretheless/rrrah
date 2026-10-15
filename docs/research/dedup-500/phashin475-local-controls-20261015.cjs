// Unchanged whole index.js in VM with explicit Express/WebDriver/filesystem/timer adapters; no server/browser/network.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const archive=JSON.parse(fs.readFileSync(path.join(__dirname,'phashin475-full-source-20261015.json'),'utf8'));const source=archive.blobs.find(x=>x.path==='index.js').content;
let passed=0;
function setup(){
 const s={requires:[],routes:{},remotes:[],ends:0,pauses:[],saves:[],intervals:[],timeouts:[],cleared:[],deleted:[],errors:[],now:0,exists:false,listen:[]};
 const app={use(){},get(p,f){s.routes.get=f},post(p,f){s.routes.post=f},listen(p,f){s.listen.push(p);f()}};
 const webdriverio={remote(opts){const b={callbacks:[],options:opts,init(){return this},setViewportSize(v,f){this.viewport=[v,f];return this},url(u){this.urlValue=u;return this},then(f){this.callbacks.push(f);return this},catch(f){this.errorHandler=f;return this},pause(ms){s.pauses.push(ms);return 'pause-result'},saveScreenshot(p){s.saves.push(p);return 'save-result-unawaited'},end(){s.ends++}};s.remotes.push(b);return b}};
 const adapters={del:x=>s.deleted.push(...x),express:()=>app,fs:{existsSync:()=>s.exists},path,mkdirp:()=>{},'node-uuid':{v1:()=>`synthetic-${s.remotes.length}`},webdriverio,'body-parser':{json:()=>()=>{}}};
 const context={require:n=>{s.requires.push(n);assert.ok(Object.hasOwn(adapters,n));return adapters[n]},__dirname:'/owned-synthetic-fixture',process:{env:{}},console:{log(){}},Date:class{getTime(){return s.now}},setInterval:(f,ms)=>{s.intervals.push({f,ms});return s.intervals.length},clearInterval:x=>s.cleared.push(x),setTimeout:(f,ms)=>s.timeouts.push({f,ms})};vm.createContext(context);vm.runInContext(source,context);s.context=context;
 s.response=()=>({statusValue:0,jsonValue:undefined,sent:[],active:false,status(x){this.statusValue=x;return this},json(x){this.jsonValue=x;return this},sendStatus(x){this.statusValue=x},sendFile(p){this.active=true;this.sent.push(p)}});
 s.request=(method,url)=>{const res=s.response();s.routes[method]({query:{url},body:{url}},res,e=>s.errors.push(e));return res};
 s.advance=b=>{assert.equal(b.callbacks.length,3);assert.equal(b.callbacks[0](),'pause-result');assert.equal(b.callbacks[1](),undefined);b.callbacks[2]()};return s;
}
function check(name,fn){fn();passed++;console.log('PASS',name)}
check('GET missingURL400 before browser allocation',()=>{const s=setup(),r=s.request('get',undefined);assert.equal(r.statusValue,400);assert.equal(r.jsonValue,'url missing');assert.equal(s.remotes.length,0)});
check('POST missingURL400 before browser allocation',()=>{const s=setup(),r=s.request('post','');assert.equal(r.statusValue,400);assert.equal(s.remotes.length,0)});
check('GET target URL forwarded unchanged to browser adapter',()=>{const s=setup();s.request('get','https://example.invalid');assert.equal(s.remotes[0].urlValue,'https://example.invalid')});
check('POST truthy object URL forwarded without type validation',()=>{const s=setup(),obj={unexpected:true};s.request('post',obj);assert.equal(s.remotes[0].urlValue,obj)});
check('screenshot save callback discards operation result',()=>{const s=setup();s.request('get','synthetic');const b=s.remotes[0];assert.equal(b.callbacks[0](),'pause-result');assert.equal(b.callbacks[1](),undefined);assert.equal(s.saves.length,1);assert.equal(s.ends,0)});
check('timeout sends500 and closes browser after15000ms',()=>{const s=setup(),r=s.request('get','synthetic');s.advance(s.remotes[0]);s.now=15001;s.intervals[0].f();assert.equal(r.statusValue,500);assert.equal(s.ends,1);assert.equal(s.cleared.length,1)});
check('deadline branch wins even when screenshot already exists',()=>{const s=setup(),r=s.request('get','synthetic');s.advance(s.remotes[0]);s.now=15001;s.exists=true;s.intervals[0].f();assert.equal(r.statusValue,500);assert.equal(r.sent.length,0)});
check('successful dispatch closes browser before response transfer completes',()=>{const s=setup(),r=s.request('get','synthetic');s.advance(s.remotes[0]);s.exists=true;s.now=100;s.intervals[0].f();assert.equal(r.sent.length,1);assert.equal(s.ends,1);assert.equal(r.active,true)});
check('fixed25000ms cleanup may delete path during active transfer',()=>{const s=setup(),r=s.request('get','synthetic');s.advance(s.remotes[0]);s.exists=true;s.intervals[0].f();assert.equal(s.timeouts[0].ms,25000);s.timeouts[0].f();assert.equal(s.deleted.length,1);assert.equal(r.active,true)});
check('registered chain error handler forwards error with no explicit browser teardown',()=>{const s=setup();s.request('get','synthetic');const err=new Error('injected');s.remotes[0].errorHandler(err);assert.equal(s.errors[0],err);assert.equal(s.ends,0);assert.equal(s.intervals.length,0)});
check('three requests allocate three independent unbounded browser sessions',()=>{const s=setup();for(let i=0;i<3;i++)s.request('get','synthetic');assert.equal(s.remotes.length,3);assert.equal(s.ends,0)});
check('whole module requires no active phash comparator',()=>{const s=setup();assert.equal(s.requires.includes('phash'),false);assert.equal(s.listen.length,1);assert.equal(s.listen[0],3000)});
console.log(JSON.stringify({passed,failed:0,scope:'Unchanged whole index.js VM; explicit Express/WebDriver/browser chain scheduling/filesystem/clock/timer/response adapters. Native callback body/control-flow proof only, no actual WebDriver thenable promise integration, browser screenshot, response stream, network/server/dependency/install/performance qualification.'}));
