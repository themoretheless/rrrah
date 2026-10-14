const fs=require('fs'),vm=require('vm');
const source=fs.readFileSync(process.argv[2],'utf8');
const nodes={},created=[],requests=[],urls=[],downloads=[];
function element(){return {innerHTML:'',value:'',style:{},children:[],classList:{add(){},remove(){}},appendChild(v){this.children.push(v)},removeChild(v){this.children=this.children.filter(x=>x!==v)},click(){downloads.push({name:this.download,href:this.href})}};}
for(const key of ['embyServerUrl','apiKey','results','loading-overlay'])nodes[key]=element();
const ctx={document:{getElementById:k=>nodes[k],createElement(){const v=element();created.push(v);return v;},body:element()},Blob,URL:{createObjectURL(b){urls.push(b);return 'blob:synthetic';},revokeObjectURL(){}},fetch:async()=>{throw Error('fixture fetch not set')}};
vm.createContext(ctx);vm.runInContext(source,ctx);const rows=[];
function test(name,movies,expectedKeys,error){
 let result=null,failure=null;try{result=ctx.findDuplicatesInLibrary(movies)}catch(e){failure=String(e)}
 if(error){if(!failure||!failure.includes(error))throw Error(name+': '+failure)}else{if(failure||JSON.stringify(Object.keys(result).sort())!==JSON.stringify(expectedKeys.sort()))throw Error(name+': '+failure+' '+JSON.stringify(result));}
 rows.push({name,input:movies,result,error:failure});
}
const movie=(Name,ProductionYear,Path)=>({Name,ProductionYear,Path});
test('empty',[],[]);
test('single',[movie('Alpha',2020,'a')],[]);
test('same-name-year-distinct-content-not-read',[movie('Alpha',2020,'different-a'),movie('Alpha',2020,'different-b')],['Alpha_2020']);
test('different-years-grouped',[movie('Alpha',2020,'a'),movie('Alpha',2021,'b')],['Alpha']);
test('case-sensitive-miss',[movie('Alpha',2020,'a'),movie('alpha',2020,'b')],[]);
test('outer-whitespace-normalized',[movie(' Alpha ',2020,'a'),movie('Alpha',2020,'b')],['Alpha_2020']);
test('missing-years',[movie('Alpha',undefined,'a'),movie('Alpha',undefined,'b')],['Alpha']);
test('same-path-counted-twice',[movie('Alpha',2020,'same'),movie('Alpha',2020,'same')],['Alpha_2020']);
test('overlapping-year-and-name-groups',[movie('Alpha',2020,'a'),movie('Alpha',2020,'b'),movie('Alpha',2021,'c')],['Alpha','Alpha_2020']);
test('delimiter-collision',[movie('Alpha_2020',undefined,'a'),movie('Alpha',2020,'b')],['Alpha_2020']);
for(const name of ['__proto__','constructor','toString','valueOf','hasOwnProperty'])test('prototype-name-'+name,[movie(name,undefined,'a')],[], 'push is not a function');
test('numeric-name',[movie(7,2020,'a')],[],'trim is not a function');
test('blank-after-trim',[movie(' ',2020,'a'),movie(' ',2020,'b')],['_2020']);
test('undefined-path',[movie('Alpha',2020),movie('Alpha',2020)],['Alpha_2020']);
test('missing-name-skipped',[movie(undefined,2020,'a'),movie('',2020,'b')],[]);
test('unicode-normalization-miss',[movie('\u00e9',2020,'a'),movie('e\u0301',2020,'b')],[]);
(async()=>{
 const network=[];
 ctx.fetch=async url=>{requests.push(url);return {ok:true,json:async()=>url.includes('VirtualFolders')?[{CollectionType:'movies',ItemId:'fixture-id',Name:'fixture-library'},{CollectionType:'tvshows',ItemId:'tv',Name:'ignored'}]:{Items:[movie('Only',2020,'a')],TotalRecordCount:100,StartIndex:0}};};
 nodes.embyServerUrl.value=' fixture.invalid ';nodes.apiKey.value='fixture&StartIndex=1';await ctx.findDuplicates();
 if(requests.length!==2||!requests[0].startsWith('http://fixture.invalid/')||!requests[1].includes('api_key=fixture&StartIndex=1')||!nodes.results.innerHTML.includes('No duplicates'))throw Error('request/default HTTP/pagination control');
 network.push({name:'default-http-raw-query-key-no-pagination',requests:[...requests],display:nodes.results.innerHTML,total_record_count:100});
 ctx.fetch=async()=>({ok:false});await ctx.findDuplicates();if(!nodes.results.innerHTML.includes('Failed to fetch libraries'))throw Error('HTTP error lost');network.push({name:'library-http-error',display:nodes.results.innerHTML});
 ctx.fetch=async()=>{throw Error('<b>fixture error</b>')};await ctx.findDuplicates();if(!nodes.results.innerHTML.includes('<b>fixture error</b>'))throw Error('error markup changed');network.push({name:'raw-error-markup',display:nodes.results.innerHTML});
 ctx.displayResults([{libraryName:"O'Reilly",count:1,duplicates:{Alpha:[{path:'fixture-a',year:2020},{path:'fixture-b',year:2020}]}}]);
 const html=nodes.results.children.at(-1).innerHTML;const onclick=html.match(/onclick="([^"]*)"/)[1].replace(/&quot;/g,'"');let syntax=null;try{new vm.Script(onclick)}catch(e){syntax=String(e)};if(!syntax)throw Error('apostrophe inline handler unexpectedly valid');network.push({name:'apostrophe-library-inline-handler',html,syntax_error:syntax});
 ctx.displayResults([{libraryName:'<b>fixture title</b>',count:1,duplicates:{Alpha:[]}}]);const rawHTML=nodes.results.children.at(-1).innerHTML;if(!rawHTML.includes('<h3><b>fixture title</b></h3>'))throw Error('library markup escaped');network.push({name:'raw-library-markup',html:rawHTML});
 ctx.downloadDuplicates('fixture-library',{Alpha:[{path:'fixture-a',year:2020},{path:'fixture-b',year:2021}]});const content=await urls.at(-1).text();if(!content.includes('fixture-a (2020)')||downloads.at(-1).name!=='fixture-library_duplicates.txt')throw Error('download control');network.push({name:'synthetic-download-text',content,record:downloads.at(-1)});
 console.log(JSON.stringify({rows,network,scope:'Whole original JS unchanged in Node vm; real grouping/async/render/download functions. Explicit minimal DOM, fake fetch responses, real Node Blob and objectURL/click recorders. No browser markup/event execution, real server/API/account/credentials/network, media reads, deletion or UI visual qualification. All inputs authored synthetic metadata; token fixture only.'},null,2));
})().catch(e=>{console.error(e);process.exitCode=1});
