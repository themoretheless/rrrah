"""Copy and verify exactly 57 completed records; never certify the live report."""
import hashlib, json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
OUT=D/'dedup-gradient-batch-full229-prefix57-audit-20261011.json'
assert not OUT.exists()
sha=lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
live=json.loads((D/'dedup-gradient-batch-full229-20261011.json').read_text())
rows=live['results'][:57]
assert len(rows)==57 and len({r['query'] for r in rows})==57
assert [r['query'] for r in rows]==live['pair_order'][:57]
assert all(sha(p)==h for p,h in live['pins'].items())
manifest=json.loads((D/'dedup-gradient-batch-production-snapshot-20261011.json').read_text())
assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
def pixel_proof(value):
    if isinstance(value,dict):
        if value.get('supported') is True and isinstance(value.get('pixels'),dict) and value['pixels'].get('supported') is True:
            return True
        return any(pixel_proof(v) for v in value.values())
    if isinstance(value,list): return any(pixel_proof(v) for v in value)
    return False
for row in rows:
    assert row['status']=='supported' and row['returncode']==0 and row['supported'] is True
    evidence=row['evidence'];assert json.loads(row['stdout'])==evidence
    assert evidence['files']==[0,1] and len(evidence['pairs'])==1
    assert evidence['managed_used']==0 and evidence['managed_peak']<=512*1024*1024
    assert evidence['source_paths']==[row['source'],row['target']]
    assert pixel_proof(evidence['pairs'][0])
audit={'status':'complete_57_record_prefix_only','required_full_pairs':229,'completed_prefix_pairs':57,'supported':57,'miss':0,'execution_failed':0,'frozen_source_manifest':str(D/'dedup-gradient-batch-production-snapshot-20261011.json'),'rows':rows,'pins':live['pins'],'auditor_sha256':sha(__file__),'limits':'Only these copied 57 known-positive pair records. The full 229-pair producer remains independent and may still run. No full-corpus, all-query negative, indexed collection or later root source qualification. The mutable live report is deliberately not pinned as a closed artifact.'}
OUT.write_text(json.dumps(audit,indent=2)+'\n')
print(audit['status'])
