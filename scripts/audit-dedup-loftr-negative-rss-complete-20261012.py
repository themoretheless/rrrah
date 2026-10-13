#!/usr/bin/env python3
"""Independently validate completed phase RSS and entire caller evidence."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1]; D=R/'docs/research'
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dedup-loftr-negative-rss-profile-20261012.json';r=json.loads(p.read_text())
assert r['status']=='complete_identical_evidence_stage_rss_diagnostic' and r['returncode']==0
assert all(sha(p)==h for p,h in r['pins'].items())
mp=D/'dedup-disc21-heldout-negative-source-20261012.json';m=json.loads(mp.read_text());name='vendor/loftr/src/loftr_model.rs'
expected=(Path(m['snapshot'])/name).read_text()
for before,after in r['instrumentation_replacements']:
    assert expected.count(before)==1;expected=expected.replace(before,after)
expected+=r['instrumentation_helper'];assert expected==(Path(r['source'])/name).read_text()
assert all(sha(Path(r['source'])/n)==h for n,h in m['file_hashes'].items() if n!=name)
raw=Path(r['stderr']).read_text();rows=[dict(call=int(c),stage=s,rss_bytes=int(b),shapes=json.loads(sh)) for c,s,b,sh in re.findall(r'RRRAH_RSS call=(\d+) stage=(\S+) rss_bytes=(\d+) shapes=(.*)',raw)]
assert rows==r['trace'];stages=['input','backbone','coarse.features','coarse.matches','coarse.released','fine.windows','features.released','fine.transform','fine.matches']
assert sorted({v['call'] for v in rows if v['call']})==list(range(1,69))
for n in range(1,69): assert [v['stage'] for v in rows if v['call']==n]==stages
bp=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';base=json.loads(bp.read_text())['results'][7]
e=json.loads(Path(r['stdout']).read_text());assert e==base['evidence']==r['evidence']
peak=int(re.search(r'(\d+)\s+maximum resident set size',raw)[1]);assert peak==r['rss_peak_bytes']
first={v['stage']:v['rss_bytes'] for v in rows if v['call']==1}
out=D/'dedup-loftr-negative-rss-complete-audit-20261012.json';assert not out.exists()
out.write_text(json.dumps(dict(status='verified_complete_identical_evidence_stage_rss_diagnostic',complete_inferences=68,rss_peak_bytes=peak,first_inference_rss_bytes=first,first_backbone_boundary_increase_bytes=first['backbone']-first['input'],pins={str(p):sha(p) for p in [p,mp,bp,Path(__file__),Path(r['stdout']),Path(r['stderr'])]},scope='All 68 ordered inference traces and entire caller JSON match the frozen negative baseline. Boundary RSS samples do not identify tensor ownership or kernel peaks. Historical source composition; no root optimization, universal memory bound or full corpus qualification.'),indent=2)+'\n')
print('Verified 68 inference traces, identical complete evidence, peak RSS',peak)
