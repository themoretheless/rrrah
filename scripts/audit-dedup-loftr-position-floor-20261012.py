#!/usr/bin/env python3
"""Audit complete position-floor candidate; never infer qualification from tests alone."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    report=D/'dedup-loftr-position-floor-runtime-20261012.json';r=json.loads(report.read_text())
    assert r['status']=='complete_candidate_not_adopted' and r['returncode']==r['build_returncode']==0
    assert all(sha(p)==v for p,v in r['pins'].items())
    source=Path(r['source']);assert all(sha(source/n)==v for n,v in r['source_hashes'].items())
    bp=D/'dedup-loftr-current-stack-runtime-20261012.json';b=json.loads(bp.read_text());base=Path(b['source'])
    assert b['returncode']==b['build_returncode']==0
    assert all(sha(p)==v for p,v in b['pins'].items())
    assert all(sha(base/n)==v for n,v in b['root_source_hashes'].items())
    assert set(r['source_hashes'])==set(b['root_source_hashes'])
    changed=sorted(n for n,v in r['source_hashes'].items() if v!=b['root_source_hashes'][n])
    assert changed==['vendor/loftr/src/loftr_model.rs']
    n=changed[0];original=(base/n).read_text();assert original.count('(256, 256),')==1
    assert (source/n).read_text()==original.replace('(256, 256),','(1, 1),')
    candidate=D/'dedup-loftr-position-floor-candidate-20261012.json';c=json.loads(candidate.read_text())
    assert c['status']=='isolated_57_units_passed_not_adopted' and c['passed']==57
    assert all(sha(p)==v for p,v in c['pins'].items())
    assert sha(source/n)==sha(Path(c['source'])/'src/loftr_model.rs')
    oldh=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-current-stack-harness-20261012');h=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-position-floor-harness-20261012')
    assert (h/'Cargo.toml').read_text()==(oldh/'Cargo.toml').read_text().replace(str(base),str(source))
    assert sha(h/'Cargo.lock')==sha(oldh/'Cargo.lock')
    old_files={str(p.relative_to(oldh/'src')):sha(p) for p in (oldh/'src').rglob('*') if p.is_file()}
    new_files={str(p.relative_to(h/'src')):sha(p) for p in (h/'src').rglob('*') if p.is_file()};assert old_files==new_files
    cmd=b['command'].copy();cmd[4]=r['command'][4];assert cmd==r['command']
    evidence=json.loads(Path(r['stdout']).read_text());assert evidence==r['evidence']
    assert json.loads(Path(b['stdout']).read_text())==b['evidence']
    same=evidence==b['evidence'];assert same==r['evidence_identical']
    def rss(p):return int(re.search(r'(\d+)\s+maximum resident set size',Path(p).read_text())[1])
    measured=rss(r['stderr']);baseline=rss(b['stderr']);assert measured==r['rss_bytes'] and baseline==b['rss_bytes']
    delta=measured-baseline;assert delta==r['rss_delta_vs_current_stack']
    out=D/'dedup-loftr-position-floor-complete-audit-20261012.json';assert not out.exists()
    result=dict(status='verified_complete_candidate_not_adopted',entire_evidence_identical=same,rss_bytes=measured,baseline_rss_bytes=baseline,rss_delta_bytes=delta,eligible_for_fixed_case_adoption=same and delta<0,production_delta=changed,unit_tests=57,pins={str(p):sha(p) for p in [report,bp,candidate,Path(__file__)]},scope='One fixed full bidirectional primary/recovery caller and 57 native units. No general memory bound, speedup, corpus, gradients, other-platform or all500 completion claim. Adoption remains separate.')
    out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['pins','scope']}))
if __name__=='__main__':main()
