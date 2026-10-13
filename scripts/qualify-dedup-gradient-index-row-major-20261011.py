#!/usr/bin/env python3
"""Verify immutable candidate evidence, then adopt only the tested index source."""
import hashlib,json,re,shutil,statistics,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 outpath=D/'dedup-gradient-index-row-major-adoption-20261011.json';assert not outpath.exists()
 mp=D/'dedup-gradient-index-row-major-formatted-snapshot-20261011.json';rp=D/'dedup-gradient-index-row-major-formatted-20261011.json';dp=D/'dedup-gradient-index-row-major-formatted-decode-20261011.json'
 m=json.loads(mp.read_text());r=json.loads(rp.read_text());d=json.loads(dp.read_text());name='crates/rrrah-dedup/src/gradient_index.rs'
 assert r['status']=='complete_bounded_frozen_row_major_index_controls' and r['tests']==48 and r['benchmark_builds']==160
 assert d['status']=='complete_bounded_frozen_row_major_index_decode_execution' and d['tests']==202 and len(d['results'])==24 and d['build_returncode']==0
 for report in [r,d]:
  for p,h in report['pins'].items():assert sha(p)==h,p
  for row in report['results']:assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256']
 assert d['source_manifest_sha256']==sha(mp)
 assert sha(r['benchmark_log'])==r['benchmark_log_sha256']
 rawrows=re.findall(r'ROW_MAJOR_INDEX_BENCH control=(\w+) baseline=(true|false) round=(\d+) elapsed_ns=(\d+) peak=(\d+) axis=(\d+)',Path(r['benchmark_log']).read_text())
 records=[{'control':n,'baseline':b=='true','round':int(k),'elapsed_ns':int(t),'peak':int(p),'axis':int(a)} for n,b,k,t,p,a in rawrows];assert records==r['measurements'] and len(records)==160
 for control,summary in r['summary'].items():
  group=[x for x in records if x['control']==control]
  for round in range(8):assert [x['baseline'] for x in group if x['round']==round]==([True,False,False,True] if round%2==0 else [False,True,True,False])
  a=statistics.median(x['elapsed_ns'] for x in group if x['baseline']);b=statistics.median(x['elapsed_ns'] for x in group if not x['baseline'])
  assert summary=={'baseline_median_ns':a,'candidate_median_ns':b,'baseline_over_candidate':a/b}
  assert a/b>1.02 and len({x['peak'] for x in group})==len({x['axis'] for x in group})==1
 lineage=[mp];current=m
 for level in range(3):
  for p,h in current['file_hashes'].items():assert sha(Path(current['snapshot'])/p)==h,p
  assert sha(current['producer'])==current['producer_sha256']
  if 'test_helper' in current:assert sha(current['test_helper'])==current['test_helper_sha256']
  if level==2:break
  pp=Path(current['parent_manifest']);assert sha(pp)==current['parent_manifest_sha256'];parent=json.loads(pp.read_text());lineage.append(pp)
  assert set(current['file_hashes'])==set(parent['file_hashes'])
  assert [p for p,h in current['file_hashes'].items() if h!=parent['file_hashes'][p]]==[name]
  current=parent
 baseline=current;raw=json.loads(Path(m['parent_manifest']).read_text());original=(Path(baseline['snapshot'])/name).read_text();candidate=(Path(raw['snapshot'])/name).read_text()
 a=original.index('    pub fn new(');b=original.index('    /// Search a horizontally',a)
 reference=original[a:b].replace('    pub fn new(',"    fn reference<'a>(").replace('Result<Self, LocalError>',"Result<GradientDescriptorIndex<'a>, LocalError>").replace('Ok(Self {','Ok(GradientDescriptorIndex {')
 assert reference in candidate
 formatted=subprocess.run(['rustfmt','--edition','2024','--emit','stdout'],input=candidate,text=True,capture_output=True,cwd=m['snapshot'],check=True).stdout
 assert formatted==(Path(m['snapshot'])/name).read_text()
 assert sha(ROOT/name)==baseline['file_hashes'][name],'Root edited since baseline; refuse overwrite'
 for p,h in m['file_hashes'].items():
  if p.startswith('crates/rrrah-dedup/') and p!=name:assert sha(ROOT/p)==h,p
 shutil.copy2(Path(m['snapshot'])/name,ROOT/name);assert sha(ROOT/name)==m['file_hashes'][name]
 out={'status':'adopted_bounded_qualified_row_major_index_build','raster_tests':48,'decode_tests':202,'decode_programs':24,'benchmark_builds':160,'summary':r['summary'],'source_manifest':str(mp),'source_change':name,'current_changed_source_matches':True,'pins':{str(p):sha(p) for p in [*lineage,rp,dp,Path(__file__)]},'scope':'Sequential per-row exact extrema replace 128 strided scans; same axis/tie/order/search results, same caller checkpoint counts and managed credit. Original constructor reference is pinned and verified. Two 128-element f64 arrays add bounded stack metadata, no heap bank.','limits':['Synthetic index construction controls, not whole-file or indexed collection throughput','No universal retrieval recall, broad corpus precision, provider RSS or all-format/platform claim']}
 outpath.write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
