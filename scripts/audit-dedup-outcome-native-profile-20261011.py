#!/usr/bin/env python3
"""Verify default diagnostic profile matches qualified explicit full opt-level3."""
import hashlib,json,shutil
from collections import Counter
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-outcome-native-profile-snapshot-20261011.json';m=json.loads(mp.read_text());pp=Path(m['parent_manifest']);parent=json.loads(pp.read_text())
 assert sha(pp)==m['parent_manifest_sha256']
 assert m['changes']==['scripts/dedup-loftr-native-probe/Cargo.toml']
 for p,h in m['file_hashes'].items():assert sha(Path(m['snapshot'])/p)==h,p
 assert set(m['file_hashes'])==set(parent['file_hashes'])
 assert [p for p,h in m['file_hashes'].items() if parent['file_hashes'][p]!=h]==m['changes']
 changed=m['changes'][0];assert sha(ROOT/changed)==m['file_hashes'][changed]
 expected=(Path(parent['snapshot'])/changed).read_text()+'\n# Keep complete-source guards and generic native search practical in diagnostics.\n[profile.dev]\nopt-level = 3\n'
 assert (ROOT/changed).read_text()==expected
 old=D/'dedup-outcome-collection-native-fullopt3-isolated-build-artifacts-20261011.jsonl';new=D/'dedup-outcome-native-profile-default-build-artifacts-20261011.jsonl'
 def profiles(path,snapshot):
  rows=[json.loads(line) for line in path.read_text().splitlines()]
  assert rows[-1]['reason']=='build-finished' and rows[-1]['success']
  result=Counter()
  for r in rows:
   if r.get('reason')!='compiler-artifact':continue
   key=[r['package_id'].replace(snapshot,'SOURCE'),r['target']['name'],r['target']['kind'],r['target']['crate_types'],r['profile'],sorted(r['features'])]
   result[json.dumps(key,sort_keys=True)]+=1
  return result,rows
 a,_=profiles(old,parent['snapshot']);b,rows=profiles(new,m['snapshot']);assert a==b,(a-b,b-a)
 binary=[r for r in rows if r.get('reason')=='compiler-artifact' and r['target']['name']=='tile_outcome_collection_files']
 assert len(binary)==1 and binary[0]['profile']['opt_level']=='3'
 source=Path(binary[0]['executable']);assert m['snapshot'].encode() in source.read_bytes()
 archive=Path('/Users/themoretheless/.codex/tmp/rrrah-outcome-native-profile-qualified-20261011');assert not archive.exists();archive.mkdir();copied=archive/source.name;shutil.copy2(source,copied);assert sha(source)==sha(copied)
 prior=D/'dedup-outcome-collection-native-optimized-audit-20261011.json';audit=json.loads(prior.read_text());assert audit['status']=='complete_bounded_explicit_unknown_collection_optimized_qualification'
 for p,h in audit['pins'].items():assert sha(p)==h,p
 out={'status':'complete_default_native_profile_build_equivalence_not_new_runtime_execution','frozen_files':len(m['file_hashes']),'profile_only_delta':changed,'artifact_profile_feature_records':sum(a.values()),'all_compiler_artifact_profiles_and_features_match_qualified_explicit_configuration':True,'binary':str(copied),'binary_sha256':sha(copied),'pins':{str(p):sha(p) for p in [mp,pp,old,new,prior,Path(__file__),D/'dedup-outcome-native-profile-default-build-20261011.log']},'scope':'Default dev profile3 matches qualified explicit override3 on the identical algorithm sources, policy and dependency lock. Archived new binary is build-verified; runtime controls are inherited source/configuration evidence, not executions of this new binary.','limits':['No universal latency improvement or corpus completeness claim','Linux and other-platform Torch, indexed scaling, provider RSS and broad format support remain separately qualified or open']}
 (D/'dedup-outcome-native-profile-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
