#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-loftr-coarse-cancel-snapshot-20261011.json';m=json.loads(mp.read_text());src=Path(m['snapshot']);assert all(sha(src/f)==h for f,h in m['file_hashes'].items());assert sha(m['producer'])==m['producer_sha256'] and sha(m['parent_manifest'])==m['parent_sha256']
 rp=D/'dedup-loftr-coarse-cancel-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_original_positive_memory_comparison' and len(r['results'])==4;assert all(sha(p)==h for p,h in r['pins'].items())
 refs={v['case']:v for v in r['results']}
 for direction in ['forward','reverse']:
  a=refs['baseline_'+direction];b=refs['candidate_'+direction];assert a['returncode']==b['returncode']==0
  for row in [a,b]:assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];assert json.loads(row['stdout'])['managed_used']==0
  assert json.loads(a['stdout'])['pairs']==json.loads(b['stdout'])['pairs']
 h=T/'rrrah-loftr-coarse-cancel-test-harness-20261011'
 for p in (src/'vendor/loftr/src').rglob('*.rs'):
  q=h/'src'/p.relative_to(src/'vendor/loftr/src')
  if p.relative_to(src).as_posix()=='vendor/loftr/src/coarse_matching/tests.rs':assert q.read_text().startswith(p.read_text())
  else:assert sha(p)==sha(q)
 log=D/'dedup-loftr-coarse-cancel-all-tests-20261011.log';assert '46 passed; 0 failed; 0 ignored' in log.read_text();previous=json.loads((D/'dedup-loftr-early-release-adoption-20261011.json').read_text());assert all(sha(R/f)==v for f,v in previous['adopted'].items());parent=json.loads(Path(m['parent_manifest']).read_text());files=['vendor/loftr/src/coarse_matching.rs','vendor/loftr/src/loftr_model.rs','vendor/loftr/src/coarse_matching/tests.rs'];assert sha(R/files[0])==parent['file_hashes'][files[0]] and sha(R/files[2])==parent['file_hashes'][files[2]];before={f:sha(R/f) for f in files};out=D/'dedup-loftr-coarse-cancel-adoption-20261011.json';assert not out.exists()
 for f in files:
  q=h/'src'/Path(f).relative_to('vendor/loftr/src');(R/f).write_bytes(q.read_bytes());assert sha(R/f)==sha(q)
 pins={str(p):sha(p) for p in [mp,rp,Path(__file__),log,h/'Cargo.toml',h/'Cargo.lock',D/'dedup-loftr-coarse-cancel-test-artifacts-20261011.jsonl',T/'rrrah-loftr-coarse-cancel-tests-qualified-20261011/loftr_memory_controls']};pins.update({str(p):sha(p) for p in (h/'src').rglob('*.rs')});out.write_text(json.dumps({'status':'adopted_bounded_coarse_cooperative_cancellation','pins':pins,'before':before,'adopted':{f:sha(R/f) for f in files},'library_tests':46,'native_controls':4,'positive_evidence_equal':True,'scope':'CPU macOS Torch2.10/tch0.23. All observed coarse/softmax and full-model cancellation checkpoints return no partial output and permit retry; masked/unmasked and grad/inference controls. Individual running kernels cannot be interrupted. No hard latency, total RSS, full corpus or platform claim. The ongoing458 replay keeps its older pinned binary.'},indent=2)+'\n');print('adopted46tests4nativecontrols')
if __name__=='__main__':main()
