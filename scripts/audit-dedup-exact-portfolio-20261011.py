#!/usr/bin/env python3
"""Verify the exact-first frozen generic gate and current changed source files."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    manifest_path=D/'dedup-exact-portfolio-fixed-snapshot-20261011.json'
    gate_path=D/'dedup-exact-portfolio-fixed-generic-20261011.json'
    m=json.loads(manifest_path.read_text());g=json.loads(gate_path.read_text())
    assert g['status']=='complete_bounded_frozen_exact_first_portfolio_execution',g['status']
    assert g['tests']==33 and g['build_returncode']==0
    assert g['source_manifest_sha256']==sha(manifest_path)
    for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
    changed=['crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/src/exact_portfolio.rs','crates/rrrah-dedup/src/lib.rs','crates/rrrah-dedup/tests/projected_portfolio.rs']
    for p in changed:assert sha(ROOT/p)==m['file_hashes'][p],p
    for p,h in g['pins'].items():assert sha(p)==h,p
    assert {r['suite']:r['tests'] for r in g['results']}=={'projected_portfolio':17,'exact_files':16}
    for r in g['results']:
        assert r['status']=='passed' and r['returncode']==0
        assert sha(r['binary'])==r['binary_sha256'] and sha(r['log'])==r['log_sha256']
    parent=json.loads(Path(m['parent_manifest']).read_text())
    assert all(h==m['file_hashes'][p] for p,h in parent['file_hashes'].items() if p not in m['changes'])
    out={'status':'complete_frozen_exact_first_portfolio_generic_qualification','tests':33,'frozen_files':len(m['file_hashes']),'current_changed_sources_match_snapshot':True,'test_only_correction':m['changes'],'pins':{str(p):sha(p) for p in [manifest_path,gate_path,Path(__file__)]},'limits':['Generic providers on macOS; no Torch/Windows/Linux extension','Pair API only; collection and native LoFTR adapters remain open','Visual MemoryBudget excludes fixed exact streaming blocks and provider RSS','Full visual corpus and all-format qualification remain incomplete']}
    (D/'dedup-exact-portfolio-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n')
    print(out['status'],out['tests'])
if __name__=='__main__':main()
