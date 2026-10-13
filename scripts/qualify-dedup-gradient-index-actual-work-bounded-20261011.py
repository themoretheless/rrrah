#!/usr/bin/env python3
import hashlib,json,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    mp=D/'dedup-gradient-index-actual-work-bounded-snapshot-20261011.json';m=json.loads(mp.read_text());snapshot=Path(m['snapshot'])
    chain=[];cursor=mp
    for _ in range(3):
        doc=json.loads(cursor.read_text());source=Path(doc['snapshot'])
        assert all(sha(source/p)==h for p,h in doc['file_hashes'].items())
        chain.append((cursor,doc))
        if len(chain)<3:
            parent=Path(doc['parent_manifest']);assert sha(parent)==doc['parent_manifest_sha256'];cursor=parent
    for _,doc in chain[:2]:
        parent=json.loads(Path(doc['parent_manifest']).read_text())
        assert set(doc['file_hashes'])==set(parent['file_hashes'])
        assert {p for p,h in doc['file_hashes'].items() if parent['file_hashes'][p]!=h}==set(doc['changes'])
        assert sha(doc['producer'])==doc['producer_sha256']
        assert all(sha(p)==h for p,h in doc['edit_pins'].items())
    reports=[]
    for kind,total in [('raster-full',53),('decode',202)]:
        p=D/f'dedup-gradient-index-actual-work-bounded-{kind}-20261011.json';report=json.loads(p.read_text())
        assert report['status'].startswith('complete_') and report['tests']==total
        assert report['source_manifest_sha256']==sha(mp)
        assert all(sha(path)==h for path,h in report['pins'].items())
        for row in report['results']:
            assert row['status']=='passed' and row['returncode']==0
            assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256']
        reports.append({'path':str(p),'sha256':sha(p),'tests':total})
    baseline=chain[2][1]
    # Preserve every dedup file except the two explicitly tested changes.
    changes=set(m['changes'])
    for p,h in baseline['file_hashes'].items():
        if p.startswith('crates/rrrah-dedup/'):
            assert sha(R/p)==h,(p,'root changed since parent qualification')
    config=Path('/Users/themoretheless/.codex/tmp/rrrah-actual-work-default-rustfmt-20261011.toml');config.write_text('')
    subprocess.run(['rustfmt','--config-path',str(config),'--edition','2024','--check',*[str(snapshot/p) for p in changes]],check=True)
    for p in changes:shutil.copy2(snapshot/p,R/p)
    assert all(sha(R/p)==m['file_hashes'][p] for p in changes)
    out={'status':'adopted_bounded_actual_comparison_index_api','source_manifest':str(mp),'source_manifest_sha256':sha(mp),'reports':reports,'changes':sorted(changes),'scope':'Actual descriptor-distance attempts, inclusive exact-radius candidates, minimum linear self-work admission, atomic cancellation and limit refusal; legacy quadratic prepayment preserved. No full corpus or whole-system RSS claim.'}
    (D/'dedup-gradient-index-actual-work-bounded-adoption-20261011.json').write_text(json.dumps(out,indent=2)+'\n')
    print(out['status'])
if __name__=='__main__':main()
