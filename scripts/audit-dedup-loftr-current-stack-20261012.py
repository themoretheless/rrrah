#!/usr/bin/env python3
"""Independently verify complete fixed caller execution on frozen current sources."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dedup-loftr-current-stack-runtime-20261012.json';r=json.loads(p.read_text());assert r['status'] in ['complete_identical_evidence_current_sources','complete_evidence_changed_current_sources'] and r['returncode']==r['build_returncode']==0
assert all(sha(p)==h for p,h in r['pins'].items());assert all(sha(Path(r['source'])/n)==h for n,h in r['root_source_hashes'].items())
bp=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';b=json.loads(bp.read_text())['results'][7];cmd=b['command'].copy();cmd[4]=r['command'][4];assert cmd==r['command']
e=json.loads(Path(r['stdout']).read_text());assert e==r['evidence'];same=e==b['evidence'];assert same==r['evidence_identical']
rss=int(re.search(r'(\d+)\s+maximum resident set size',Path(r['stderr']).read_text())[1]);assert rss==r['rss_bytes'];baseline=int(re.search(r'(\d+)\s+maximum resident set size',Path(b['stderr_path']).read_text())[1])
source=Path(r['source']);a=D/'dedup-loftr-sequential-backbone-adoption-20261012.json';adopt=json.loads(a.read_text());assert all(sha(source/'vendor/loftr/src'/n)==h for n,h in adopt['after'].items())
drift=[n for n,h in r['root_source_hashes'].items() if not (R/n).is_file() or sha(R/n)!=h]
out=D/'dedup-loftr-current-stack-complete-audit-20261012.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_complete_current_source_fixed_pair',whole_evidence_identical_to_historical_baseline=same,rss_bytes=rss,original_rss_bytes=baseline,rss_delta_bytes=rss-baseline,frozen_current_files=len(r['root_source_hashes']),root_drift_since_snapshot=drift,pins={str(p):sha(p) for p in [p,bp,a,Path(b['stderr_path']),Path(__file__)]},scope='Full unchanged fixed Q00992/R043984 bidirectional primary/recovery caller completed on frozen current local crates/vendor source snapshot, including exact56-unit qualified native adoption. Prior probe manifest/lock/runtime/features/build configuration retained. One pair and one-shot RSS; not all root features, full458/all500, precision corpus, physical other platforms, gradients, current live-root drift or universal memory cap.'),indent=2)+'\n');print('Verified current-source case, evidence identical:',same,'RSS:',rss,'root drift files:',len(drift))
