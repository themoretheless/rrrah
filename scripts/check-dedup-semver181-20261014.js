const fs = require('fs'), vm = require('vm'), path = require('path');
const source = fs.readFileSync(process.argv[2], 'utf8');
function run(name, nodes, expected, error) {
  const deleted=[], reads=[];
  function req(p) {
    if(p==='fs') return {
      existsSync(p) {return Object.keys(nodes).some(n=>path.dirname(n)===p);},
      readdirSync(p) {return Object.keys(nodes).filter(n=>path.dirname(n)===p).map(n=>path.basename(n));},
      lstatSync(p) {return {isSymbolicLink:()=>!!nodes[p].linked};}
    };
    if(p==='path') return path;
    if(p==='semver') return {parse(s) {const m=typeof s==='string'&&s.match(/^(\d+)\.(\d+)\.(\d+)(?:-[a-z0-9.-]+)?$/i);return m?{major:+m[1],minor:+m[2],patch:+m[3]}:null;}};
    if(p==='rimraf') return {sync(p) {deleted.push(p);}};
    const n=path.dirname(p);reads.push(n);if(!nodes[n])throw Error('missing fixture package: '+n);return nodes[n];
  }
  let failure=null;try {vm.runInNewContext(source,{require:req,process:{cwd:()=>'/fixture'},console:{log() {}}},{timeout:1000});}catch(e){failure=String(e);}
  if(JSON.stringify(deleted)!==JSON.stringify(expected))throw Error(name+': unexpected deletions '+JSON.stringify(deleted));
  if(error ? !failure||!failure.includes(error) : failure)throw Error(name+': '+failure);
  return {name,deleted,package_reads:reads,error:failure};
}
const pkg=(name,version,extra={})=>({name,version,...extra});
function normal(top,inner){return {'/fixture':pkg('root','1.0.0'),'/fixture/node_modules/library':pkg('library',top),'/fixture/node_modules/parent':pkg('parent','1.0.0'),'/fixture/node_modules/parent/node_modules/library':pkg('library',inner)};}
const inner='/fixture/node_modules/parent/node_modules/library';
const rows=[];
for(const [name,top,child,remove]of[
 ['same-major-different-bytes-and-range','1.2.0','1.0.0',true],
 ['newer-child-replaced-by-older-top','1.0.0','1.9.0',true],
 ['different-major','1.9.0','2.0.0',false],
 ['zero-exact','0.1.0','0.1.0',true],
 ['zero-different-minor','0.1.0','0.2.0',false],
 ['zero-different-patch','0.1.0','0.1.1',false],
 ['zero-prerelease-collapsed','0.1.0','0.1.0-beta',true]
]){const nodes=normal(top,child);nodes['/fixture/node_modules/parent'].dependencies={library:child};rows.push(run(name,nodes,remove?[inner]:[]));}
let nodes=normal('1.0.0','1.0.0');nodes[inner].name='different';rows.push(run('different-names',nodes,[]));
nodes=normal('1.0.0','1.0.0');nodes['/fixture/node_modules/parent'].linked=true;const linked=run('linked-subtree-read-before-skip',nodes,[]);if(!linked.package_reads.includes(inner))throw Error('linked subtree was not parsed');rows.push(linked);
nodes=normal('1.0.0','1.0.0');nodes['/fixture/node_modules/parent/node_modules/alias']=pkg('library','1.8.0');rows.push(run('two-child-duplicates',nodes,[inner,'/fixture/node_modules/parent/node_modules/alias']));
nodes=normal('1.0.0','not-a-version');rows.push(run('invalid-version',nodes,[],'null'));
nodes=normal('1.0.0',undefined);rows.push(run('missing-version',nodes,[],'null'));
nodes={'/fixture':pkg('root','1.0.0'),'/fixture/node_modules/@scope':undefined};rows.push(run('scoped-folder-not-package',nodes,[],'missing fixture package'));
nodes={'/fixture':pkg('root','1.0.0'),'/fixture/node_modules/.bin':undefined};rows.push(run('hidden-directory-skipped',nodes,[]));
nodes={'/fixture':pkg('root','1.0.0'),'/fixture/node_modules/first':pkg('same','1.0.0'),'/fixture/node_modules/alias':pkg('same','1.0.0')};rows.push(run('same-level-duplicate-not-removed',nodes,[]));
console.log(JSON.stringify({rows,scope:'Whole original CLI source unchanged under vm. Fully synthetic in-memory filesystem/package tree, numeric/prerelease-component parser adapter, deletion recorder and suppressed logger. No actual semver3/rimraf2/npm resolution, filesystem deletion, links, network, package installs or production input; not compatibility or full runtime qualification.'},null,2));
