// Complete unchanged four published scripts, real jsdom DOM, recording-only click handlers.
const fs=require('fs'),path=require('path'),vm=require('vm'),crypto=require('crypto'),assert=require('assert');
const R=path.resolve(__dirname,'..'),W=path.join(R,'.dedup-work/lastfm56-20261016'),S=path.join(W,'source'),A=path.join(W,'native-controls');fs.mkdirSync(A);
const {JSDOM}=require(path.join(W,'node-tools/node_modules/jsdom'));
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const files=['lastfm-smart-deduper.js','other/compressed-js/compressed-mode0-silent.js','other/compressed-js/compressed-mode1-balanced.js','other/compressed-js/compressed-mode2-safe.js'];
const track=(song,min,artist='artist')=>({song,min,artist});
const cases=[
 ['author1',[track('a',0),track('a',0),track('a',0)]],
 ['author2',[track('a',0),track('a',0),track('a',5)]],
 ['author3',[track('a',0),track('b',0),track('b',5)]],
 ['author4',[track('a',0),track('b',0),track('a',0)]],
 ['author5',[track('a',0),track('b',4),track('a',9)]],
 ['author6',[track('a',0),track('b',5),track('a',5)]],
 ['author7',[track('a',0),track('a',5),track('b',5)]],
 ['author8',[track('a',0),track('a',5),track('b',10)]],
 ['empty',[]],['invalid_date',[track('a','bad'),track('a','bad')]],
 ['threshold15',[track('a',0),track('a',15)]],['threshold16',[track('a',0),track('a',16)]],
 ['concatenated_identity_collision',[track('a - b',0,'c'),track('a',0,'b - c')]],
 ['cache_distance5_missed',[track('a',0),track('b',1),track('c',2),track('d',3),track('a',4)]],
 ['missing_buttons_uncertain',[{...track('a',0),noButtons:true},{...track('a',5),noButtons:true}]],
 ['missing_delete_button',[track('a',0),{...track('a',0),noDelete:true}]],
 ['one_minute_forced_boundary',[track('a',0),track('a',1)]],
];
function run(file,fixture){
 const dom=new JSDOM('<!doctype html><body></body>',{runScripts:'outside-only'}),doc=dom.window.document,rows=[],clicks=[];
 fixture.forEach((x,i)=>{const row=doc.createElement('div');row.className='chartlist-row';row.dataset.index=String(i);
 for(const [cls,text] of [['chartlist-name',x.song],['chartlist-artist',x.artist]]){const div=doc.createElement('div');div.className=cls;const a=doc.createElement('a');a.innerText=text;div.appendChild(a);row.appendChild(div);}
 const time=doc.createElement('div');time.className='chartlist-timestamp';const span=doc.createElement('span');span.title=x.min==='bad'?'not a date':new Date(Date.UTC(2020,0,1,0,x.min)).toISOString();time.appendChild(span);row.appendChild(time);
 if(!x.noButtons){const more=doc.createElement('button');more.innerText='More';row.appendChild(more);}
 if(!x.noDelete){const del=doc.createElement('a');del.className='more-item--delete';del.addEventListener('click',()=>clicks.push(i));row.appendChild(del);}
 doc.body.prepend(row);rows.push(row);});
 let error=null;try{vm.runInContext(fs.readFileSync(path.join(S,file),'utf8'),dom.getInternalVMContext(),{timeout:2000});}catch(e){error=e.name;}
 const state=rows.map((row,i)=>({index:i,background:row.style.background,clicks:clicks.filter(j=>j===i).length,choices:[...row.querySelectorAll('button')].filter(b=>b.innerText==='Delete').length}));dom.window.close();return {error,state};
}
const authorExpected=[[1,2,2],[1,2,3],[0,1,3],[1,0,2],[1,0,3],[1,0,2],[1,2,0],[1,3,0]];
function codes(result){return result.state.map(x=>x.clicks?2:x.choices?3:x.background==='rgb(221, 255, 217)'?1:0);}
const runs=[];
for(let iteration=0;iteration<2;iteration++){
 const variants=[];
 for(let variant=0;variant<files.length;variant++){
  const rows=cases.map(([name,data])=>({case:name,...run(files[variant],data)}));
  if(variant===0||variant===2){for(let i=0;i<8;i++)assert.deepStrictEqual(codes(rows[i]),authorExpected[i]);}
  assert.deepStrictEqual(rows[8].state,[]);assert.deepStrictEqual(codes(rows[9]),[0,0]);assert.deepStrictEqual(codes(rows[11]),[0,0]);assert.deepStrictEqual(codes(rows[13]),[0,0,0,0,0]);
  if(variant===0||variant===2){assert.deepStrictEqual(codes(rows[12]),[1,2]);assert.strictEqual(rows[14].error,'TypeError');assert.strictEqual(rows[15].error,'TypeError');assert.deepStrictEqual(codes(rows[16]),[1,2]);}
  if(variant===3)assert(rows.every(row=>row.state.every(x=>x.clicks===0)));
  variants.push({file:files[variant],scenarios:rows});
 }
 assert.deepStrictEqual(variants[0].scenarios,variants[2].scenarios);runs.push(variants);
}
assert.deepStrictEqual(runs[0],runs[1]);
const sourcePins=Object.fromEntries(files.map(f=>[path.relative(R,path.join(S,f)),hash(path.join(S,f))]));
const dependencyPins=Object.fromEntries(Object.keys(require.cache).filter(p=>p.startsWith(path.join(W,'node-tools/node_modules'))).map(p=>[path.relative(R,p),hash(p)]));
const report={status:'verified_four_complete_published_scripts_17scenarios_twice',producer_sha256:hash(__filename),source_pins:sourcePins,loaded_dependency_file_pins:dependencyPins,dependency_versions:{jsdom:require(path.join(W,'node-tools/node_modules/jsdom/package.json')).version},runs,scope:'Four whole unchanged IIFEs executed in fresh actualjsdom26.1DOM/NodeVM per fixture; nativeDate/DOM/click events. Ownedsynthetic rows and deletionclick recorder only; no actualLast.fm/network/browseraccount.17scenarios each4variants twice=136 executions.8author documented whiteboxcases verified primary/balancedpublished parity.',findings:['Concatenated song-title/artist key aliases distinct title/artist tuples.','Temporalnearby unrelated song can promote previously uncertain duplicate before sameSongcheck; reproduced author7 and8 contrast. This is explicit heuristic, not exactidentity proof.','Missing menu buttons makes uncertain modes throw; missingdelete target throws onforced click. Forced state trusts click without remote acknowledgement.','Invaliddates silently miss duplicates; distance5 falls outside4rowcache;15min inclusive,16 excluded;safe mode made noautomaticclicks acrossallfixtures.'],limits:['Nativejsdom is not actual browser/liveLast.fmDOM/API/authorization/delete success, precision/recall orperformance qualification.','README percentageaccuracy claims notmeasured here. Selectedscript body execution doesnot prove universal correctness or safeautomaticdeletion.','No production code copied oroptimizationadoption.']};
const out=path.join(R,'docs/research/dedup-500/lastfm56-native-controls-20261016.json');assert(!fs.existsSync(out));fs.writeFileSync(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({scenarios:17,variants:4,runs:136,status:'verified'}));
