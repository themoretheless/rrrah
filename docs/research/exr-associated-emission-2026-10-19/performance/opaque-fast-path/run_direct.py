from pathlib import Path
import subprocess,json,statistics
p=Path('/tmp/rrrah-associated-perf-20261019'); rows=[]
for fixture in ['opaque','fractional']:
 for round in range(4):
  for variant in (['after','optimized'] if round%2==0 else ['optimized','after']):
   r=subprocess.run([str(p/(variant+'-bench')),str(p/(fixture+'.exr')),'0','7','--managed-srgb'],capture_output=True,text=True,check=True)
   (p/f'{fixture}-{round}-{variant}.stdout').write_text(r.stdout); (p/f'{fixture}-{round}-{variant}.stderr').write_text(r.stderr)
   row=json.loads(r.stdout); row.update(fixture=fixture,round=round,variant=variant);rows.append(row)
summary={}
for fixture in ['opaque','fractional']:
 f=[r for r in rows if r['fixture']==fixture]; assert len({r['display_blake3'] for r in f})==1
 med={v:statistics.median(r['cpu_ready_p50_ms'] for r in f if r['variant']==v) for v in ['after','optimized']}
 summary[fixture]={'median_cpu_ready_p50_ms':med,'after_change_percent':100*(med['optimized']/med['after']-1),'hash_exact':True}
(p/'direct-optimized-results.json').write_text(json.dumps({'rows':rows,'summary':summary},indent=2)+'\n'); print(json.dumps(summary,indent=2))
