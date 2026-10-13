#!/usr/bin/env python3
"""Audit explicit unknown collection source, copied tests and real corpus replay."""
import gzip,hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-outcome-collection-snapshot-20261011.json';gp=D/'dedup-outcome-collection-generic-20261011.json';np=D/'dedup-outcome-collection-native-opt3-20261011.json'
 m=json.loads(mp.read_text());g=json.loads(gp.read_text());n=json.loads(np.read_text())
 assert g['status']=='complete_bounded_frozen_outcome_collection_generic_execution' and g['tests']==202 and len(g['results'])==24 and g['build_returncode']==0
 assert g['source_manifest_sha256']==sha(mp)
 assert n['status']=='complete_bounded_policy_equivalent_corpus_unknown_replay' and n['invocations']==3
 for report in [g,n]:
  for p,h in report['pins'].items():assert sha(p)==h,p
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 pp=Path(m['parent_manifest']);assert sha(pp)==m['parent_manifest_sha256'];parent=json.loads(pp.read_text())
 assert sorted(p for p,h in m['file_hashes'].items() if parent['file_hashes'].get(p)!=h)==sorted(m['changes'])
 for p in m['changes']:assert sha(ROOT/p)==m['file_hashes'][p],p
 for row in g['results']:
  assert row['status']=='passed' and row['returncode']==0 and sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256']
 assert sum(row['tests'] for row in g['results'])==202
 assert {row['suite']:row['tests'] for row in g['results']}['portfolio_collection']==12
 for row in n['results']:
  assert row['status']=='passed' and sha(row['log'])==row['log_sha256']
  if row['case']=='missing_weights':assert row['returncode']!=0 and not row['stdout'].strip()
  else:assert row['returncode']==0 and json.loads(row['stdout'])==row['evidence'] and row['evidence']['managed_used']==0
 replay=n['results'][0];assert replay['case']=='corpus_unknown'
 evidence=replay['evidence']['pairs'][0]['evidence'];assert evidence['supported'] is None and evidence['terminal']=='inconclusive'
 old=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/current-all-query-origins-qualified-20261011/00023.json.gz')
 previous=json.loads(gzip.open(old,'rt').read());assert evidence['reason'] in previous['stderr']
 assert replay['command'][-5:]==['640','3.4214785442741977','model-first','/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/original-resolution-negative-200201/202300.jpg','/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/original-resolution-strong-all/200001.jpg']
 op=D/'dedup-outcome-collection-opt3-generic-20261011.json';o=json.loads(op.read_text())
 assert o['status']=='complete_bounded_frozen_outcome_collection_opt3_generic_execution' and o['tests']==16 and o['build_returncode']==0
 for p,h in o['pins'].items():assert sha(p)==h,p
 for row in o['results']:
  assert row['status']=='passed' and row['returncode']==0 and sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256']
 assert {row['suite']:row['tests'] for row in o['results']}=={'rrrah_dedup':4,'portfolio_collection':12}
 artifacts=D/'dedup-outcome-collection-native-opt3-build-artifacts-20261011.jsonl'
 own=[r for r in map(json.loads,artifacts.read_text().splitlines()) if r.get('reason')=='compiler-artifact' and r['target']['name']=='rrrah_dedup']
 assert len(own)==1 and own[0]['profile']['opt_level']=='3' and m['snapshot'] in own[0]['package_id']
 out={'status':'complete_bounded_explicit_unknown_collection_opt3_qualification','generic_tests':202,'opt3_tests':16,'programs':24,'native_invocations':3,'frozen_files':len(m['file_hashes']),'source_changes':m['changes'],'current_changed_sources_match':True,'corpus_geometry_reason_matches_previous_error_exactly':True,'pins':{str(p):sha(p) for p in [mp,gp,np,pp,old,op,artifacts,Path(__file__)]},'limits':['Only the new outcome API retains unknowns; legacy API behavior remains compatible','One policy-equivalent real corpus unknown plus self-support and missing-weights controls','Full corpus, Linux current source, indexed scaling, hard Torch RSS and all-format support remain open']}
 (D/'dedup-outcome-collection-native-opt3-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
