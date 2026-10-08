"""Fixed twelve controls, filter0 parity then explicit filter3/filter8 study."""
import hashlib,json,shutil,subprocess
from pathlib import Path
base=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
reference=base/'dedup-rank-region-real.json';old=json.loads(reference.read_text());assert old['status']=='complete_native_rank_diagnostic'
audit=base/'dedup-rank-region-real-audit.json';qualified=json.loads(audit.read_text());assert qualified['status']=='verified_rank_real_membership_and_taps' and qualified['input_hashes'][str(reference)]==digest(reference)
resolved=[]
for name,expected in old['input_hashes'].items():
 p=Path(name)
 if digest(p)!=expected:
  assert p==Path('crates/rrrah-dedup/src/rank_region.rs')
  p=base/'dedup-rank-region-before-filter.rs'
 assert digest(p)==expected,name
 resolved.append(p)
exe=root/'filtered-rank-region-probe';assert not exe.exists();shutil.copy2('target/debug/examples/filtered_rank_region_probe',exe)
paths=[reference,audit,exe,Path(__file__),Path('crates/rrrah-dedup/examples/filtered_rank_region_probe.rs'),Path('crates/rrrah-dedup/src/rank_region.rs'),*resolved]
pins={str(p):digest(p) for p in paths};output=base/'dedup-filtered-rank-real.json';assert not output.exists()
report={'status':'running','required_cases':36,'input_hashes':pins,'rows':[],'scope':'Same twelve supplied-model/domain controls with explicit filter radii0,3,8. Radius0 must reproduce full original evidence exactly. Neighbor radius8/contrast.005/minpairs1000 unchanged; increased declared reads cover added box taps. No copy decision, broad precision or automatic fold recovery.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save()
for radius in [0,3,8]:
 for prior in old['rows']:
  result=subprocess.run([str(exe),prior['source'],prior['query'],prior['input'],str(radius)],capture_output=True,text=True)
  assert all(digest(k)==v for k,v in pins.items())
  row={'case':prior['case'],'filter_radius':radius,'source':prior['source'],'query':prior['query'],'input':prior['input'],'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr};report['rows'].append(row);save();assert result.returncode==0
  row['evidence']=json.loads(result.stdout);assert row['evidence']['managed_used']==0
  if radius==0:assert row['evidence']==prior['evidence'],prior['case']
  save();print(radius,prior['case'],row['evidence']['directions'],flush=True)
report['status']='complete_native_filtered_rank_diagnostic';save()
