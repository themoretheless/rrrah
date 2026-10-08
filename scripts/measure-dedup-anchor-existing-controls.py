"""Frozen completed candidate prefix, then native anchor confirmation; no new retrieval."""
import hashlib,json,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
source=D/'dedup-local-rank-eyeglasses-controls.json';raw=source.read_bytes();data=json.loads(raw);n=len(data['results']);snapshot=D/f'dedup-anchor-controls-prefix-{n}-input.json';assert not snapshot.exists();snapshot.write_bytes(raw)
exe=T/'anchor-rank-pixels-probe-qualified'
if not exe.exists():exe.write_bytes(Path('target/release/examples/anchor_rank_pixels_probe').read_bytes());exe.chmod(0o755)
work=T/f'anchor-controls-prefix-{n}';work.mkdir(exist_ok=False)
pins={str(snapshot):h(snapshot),str(exe):h(exe),str(Path(__file__)):h(Path(__file__))};results=[]
for row in data['results']:
 assert row['candidate_returncode']==0
 e=row['candidate'];assert e['status']=='ok' and e['managed_used']==0
 result={'original':row['original'],'positive_control':row['positive_control'],'candidate_inliers':len(e['inliers']),'native':[],'supported':False}
 if e['matrix'] is not None:
  assert len(e['inliers'])>=10
  original=Path(row['original']).stem
  pixels=T/'local-rank-eyeglasses-controls'/f'{original}.rgba32';target=T/'rank-normalized-pixel-oracle'/'occlusion-214402.rgba32'
  assert h(pixels)==data['generated_hashes'][str(pixels)] and h(target)==data['input_hashes'][str(target)]
  model=work/f'{original}-model.txt';model.write_text(' '.join(str(x) for r in e['matrix'] for x in r)+'\n')
  points=work/f'{original}-points.txt';points.write_text('\n'.join(' '.join(str(x) for p in e['correspondences'][i] for x in p) for i in e['inliers'])+'\n')
  files=[pixels,target,model,points];before={str(p):h(p) for p in files}
  p=subprocess.run([str(exe),*map(str,files)],capture_output=True,text=True);assert p.returncode==0,p.stderr
  result['native']=[json.loads(line) for line in p.stdout.splitlines()];assert len(result['native'])==3
  assert all(h(p)==v for p,v in ((Path(k),v) for k,v in before.items()));pins.update(before)
  result['supported']=any(v.get('supported',False) for v in result['native'] if v['filter_radius']==8)
 results.append(result)
assert all(h(Path(k))==v for k,v in pins.items())
out=D/f'dedup-anchor-controls-prefix-{n}.json';out.write_text(json.dumps({'status':'verified_frozen_candidate_prefix_anchor_confirmation','pairs':n,'negatives':sum(not x['positive_control'] for x in results),'false_supports':sum(x['supported'] and not x['positive_control'] for x in results),'positive_supported':sum(x['supported'] and x['positive_control'] for x in results),'results':results,'pins':pins,'scope':'Completed prefix from existing frozen candidate campaign. All original global inliers supplied to native anchors. Negatives without geometry reject before pixels; no full156 precision, hard geometric negatives, semantic/burst or collection qualification.'},indent=2)+'\n');print(out)
