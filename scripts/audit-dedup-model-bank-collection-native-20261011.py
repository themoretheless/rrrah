#!/usr/bin/env python3
import hashlib,json,re,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-model-bank-collection-native-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_real_cpu_model_bank_collection' and r['returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['log'])==r['log_sha256'];assert Path(r['log']).read_text()==r['stdout']+r['stderr'];pins[r['log']]=r['log_sha256']
 mp=D/'dedup-model-bank-collection-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items())
 gp=D/'dedup-model-bank-collection-verification-20261011.json';g=json.loads(gp.read_text());assert g['status']=='verified_110_tests_model_bank_collection_routing';assert all(sha(k)==v for k,v in g['pins'].items())
 gm=json.loads((D/'dedup-model-bank-collection-snapshot-20261011.json').read_text());assert all(m['file_hashes'][k]==gm['file_hashes'][k] for k in ['crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/src/search_portfolio.rs','crates/rrrah-dedup/src/model_banks.rs','crates/rrrah-dedup/src/joint_witnesses.rs','crates/rrrah-dedup/src/model_compare.rs','crates/rrrah-dedup/src/geometry.rs'])
 runtime=D/'dedup-loftr-native-20261010-runtime-pins.json';rt=json.loads(runtime.read_text());libs={k:v for k,v in rt['pins'].items() if k.endswith('.dylib')};assert libs and all(sha(k)==v for k,v in libs.items());pins.update(libs)
 v=json.loads(r['stdout']);assert v['files']==[2,6,9] and v['managed_used']==0 and v['managed_retained_before_drop']>0
 pairs=v['pairs'];assert [(p['left'],p['right']) for p in pairs]==[(2,6),(2,9),(6,9)];assert all(p['support'] is True for p in pairs)
 ref=D/'dedup-model-bank-portfolio-native-20261011.json';previous=json.loads(json.loads(ref.read_text())['stdout']);bits=lambda h:[[struct.unpack('>Q',struct.pack('>d',x))[0] for x in row] for row in h]
 for pair in pairs[1:]:
  d=pair['detail'];assert d['attempted_stages']==1 and d['first_miss']=='None';assert d['selected_points']==989 and d['geometry_inliers']==18;assert bits(d['transform'])==bits(previous['transform']);assert d['pixels']==previous['pixels']
 counts=[]
 for pair in pairs:
  d=pair['detail'];assert d['attempted_stages']==1 and d['first_miss']=='None';rows=[tuple(map(int,x)) for x in re.findall(r'sites: (\d+), valid_sites: (\d+), informative_pairs: (\d+), agreeing_pairs: (\d+)',d['pixels'])];assert len(rows)==2 and all(info>=1000 and valid/sites>=.3 and agree/info>=.9 for sites,valid,info,agree in rows);counts.append(rows)
 third=Path(r['command'][-1]);assert sha(third)==sha(r['plan']['left'])
 rss=re.search(r'(\d+)\s+maximum resident set size',r['stderr']);assert rss
 pins.update({str(x):sha(x) for x in [p,mp,gp,runtime,ref,Path(__file__)]});out=D/'dedup-model-bank-collection-native-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_actual_cpu_three_id_model_bank_collection','pins':pins,'files':[2,6,9],'pair_support':[True,True,True],'cross_pair_reference_transform_and_pixels_identical':True,'pixel_counts':counts,'elapsed_seconds':r['elapsed_seconds'],'max_rss_bytes':int(rss[1]),'managed_peak':v['managed_peak'],'managed_retained_before_drop':v['managed_retained_before_drop'],'managed_used':0,'scope':'Real CPU exhaustive3-ID bank collection with duplicate source bytes;110-test core coherent, cached CPU wrapper, original pixels for all pairs and exact pair-reference parity. Not diverse corpus/negative precision/scheduled CPU/NativeFirst CPU/scalability/all500/RSS cap qualification.'},indent=2)+'\n');print('Verified actual CPU3-ID bank collection')
if __name__=='__main__':main()
