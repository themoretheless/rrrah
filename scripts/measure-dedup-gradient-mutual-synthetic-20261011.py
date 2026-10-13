"""Isolated same-process four-lane baseline/candidate controls; not real-image qualification."""
import hashlib,json,statistics,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-gradient-mutual-synthetic-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-mutual-synthetic-qualified-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 manifest_path=D/'dedup-gradient-mutual-snapshot-20261011.json';m=json.loads(manifest_path.read_text());snapshot=Path(m['snapshot'])
 source=snapshot/'crates/rrrah-dedup/src/gradient.rs';oracle=snapshot/'crates/rrrah-dedup/src/gradient_matching_tests.rs';reference=snapshot/'crates/rrrah-dedup/src/gradient_mutual_tests.rs'
 for path in [source,oracle,reference]:assert sha(path)==m['file_hashes'][str(path.relative_to(snapshot))]
 text=source.read_text();production=text[text.index('fn distances_four('):text.index('\n/// Describe a rotated')]
 text=reference.read_text();baseline=text[text.index('fn baseline('):text.index('\nfn policy()')]
 text=oracle.read_text();bank=text[text.index('fn bank('):text.index('\nfn policy()')]
 shell='''#[derive(Debug,Clone,Copy,PartialEq)] struct GradientDescriptor([f64;128]);
#[derive(Debug,Clone,Copy,PartialEq)] struct GradientFeature {position:[f64;2],descriptor:GradientDescriptor}
#[derive(Debug,Clone,Copy)] struct GradientMatchPolicy {max_comparisons:u64,max_squared_distance:f64,squared_ratio:f64}
#[derive(Debug)] enum LocalError {Invalid,Cancelled,Budget}
mod geometry {#[derive(Debug)] pub struct Correspondence {pub source:[f64;2],pub target:[f64;2]}}
'''
 main='''fn main() {
let policy=GradientMatchPolicy {max_comparisons:10_000_000,max_squared_distance:2.,squared_ratio:0.8};
for sparse in [false,true] {for positive in [false,true] {
let left=bank(500,173,sparse);let right=if positive {left.clone()} else {bank(500,591,sparse)};
for radius in [None,Some(64.)] {
let expected:Vec<_>=baseline(&left,&right,policy,radius,||false).unwrap().into_iter().map(|p|(p.source,p.target)).collect();
for fast in [false,true,true,false] {
let start=std::time::Instant::now();
let result=if fast {match_gradients_selected(&left,&right,policy,radius,||false)} else {baseline(&left,&right,policy,radius,||false)}.unwrap();
let elapsed=start.elapsed().as_secs_f64();
let got:Vec<_>=result.into_iter().map(|p|(p.source,p.target)).collect();assert_eq!(got,expected);
println!("{} {} {} {} {} {:.9}",sparse,positive,radius.is_some(),fast,expected.len(),elapsed);
}}}}
}
'''
 unit=ARCHIVE/'matcher.rs';unit.write_text(shell+production+'\n'+baseline+'\n'+bank+'\n'+main);binary=ARCHIVE/'matcher'
 build=subprocess.run(['rustc','--edition=2024','-O',str(unit),'-o',str(binary)],capture_output=True,text=True);assert build.returncode==0,build.stderr
 run=subprocess.run([str(binary)],capture_output=True,text=True);assert run.returncode==0,run.stderr
 log=ARCHIVE/'timings.log';log.write_text(run.stdout+run.stderr);rows=[]
 for line in run.stdout.splitlines():
  sparse,positive,radius,fast,count,seconds=line.split();rows.append({'sparse':sparse=='true','positive':positive=='true','distinct':radius=='true','candidate':fast=='true','matches':int(count),'seconds':float(seconds)})
 assert len(rows)==32;summary=[]
 for sparse in [False,True]:
  for positive in [False,True]:
   for radius in [False,True]:
    selected=[r for r in rows if (r['sparse'],r['positive'],r['distinct'])==(sparse,positive,radius)];assert [r['candidate'] for r in selected]==[False,True,True,False]
    assert len({r['matches'] for r in selected})==1
    before=statistics.median(r['seconds'] for r in selected if not r['candidate']);after=statistics.median(r['seconds'] for r in selected if r['candidate'])
    summary.append({'sparse':sparse,'positive':positive,'distinct':radius,'matches':selected[0]['matches'],'baseline_median_seconds':before,'candidate_median_seconds':after,'baseline_to_candidate_ratio':before/after})
 pins={str(p):sha(p) for p in [manifest_path,source,oracle,reference,unit,binary,log,Path(__file__)]}
 OUT.write_text(json.dumps({'status':'complete_eight_isolated_synthetic_controls','samples':rows,'summaries':summary,'pins':pins,'scope':__doc__,'limits':['Simple type shells and production/reference functions extracted from frozen source; no crate integration qualification','Constant false cancellation callback; full callback semantics separately covered by mathematical oracle','Shared host and synthetic features; not real-image or full-library speedup evidence','Candidate retention still requires frozen205 gate and real-photo controls']},indent=2)+'\n');print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
