#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-overlap-cached-exposure-audit-20261012.json';assert not out.exists();rp=D/'dedup-disc21-overlap-cached-exposure-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_two_development_controls' and r['build_returncode']==0;assert all(sha(p)==h for p,h in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 gate=D/'dedup-overlap-exposure-search-verification-20261012.json';g=json.loads(gate.read_text());assert g['tests']==179 and all(sha(p)==h for p,h in g['pins'].items());mp=D/'dedup-overlap-exposure-search-snapshot-20261012.json';m=json.loads(mp.read_text());s=next(Path(p).parents[2]for p in r['pins']if p.endswith('/crates/rrrah-dedup/Cargo.toml'));assert all(sha(s/f)==h for f,h in m['file_hashes'].items())
 parent=D/'dedup-disc21-overlap-calibrated-strong-audit-20261011.json';a=json.loads(parent.read_text());assert all(sha(p)==h for p,h in a['pins'].items());old=json.loads((D/'dedup-disc21-overlap-calibrated-strong-20261011.json').read_text());summaries=[]
 for row in r['results']:
  assert row['status']=='complete' and row['returncode']==0;v=json.loads(row['stdout']);assert v['managed_used']==0;e=v['evidence'];assert e['attempts']==9
  previous=next(x for x in old['results']if x['query']==row['query']and x['reference']==row['reference']);b=json.loads(previous['stdout'])['evidence'];assert e['supported']==b['supported'] and e['selected_bank']==b['selected_bank'] and e['original_dimensions']==b['original_dimensions'];assert e['raw_proposals']==sum(x['raw_proposals']for x in b['banks']);assert e['selected_points']==sum(len(x['points'])for x in b['banks'])
  if e['supported']:
   accepted=next(x for x in b['pixel_attempts']if x['supported']);assert e['transform']==accepted['transform'] and e['geometry_inliers']==len(accepted['witnesses'])==24
   expected=re.search(r'anchors: (AnchorRankEvidence \{.*?supported: true \}), exposures:',accepted['pixels']).group(1);assert e['pixels']=='Some('+expected+')';assert e['pixel_rank_radius']==2 and e['pixel_window_radius']==2 and e['pixel_attempts']==3;assert e['exposures'].startswith('Some([RankExposure')
  else:assert e['pixels']=='None' and e['exposures']=='None' and e['transform'] is None
  summaries.append({'query':row['query'],'reference':row['reference'],'supported':e['supported'],'attempts':9,'selected_points':e['selected_points'],'geometry_inliers':e['geometry_inliers'],'pixel_attempts':e['pixel_attempts'],'managed_peak':v['managed_peak']})
 source=next(Path(p)for p in r['pins']if p.endswith('/src/bin/disc21_overlap_cached_exposure_files.rs'));assert 'search_loftr_model_banks_overlap_exposure_files' in source.read_text();wrapper=(s/'crates/rrrah-dedup/src/model_loftr.rs').read_text();assert 'if slot.is_none()'in wrapper and 'let cap=policy.comparison.proposals.max_proposals;let mut model=None;'in wrapper
 pins={str(p):sha(p)for p in [rp,gate,mp,parent,source,Path(__file__)]};report={'status':'verified_actual_cached_cpu_library_overlap_exposure_controls','pins':pins,'summaries':summaries,'scope':'Public generic guarded9-region exposure search and cached CPU wrapper. Actual positive/negative controls match previous research roster counts and decisions; positive selected matrix and pixel evidence exactly match. Provider cache source loads model only when shared Option is empty; no instrumented load counter or performance A/B is claimed. Negative remains inconclusive. No default/fallback composition adoption, general precision/full-cohort/full-format/platform/RSS/all500 qualification.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status']);print(summaries)
if __name__=='__main__':main()
