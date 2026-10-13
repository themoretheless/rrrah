#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-scheduled-loftr-native-runtime-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_scheduled_loftr_native_controls';assert all(sha(p)==h for p,h in r['pins'].items())
 mp=Path(r['source_manifest']);m=json.loads(mp.read_text());snapshot=Path(m['snapshot']);assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items());assert sha(m['producer'])==m['producer_sha256'];pp=Path(m['parent_manifest']);assert sha(pp)==m['parent_manifest_sha256'];parent=json.loads(pp.read_text())
 assert {p for p,h in m['file_hashes'].items() if parent['file_hashes'].get(p)!=h}==set(m['changes'])
 assert [x['case'] for x in r['results']]==['proposed_other_self_fallback','no_hints_self','missing_weights'];assert [x['returncode'] for x in r['results']]==[0,0,1]
 rows=[]
 for row in r['results']:
  assert row['status']=='passed_bounded_control' and sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr']
  if row['returncode']:
   assert not row['stdout'].strip() and ('No such file' in row['stderr'] or 'not found' in row['stderr']);continue
  value=json.loads(row['stdout']);n=3 if row['case']=='proposed_other_self_fallback' else 2
  assert value['files']==list(range(n));assert len(value['pairs'])==n*(n-1)//2;assert [(p['left'],p['right']) for p in value['pairs']]==[(i,j) for i in range(n) for j in range(i+1,n)]
  assert value['proposal_pairs']==([[1,2]] if n==3 else []);assert value['managed_used']==0 and 0<value['managed_peak']<=512*1024*1024
  positive=value['pairs'][0]['evidence'];assert positive['supported'] is True and positive['terminal']=='model_tiles';assert positive['attempts'];model=positive['attempts'][-1]['evidence'];assert model['supported'] is True
  pixels=model['pixels'];assert pixels['supported'] is True and all(a>=10 for a in pixels['anchors']) and len(pixels['directions'])==2
  for direction in pixels['directions']:
   assert direction['sites']>0 and direction['informative_pairs']>=1000 and direction['valid_sites']/direction['sites']>=0.3 and direction['agreeing_pairs']/direction['informative_pairs']>=0.9
  rss=re.findall(r'^\s*(\d+)\s+maximum resident set size$',row['stderr'],re.M)
  rows.append({'case':row['case'],'pairs':len(value['pairs']),'managed_peak':value['managed_peak'],'observed_maximum_resident_bytes':int(rss[0]) if len(rss)==1 else None,'self_model_anchors':pixels['anchors'],'decisions':row['decisions']})
 generic_path=D/'dedup-scheduled-portfolio-audit-20261011.json';g=json.loads(generic_path.read_text());assert g['status']=='complete_bounded_scheduled_portfolio_generic_gate' and g['tests']==55
 for p,h in g['pins'].items():assert sha(p)==h
 for rel in ['collection.rs','portfolio_collection.rs','pair_schedule.rs']:
  path=Path('/Users/themoretheless/.codex/tmp/rrrah-scheduled-portfolio-qualified-20261011/source/crates/rrrah-dedup/src')/rel;assert sha(path)==sha(snapshot/'crates/rrrah-dedup/src'/rel)
 out={'status':'complete_bounded_frozen_scheduled_loftr_native_audit','source_manifest':str(mp),'frozen_files':len(m['file_hashes']),'controls':3,'successful_controls':2,'successful_pair_records':4,'expected_missing_weight_failures':1,'optimized_emitted_runtime_packages':r['optimized_emitted_runtime_packages'],'generic_tests':55,'results':rows,'pins':{str(p):sha(p) for p in [rp,mp,Path(__file__),generic_path]},'limits':['Only two distinct normalized 320-side images, model max-side160 and diagnostic untagged-linear assumption','Different-origin decisions are observed outputs, not semantic-negative calibration','No full corpus/original-resolution accuracy, hard RSS, kernel-interruption or cross-platform qualification','Initial producer stopped before runtime because it incorrectly included host BLAKE3 opt-level0; corrected producer validates emitted normal-runtime dependencies separately']}
 (D/'dedup-scheduled-loftr-native-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'],out['controls'],out['successful_pair_records'])
if __name__=='__main__':main()
