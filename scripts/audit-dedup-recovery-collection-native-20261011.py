#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-recovery-collection-native-audit-20261011.json';assert not out.exists();rp=D/'dedup-recovery-collection-native-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_native_recovery_collection_controls' and len(r['results'])==r['required']==4
 pins={str(rp):sha(rp),str(Path(__file__)):sha(Path(__file__))};pins.update(r['pins']);m=json.loads(Path(r['source_manifest']).read_text());snapshot=Path(m['snapshot']);assert all(sha(snapshot/f)==h for f,h in m['file_hashes'].items());assert all(sha(p)==h for p,h in pins.items())
 metadata=T/'dedup-recovery-collection-native-metadata-20261011.json';meta=json.loads(metadata.read_text());packages={p['id']:p for p in meta['packages']};nodes={n['id']:n for n in meta['resolve']['nodes']};todo=[meta['resolve']['root']];normal=set()
 while todo:
  pid=todo.pop()
  if pid in normal or any('proc-macro' in t['kind'] for t in packages[pid]['targets']):continue
  normal.add(pid);todo.extend(d['pkg'] for d in nodes[pid]['deps'] if any(k['kind'] is None for k in d['dep_kinds']))
 build=T/'dedup-recovery-collection-native-build-20261011.jsonl';compiled=[json.loads(line) for line in build.read_text().splitlines() if json.loads(line).get('reason')=='compiler-artifact'];emitted={e['package_id'] for e in compiled if not set(e['target']['kind']) & {'custom-build','proc-macro'}};optimized={e['package_id'] for e in compiled if not set(e['target']['kind']) & {'custom-build','proc-macro'} and e['profile']['opt_level']=='3'};assert normal & emitted <= optimized
 for name in ['rrrah_dedup','rrrah_decode','loftr','tch','recovery_collection_files']:
  found=[e for e in compiled if e['target']['name']==name];assert found and all(e['profile']['opt_level']=='3' for e in found)
 generic=D/'dedup-recovery-collection-fixed-current-audit-20261011.json';g=json.loads(generic.read_text());assert g['status']=='complete_bounded_current_source_recovery_collection_tests' and g['tests']==52
 for f in ['anchor_rank_file.rs','search_portfolio.rs','portfolio_collection.rs','model_loftr.rs']:
  rel='crates/rrrah-dedup/src/'+f;p=Path(next(path for path in g['archive_pins'] if path.endswith('/source/'+rel)));assert sha(p)==g['archive_pins'][str(p)]==sha(snapshot/rel)
 ap=D/'dedup-opposite-native-20261011.json';old=json.loads(ap.read_text());old_positive=json.loads(old['results'][0]['stdout']);decisions=[]
 for plan,result in zip(r['plan'],r['results']):
  assert result['case']==plan['case'];assert sha(result['log'])==result['log_sha256'];assert Path(result['log']).read_text()==result['stdout']+result['stderr'];pins[result['log']]=result['log_sha256'];rss=re.search(r'(\d+)\s+maximum resident set size',result['stderr']);v={'case':result['case'],'status':result['status'],'elapsed_seconds':result['elapsed_seconds'],'max_rss_bytes':int(rss[1]) if rss else None}
  if result['returncode']==0:
   value=json.loads(result['stdout']);assert value['managed_used']==0;v['decisions']=[p['evidence']['supported'] for p in value['pairs']]
   for pair in value['pairs']:
    e=pair['evidence'];assert e['supported'] is None or isinstance(e['supported'],bool)
    if e['supported']:
     pixels=e.get('pixels')
     if pixels is not None:
      assert min(pixels['anchors'])>=10
      for d in pixels['directions']:assert d['informative_pairs']>=1000 and d['valid_sites']/d['sites']>=.3 and d['agreeing_pairs']/d['informative_pairs']>=.9
   if plan['expected']=='supported':
    e=value['pairs'][0]['evidence'];assert e['transform']==old_positive['transform'] and e['pixels']==old_positive['pixels'];assert e['native_recipes']==old_positive['attempted_recipes']==1
  decisions.append(v)
 pins[str(metadata)]=sha(metadata);pins[str(generic)]=sha(generic);pins[str(ap)]=sha(ap);assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_bounded_native_recovery_collection','pins':pins,'optimized_normal_emitted_packages':len(normal & emitted),'generic_tests':52,'results':decisions,'scope':'Exact original reverse recovery matches previous explicit-stage transform and pixels at unchanged thresholds. Other original unknown remains unknown. Normalized three-file schedule and missing weights controls; not full-corpus/negative precision/scalability/RSS cap/platform completion.'},indent=2)+'\n');print('verified_bounded_native_recovery_collection')
if __name__=='__main__':main()
