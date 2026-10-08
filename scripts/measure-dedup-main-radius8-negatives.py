"""Same-recipe full different-origin gates for the four radius8 recoveries.
Retains native errors and false supports; no verdict inferred from partial rows.
"""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');b=Path('docs/research');exe=root/'gradient-scales-probe-screen-filter8'
pos=b/'dedup-main-photometric-misses-radius8-audit.json';audit=json.loads(pos.read_text());assert audit['status']=='verified_fixed_geometry_radius8_all_photometric_misses';queries=[v['query'] for v in audit['summary'] if v['supported_regions']];assert set(queries)=={'205901.jpg','206301.jpg','207002.jpg','207401.jpg'}
prepared=root/'prepared.json';m=json.loads(prepared.read_text());originals=m['images']['original'];assert len(originals)==157
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(h(k)==v for k,v in audit['input_hashes'].items())
strong={v['filename']:v for v in m['images']['strong']};files=[exe,pos,prepared,Path(__file__)]
for v in originals:
 p=root/'original-resolution-negative-200201'/v['filename'];assert h(p)==v['source_sha256'];files.append(p)
for query in queries:
 p=root/'original-resolution-strong-all'/query;assert h(p)==strong[query]['source_sha256'];files.append(p)
pins={str(p):h(p) for p in files};output=b/'dedup-main-radius8-full-negatives.json';assert not output.exists()
report={'status':'running_negatives','queries':queries,'required_negatives':624,'results':[],'input_hashes':pins,'scope':'Same native radius8 candidate+confirmation recipe on all156 different publisher-origin originals for each of four recovered queries. Finite origin negatives; not semantic/burst precision or automatic default promotion.'}
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(output)
save()
for query in queries:
 target=root/'original-resolution-strong-all'/query;group=strong[query]['group_id'];negatives=[v for v in originals if v['group_id']!=group];assert len(negatives)==156
 for v in negatives:
  source=root/'original-resolution-negative-200201'/v['filename'];checks=[exe,source,target,pos]
  assert all(h(p)==pins[str(p)] for p in checks)
  run=subprocess.run([str(exe),'original-managed-candidate-union-filter8',str(source),str(target)],capture_output=True,text=True)
  assert all(h(p)==pins[str(p)] for p in checks)
  row={'original':v['filename'],'original_group':v['group_id'],'query':query,'query_group':group,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
  if run.returncode==0:
   e=json.loads(run.stdout);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024;row['evidence']=e
  report['results'].append(row);save()
assert all(h(p)==v for p,v in pins.items());assert len(report['results'])==624;report['status']='complete';save()
