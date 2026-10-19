from pathlib import Path
import subprocess,os,json,statistics
p=Path('/tmp/rrrah-prefetch-policy-ab-20261019');p.mkdir(exist_ok=True)
binary='/tmp/rrrah-alpha-isolated-target-20261016/debug/deps/rrrah-b180ac92e3f43d6e'
rows=[]; references=None
for round, policy in enumerate(['legacy','admission','admission','legacy','legacy','admission']):
 env=dict(os.environ,RRRAH_COLLECTION_VARY_PREFETCH='1',RRRAH_COLLECTION_MANIFEST='/tmp/rrrah-external-mixed-collection/manifest-1024.json',RRRAH_COLLECTION_REPORT=str(p/f'{round}-{policy}.json'),RRRAH_BENCH_LEGACY_PREFETCH_SWAP='1' if policy=='legacy' else '0')
 with (p/f'{round}-{policy}.log').open('w') as log:
  subprocess.run([binary,'external_mixed_collection_preserves_navigation_prefetch_ttl_swap_and_pressure','--ignored','--nocapture'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 r=json.loads(Path(env['RRRAH_COLLECTION_REPORT']).read_text()); hashes=[(v['sha256'],v['status'],v.get('digest')) for v in r['rows']]
 if references is None:references=hashes
 assert hashes==references
 assert r['root_final']==0 and r['swap']['errors']==0
 rows.append({'round':round,'policy':policy,'seconds':r['seconds'],'swap':r['swap'],'max_pre_pressure':r['max_observed_pre_pressure_used'],'navigation':r['navigation_sources'],'nearest_hits':r['nearest_navigation_hits'],'ttl_restores':r['ttl_restores']})
 (p/'progress.json').write_text(json.dumps(rows,indent=2)+'\n')
summary={policy:{'median_seconds':statistics.median(v['seconds'] for v in rows if v['policy']==policy),'median_reads':statistics.median(v['swap']['reads'] for v in rows if v['policy']==policy)} for policy in ['legacy','admission']}
summary['admission_seconds_percent']=100*(summary['admission']['median_seconds']/summary['legacy']['median_seconds']-1)
(p/'result.json').write_text(json.dumps({'rows':rows,'summary':summary,'all_source_and_prepared_digests_equal':True},indent=2)+'\n');print(json.dumps(summary,indent=2))
