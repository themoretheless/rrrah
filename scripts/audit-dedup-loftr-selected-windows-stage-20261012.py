#!/usr/bin/env python3
import hashlib,json,re,subprocess
from collections import Counter
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 sp=D/'dedup-loftr-selected-windows-source-v2-20261012.json';s=json.loads(sp.read_text());bp=Path(s['parent_manifest']);b=json.loads(bp.read_text())
 for m in [s,b]:assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
 assert set(s['deltas_from_parent'])=={'vendor/loftr/src/fine_preprocess.rs','vendor/loftr/src/fine_preprocess/tests.rs'}
 for p,h in b['file_hashes'].items():
  if p.startswith(('vendor/loftr/src/','crates/rrrah-dedup/src/')):assert sha(R/p)==h,p
 original=(Path(b['snapshot'])/'vendor/loftr/src/fine_preprocess.rs').read_text();candidate=(Path(s['snapshot'])/'vendor/loftr/src/fine_preprocess.rs').read_text()
 start=original.index('    pub fn forward(');end=original.index('\n}\n\nfn first_dim',start);old=original[start:end]
 start=candidate.index('    #[cfg(test)]\n    fn forward_dense_reference(');end=candidate.index('\n}\n\n// CPU',start);oracle=candidate[start:end].replace('    #[cfg(test)]\n    fn forward_dense_reference(','    pub fn forward(',1);assert oracle==old
 assert 'feat.requires_grad()' in candidate
 tp=D/'dedup-loftr-selected-windows-v2-direct-tests-20261012.json';t=json.loads(tp.read_text());assert t['returncode']==0
 stagep=D/'dedup-loftr-selected-windows-stage-20261012.json';stage=json.loads(stagep.read_text());assert stage['status']=='complete_stage_only_not_adopted'
 for m in [t,stage]:assert all(sha(p)==h for p,h in m['pins'].items())
 text=Path(t['log']).read_text();passed=Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M));ignored=Counter(re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M));assert sum(passed.values())==50 and sum(ignored.values())==1 and '50 passed; 0 failed; 1 ignored' in text
 listed=subprocess.check_output(t['command'][:3]+['--list'],text=True);assert set(re.findall(r'^(.*): test$',listed,re.M))==set(passed)|set(ignored)
 assert 'fine_preprocess::tests::selected_windows_match_complete_dense_forward_with_borders_repeats_and_fallbacks' in passed
 assert 'fine_preprocess::tests::selected_window_gradient_matches_dense_repeated_border_windows' in passed
 rows=stage['results'];assert [r['mode'] for r in rows]==['dense','selected','selected','dense'];assert len({r['hash'] for r in rows})==1
 for row in rows:
  assert row['returncode']==0 and Path(row['log']).read_text()==row['stdout']+row['stderr'] and sha(row['log'])==row['log_sha256']
  assert int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1])==row['rss_bytes']
 out=D/'dedup-loftr-selected-windows-stage-audit-20261012.json';assert not out.exists();pins=dict(stage['pins']);pins.update({str(p):sha(p) for p in [sp,bp,tp,stagep,Path(__file__)]})
 out.write_text(json.dumps({'status':'verified_50_native_tests_and_four_fresh_stage_controls_not_adopted','passed':50,'ignored':1,'tests':sorted(passed),'dense_rss_bytes':[r['rss_bytes'] for r in rows if r['mode']=='dense'],'selected_rss_bytes':[r['rss_bytes'] for r in rows if r['mode']=='selected'],'stage_output_fnv64':rows[0]['hash'],'pins':pins,'scope':'Whole original dense forward retained byte-for-byte as cfg(test) oracle. Two new tests cover six exact-output shapes/configurations and gradient fallback; current baseline full dedup/LoFTR source verified. Fresh-process stage only,128channels320x320,256/6400windows. FNV64 is a noncryptographic output fingerprint. No total-model/collection gain, hard cap, isolated timing, Linux/Windows/full500 qualification or adoption.'},indent=2)+'\n');print('Verified50 native tests, one manual ignored, stage RSS dense/selected', [r['rss_bytes'] for r in rows])
if __name__=='__main__':main()
