"""Complete unchanged suffix module versus independent byte-slice oracles."""
from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/text85-20261016';S=W/'source';A=W/'suffix-controls';A.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=r'''
#[path="SOURCE"] mod table;
use table::SuffixTable;
fn words(max:usize)->Vec<Vec<u8>>{let mut all=vec![vec![]];let mut layer=vec![vec![]];for _ in 0..max{let mut next=vec![];for w in &layer{for c in [0u8,1,255]{let mut v=w.clone();v.push(c);next.push(v)}}all.extend(next.clone());layer=next;}all}
fn main(){
 let texts=words(7);let mut queries=words(4);queries.push(vec![2]);queries.push(vec![0,2]);let mut calls=0u64;
 for text in &texts{
  let sa=SuffixTable::new(text.as_slice());let naive=SuffixTable::new_naive(text.as_slice());let mut oracle:Vec<u64>=(0..text.len() as u64).collect();oracle.sort_by(|a,b|text[*a as usize..].cmp(&text[*b as usize..]));assert_eq!(sa.table(),oracle);assert_eq!(naive.table(),oracle);assert_eq!(sa.text(),text);assert_eq!(sa.len(),text.len());assert_eq!(sa.is_empty(),text.is_empty());
  let lcps=sa.lcp_lens();assert_eq!(lcps.len(),text.len());for i in 1..oracle.len(){let a=oracle[i-1] as usize;let b=oracle[i] as usize;let mut k=0;while a+k<text.len()&&b+k<text.len()&&text[a+k]==text[b+k]{k+=1}assert_eq!(lcps[i],k as u64)}
  for q in &queries{
   let expected:Vec<u64>=if q.is_empty(){vec![]}else{(0..text.len()).filter(|&i|text[i..].starts_with(q)).map(|i|i as u64).collect()};let mut actual=sa.positions(q).to_vec();actual.sort();assert_eq!(actual,expected,"text={:?} query={:?}",text,q);assert_eq!(sa.contains(q),!expected.is_empty());calls+=1;
  }
  let (t,a)=sa.into_parts();let rebuilt=SuffixTable::from_parts(t,a);assert_eq!(rebuilt.table(),oracle);
 }
 let malformed=SuffixTable::from_parts(&b"abc"[..],&[99u64,0,1][..]);let bad=std::panic::catch_unwind(||malformed.suffix_bytes(0));assert!(bad.is_err());
 println!("TEXTS={} QUERIES={} MALFORMED_PANIC=1",texts.len(),calls);
}
'''.replace('SOURCE',str(S/'src/table.rs'))
(A/'controls.rs').write_text(source);runs=[]
for checked in [True,False]:
 exe=A/('checked' if checked else 'unchecked');cmd=['rustc','+1.98.1','--edition=2021','-C','opt-level=3','-C','overflow-checks='+str(checked).lower(),str(A/'controls.rs'),'-o',str(exe)];p=subprocess.run(cmd,capture_output=True,text=True);(A/(exe.name+'-build.log')).write_text(p.stdout+p.stderr);assert p.returncode==0
 for n in range(2):
  p=subprocess.run([str(exe)],capture_output=True,text=True,timeout=60);(A/f'{exe.name}-run{n}.stdout').write_text(p.stdout);(A/f'{exe.name}-run{n}.stderr').write_text(p.stderr);assert p.returncode==0,(exe,p.stderr);assert p.stdout.strip()=='TEXTS=3280 QUERIES=403440 MALFORMED_PANIC=1';runs.append({'checked_overflow':checked,'run':n,'texts':3280,'queries':403440,'stdout_sha256':h(A/f'{exe.name}-run{n}.stdout')})
p=R/'docs/research/dedup-500/text85-suffix-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_complete_unchanged_suffix_module_exhaustive_small_byte_oracles','whole_repo_complete':False,'producer_sha256':h(Path(__file__)),'source':str((S/'src/table.rs').relative_to(R)),'source_sha256':h(S/'src/table.rs'),'runs':runs,'artifact_pins':{str(p.relative_to(R)):h(p) for p in A.iterdir() if p.is_file()},'scope':'Whole unchanged original940line table.rs included via Rust pathattribute, no adapters/dependencies/extracted bodies. NativeRust1.98.1,opt3,overflowchecked/unchecked eachtwice. Exhaustive3symbol byte texts lengths0..7, allqueries0..4 plus2absent queries; independentbyte-slice sorted suffix/LCP/occurrence oracles, originalnaive parity andinto_parts roundtrip.','findings':['All3280boundedtexts and403440queries perrun satisfy suffixconstruction/order,LCP,contains/positions/oracle; emptyquery deliberatelyreturnsno matches.','from_parts checks lengthonly; invalid99index accepted then suffix_bytes panics, matching explicitlydocumented uncheckedinvariant.','Documentation still describes32bit/4byte indices andUTF8 restrictions whileimplementation u64/bytebased. LCPimplementation isquadratic andallocates unusedinversevector. Static performance concern, no timing/adoption.'],'limits':['Onlywhole suffixmodule; mainCLI readthroughline260 only, otherprojectfiles notyetread, no fullsource countincrease.','No fullCargo/upstreamsuite/diskstream/chunkmerge/trainingdataset/hugefile/physicalIO/hardRSS/platform/performance or productionadoption qualification.']},indent=2)+'\n');print(json.dumps({'status':'verified','runs':4,'texts_total':13120,'queries_total':1613760}))
