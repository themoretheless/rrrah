"""Real ordinal evidence controls; counts do not establish copy admission."""
import hashlib,json,shutil,subprocess
from pathlib import Path
base=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
exe=root/'rank-region-probe';assert not exe.exists();shutil.copy2('target/debug/examples/rank_region_probe',exe)
positive_path=base/'dedup-folded-training-diagnostic.json';positive=json.loads(positive_path.read_text());assert positive['status']=='complete_native_diagnostic'
negative_path=base/'dedup-affine-grid-meanfix-checkpoint-111.json';negative=json.loads(negative_path.read_text())
negative_audit=base/'dedup-affine-grid-meanfix-checkpoint-111-audit.json';audit=json.loads(negative_audit.read_text());assert audit['report_sha256']==digest(negative_path) and audit['region_supports']==5
cases=[]
for row in positive['rows']:
 source=root/'original-resolution-negative-200201'/('200500.jpg' if row['case']=='screen_control' else '201700.jpg')
 query=root/'original-resolution-strong-all'/('200501.jpg' if row['case']=='screen_control' else '201702.jpg')
 cases.append((row['case'],source,query,Path(row['input'])))
for row in negative['rows']:
 if not row.get('supports'):continue
 matrix=list(map(float,Path(row['input']).read_text().split()));assert len(matrix)==9
 for index,region in enumerate(row['evidence']['regions']):
  if not all('error' not in d and d['matched']>=.9*d['samples'] for d in region['directions']):continue
  path=root/f'rank-negative-{Path(row["source"]).stem}-{index}.txt';assert not path.exists()
  path.write_text(' '.join(map(str,[*matrix,*region['domains'][0],*region['domains'][1]]))+'\n')
  cases.append((f'negative_{Path(row["source"]).stem}_{index}',Path(row['source']),root/'original-resolution-strong-all/200501.jpg',path))
assert len(cases)==12
paths=[exe,positive_path,negative_path,negative_audit,Path(__file__),Path('crates/rrrah-dedup/examples/rank_region_probe.rs'),*[Path('crates/rrrah-dedup/src')/name for name in ['rank_region.rs','lib.rs','linear.rs','geometry.rs']]]
paths += [p for _,a,b,c in cases for p in [a,b,c]]
pins={str(p):digest(p) for p in paths};output=base/'dedup-rank-region-real.json';assert not output.exists()
report={'status':'running','input_hashes':pins,'required_cases':12,'rows':[],'scope':'One screen, six folded witnesses and five previously observed color-only negative regions. Radius8/mincontrast.005/minpairs1000 explicit in frozen probe. Target-axis offsets separately in each direction. Supplied geometry diagnostic, not automatic rank-based copy admission.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save()
for name,left,right,path in cases:
 result=subprocess.run([str(exe),str(left),str(right),str(path)],capture_output=True,text=True)
 assert all(digest(k)==v for k,v in pins.items())
 row={'case':name,'source':str(left),'query':str(right),'input':str(path),'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr};report['rows'].append(row);save();assert result.returncode==0
 row['evidence']=json.loads(result.stdout);assert row['evidence']['managed_used']==0;save();print(name,row['evidence']['directions'],flush=True)
report['status']='complete_native_rank_diagnostic';save()
