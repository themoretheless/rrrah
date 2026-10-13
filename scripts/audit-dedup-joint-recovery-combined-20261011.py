#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-joint-recovery-combined-audit-20261011.json';assert not out.exists()
 paths=[D/'dedup-joint-recovery-audit-20261011.json',D/'dedup-joint-recovery-testfix-audit-20261011.json'];old,new=[json.loads(p.read_text()) for p in paths];assert old['status']=='failed' and len(old['results'])==7;assert new['status']=='complete_bounded_frozen_joint_recovery_collection_20_tests'
 manifests=[json.loads(Path(x['source_manifest']).read_text()) for x in [old,new]];before,after=manifests;changed=[f for f,h in after['file_hashes'].items() if before['file_hashes'].get(f)!=h];assert changed==['crates/rrrah-dedup/tests/portfolio_collection.rs']
 pins={str(p):sha(p) for p in paths+[Path(__file__)]};results={}
 for gate,m in zip([old,new],manifests):
  assert all(sha(p)==h for p,h in gate['pins'].items());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items())
  for row in gate['results']:
   assert row['suite'] not in results
   for key in ['binary','log']:assert sha(row[key])==row[key+'_sha256'];pins[row[key]]=row[key+'_sha256']
   assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text())==[str(row['tests'])];results[row['suite']]=row['tests']
 assert len(results)==8 and sum(results.values())==80
 owned=['geometry','local_scan','anchor_rank_file','search_portfolio','portfolio_collection'];current={}
 for name in owned:
  f=f'crates/rrrah-dedup/src/{name}.rs';assert sha(R/f)==after['file_hashes'][f];current[f]=sha(R/f)
 cpu=D/'dedup-joint-recovery-loftr-compile-audit-20261011.json';c=json.loads(cpu.read_text());assert c['status']=='verified_frozen_cpu_joint_recovery_compile';assert all(sha(p)==h for p,h in c['pins'].items());assert all(sha(R/f)==h for f,h in c['current_owned_source'].items());pins[str(cpu)]=sha(cpu)
 out.write_text(json.dumps({'status':'verified_80_tests_and_cpu_compile_joint_recovery','pins':pins,'suites':results,'tests':80,'current_owned_source':current,'test_only_delta':changed,'scope':'Seven passing suites (60 tests) from preserved failed first gate plus corrected collection suite (20 tests). Only collection test policy changed between frozen snapshots; production source unchanged. CPU wrappers compile in separately pinned compatible native snapshot. Not whole current HDR workspace, real-provider combined runtime, corpus, 500 upstream or hard RSS qualification.'},indent=2)+'\n');print('verified80 tests and CPU wrapper compilation')
if __name__=='__main__':main()
