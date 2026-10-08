"""Test radius-8 confirmation on every photometric miss; retain geometry parity.
No default or threshold change. Each native result remains a diagnostic.
"""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
b=Path('docs/research');exe=root/'gradient-scales-probe-screen-filter8'
analysis=b/'dedup-main-problem-refusal-analysis.json';baseline=b/'dedup-combined-domain-release-original-full.json';old=json.loads(baseline.read_text());rows=json.loads(analysis.read_text())['rows'];rows=[r for r in rows if r['reason']=='photometric' and r['query']!='205901.jpg'];assert len(rows)==15
strong=json.loads((b/'dedup-original-strong-inputs.json').read_text());paths={r['name']:Path(r['path']) for r in strong['records']}
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
known=json.loads((b/'dedup-screen-filter8.json').read_text());assert known['input_hashes'][str(exe)]==h(exe)
files=[exe,analysis,baseline,Path(__file__)]
for r in rows:files += [root/'original-resolution-negative-200201'/r['original'],paths[r['query']]]
pins={str(p):h(p) for p in files};output=b/'dedup-main-photometric-misses-radius8.json';assert not output.exists()
report={'status':'running_pairs','required_pairs':15,'results':[],'input_hashes':pins,'scope':'All 15 other photometric misses;205901 separate controlled run. Radius8 linear confirmation, unchanged .9/min1000/coverage. No broad precision or default promotion.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(output)
save()
for r in rows:
 assert all(h(p)==v for p,v in pins.items())
 p=subprocess.run([str(exe),'original-managed-candidate-union-filter8',str(root/'original-resolution-negative-200201'/r['original']),str(paths[r['query']])],capture_output=True,text=True)
 assert all(h(path)==v for path,v in pins.items())
 row={'original':r['original'],'query':r['query'],'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 if p.returncode==0:
  e=json.loads(p.stdout);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
  initial=next(v['evidence'] for v in old['results'] if v['query']==r['query'])
  row['evidence']=e;row['same_geometry']=all(e[k]==initial[k] for k in ['correspondences','matrix','inliers'])
  row['same_region_domains']=[v['domains'] for v in e['regions']]==[v['domains'] for v in initial['regions']]
 report['results'].append(row);save()
report['status']='complete';save()
