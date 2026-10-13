#!/usr/bin/env python3
import hashlib,json,re,statistics
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-loftr-block-masks-audit-20261011.json';assert not out.exists();rp=D/'dedup-loftr-block-masks-bench-reshape-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_block_masks_selector_benchmark' and len(r['results'])==6;assert all(sha(p)==h for p,h in r['pins'].items());mp=D/'dedup-loftr-block-masks-bench-reshape-harness-20261011.json';m=json.loads(mp.read_text());S=Path(m['harness']);assert all(sha(S/f)==h for f,h in m['files'].items());generic=D/'dedup-loftr-block-masks-tests-audit-20261011.json';g=json.loads(generic.read_text());assert g['tests']==48 and all(sha(p)==h for p,h in g['pins'].items());assert sha(S/'src/coarse_matching.rs')==g['production_source_sha256']==sha(R/'vendor/loftr/src/coarse_matching.rs');pins={str(p):sha(p) for p in [rp,mp,generic,Path(__file__)]}
 expected=None
 for row in r['results']:
  assert sha(row['log'])==row['log_sha256'];s=Path(row['log']).read_text();assert 'test result: ok. 1 passed; 0 failed;' in s;indices=json.loads(re.search(r'MASK_INDICES=(\[[^\n]*\])',s)[1]);assert indices==row['indices'];assert row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',s)[1]);expected=indices if expected is None else expected;assert indices==expected;pins[row['log']]=row['log_sha256']
 values={mode:{'median_rss_bytes':statistics.median([v['rss_bytes'] for v in r['results'] if v['mode']==mode]),'median_elapsed_seconds':statistics.median([v['elapsed_seconds'] for v in r['results'] if v['mode']==mode])} for mode in ['dense','blocked']};saved=values['dense']['median_rss_bytes']-values['blocked']['median_rss_bytes'];assert saved>0
 out.write_text(json.dumps({'status':'verified_bounded_block_mask_memory_reduction','pins':pins,'tests':48,'matches':len(expected)//3,'measurements':values,'median_rss_saved_bytes':saved,'median_rss_reduction_fraction':saved/values['dense']['median_rss_bytes'],'scope':'Three fresh CPU processes per selector,2threads,6400x6400 synthetic confidence. All ordered indices identical;48 current-source tests passed separately. Selector peak RSS measured, not full LoFTR/provider/collection RSS or a hard cap. Earlier loader and diagnostic flatten failures preserved. Full combined recovery replay uses older pinned LoFTR, excluding this change.'},indent=2)+'\n');print(json.dumps(values));print('saved RSS bytes',saved)
if __name__=='__main__':main()
