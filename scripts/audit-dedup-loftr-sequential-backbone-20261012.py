#!/usr/bin/env python3
"""Audit isolated sequential candidate source, units, complete evidence and RSS."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dedup-loftr-sequential-backbone-runtime-20261012.json';r=json.loads(p.read_text())
assert r['status'] in ['complete_identical_evidence_candidate_not_adopted','complete_evidence_changed_not_adopted']
assert r['returncode']==r['unit_returncode']==0
assert all(sha(p)==h for p,h in r['pins'].items())
mp=D/'dedup-disc21-heldout-negative-source-20261012.json';m=json.loads(mp.read_text());source=Path(r['source']);name='vendor/loftr/src/loftr_model.rs'
code=(Path(m['snapshot'])/name).read_text();call=r['original_call'];assert code.count(call)==1;code=code.replace(call,'self.backbone_features(&image0, &image1, cancel)?')
for removed in [r['removed_stacked_branch'],r['removed_arguments']]:assert code.count(removed)==1;code=code.replace(removed,'')
assert code==(source/name).read_text();assert all(sha(source/n)==h for n,h in m['file_hashes'].items() if n!=name)
log=Path(r['unit_log']).read_text();v=re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',log);assert v
assert 'sequential_backbone_matches_stacked_features_exactly ... ok' in log
bp=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';base=json.loads(bp.read_text())['results'][7]
cmd=base['command'].copy();cmd[4]=r['command'][4];assert cmd==r['command']
e=json.loads(Path(r['stdout']).read_text());assert e==r['evidence'];same=e==base['evidence'];assert same==r['evidence_identical']
rss=int(re.search(r'(\d+)\s+maximum resident set size',Path(r['stderr']).read_text())[1]);assert rss==r['rss_bytes']
original=int(re.search(r'(\d+)\s+maximum resident set size',Path(base['stderr_path']).read_text())[1]);diag=json.loads((D/'dedup-loftr-negative-rss-complete-audit-20261012.json').read_text())['rss_peak_bytes']
out=D/'dedup-loftr-sequential-backbone-complete-audit-20261012.json';assert not out.exists()
out.write_text(json.dumps(dict(status='verified_isolated_candidate_not_adopted',native_units_passed=int(v[1]),entire_evidence_identical=same,rss_bytes=rss,original_uninstrumented_rss_bytes=original,original_instrumented_rss_bytes=diag,rss_delta_vs_original_bytes=rss-original,rss_delta_vs_instrumented_bytes=rss-diag,pins={str(p):sha(p) for p in [p,mp,bp,Path(__file__)]},scope='One complete fixed worst negative pair and frozen native unit source. One-shot RSS measurements, no repeatability or timing benchmark. Current native stack differs; no root adoption/full458/full500/platforms/hard total memory cap qualification.'),indent=2)+'\n')
print('Verified candidate; evidence identical:',same,'RSS:',rss,'delta:',rss-original)
