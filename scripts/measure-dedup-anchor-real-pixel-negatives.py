"""Unrelated real target pixels under borrowed positive geometry; not retrieval precision."""
import hashlib,json,struct,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');pixels=T/'all-photometric-local-rank';exe=T/'anchor-rank-pixels-probe-qualified';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();out=D/'dedup-anchor-real-pixel-negatives.json';assert not out.exists()
base=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());manifest=T/'prepared.json';m=json.loads(manifest.read_text());groups={x['filename']:x['group_id'] for x in m['images']['strong']};files=[manifest,exe,Path(__file__),D/'dedup-anchor-rank-native-real.json'];dims={}
for p in pixels.glob('*.rgba32'):
 with p.open('rb') as f:assert f.read(21)==b'RRRAH-RANK-RGBA32-V1\n';dims[p.stem]=struct.unpack('<II',f.read(8))
queries=[v['query'][:-4] for v in base['results'] if (pixels/f"{v['query'][:-4]}.rgba32").exists()]
rows=[]
for original,query in [('202500','202502'),('212600','212602'),('204700','204702'),('200500','200501')]:
 model=T/'anchor-rank-native'/f'{query}-model.txt';points=T/'anchor-rank-native'/f'{query}-points.txt'
 for other in queries:
  if other==query or dims[other]!=dims[query]:continue
  assert groups[query+'.jpg']!=groups[other+'.jpg']
  args=[pixels/f'{original}.rgba32',pixels/f'{other}.rgba32',model,points];pins={str(p):h(p) for p in args};run=subprocess.run([str(exe),*map(str,args)],capture_output=True,text=True);assert run.returncode==0,run.stderr
  evidence=[json.loads(line) for line in run.stdout.splitlines()];assert len(evidence)==3 and all(x['status']=='ok' for x in evidence),evidence
  assert all(h(p)==pins[str(p)] for p in args);files.extend(args)
  rows.append({'borrowed_geometry_query':query+'.jpg','original':original+'.jpg','unrelated_query':other+'.jpg','evidence':evidence})
files=list(dict.fromkeys(files));result={'status':'complete_real_pixel_borrowed_geometry_negatives','pairs':len(rows),'false_supports_filter8':sum(e['supported'] for r in rows for e in r['evidence'] if e['filter_radius']==8),'rows':rows,'pins':{str(p):h(p) for p in files},'scope':'Different-origin normalized real targets with identical frame dimensions replace the positive target pixels, while full original positive model/global inlier points remain unchanged. Every call reaches native pixel counts. Adversarial supplied geometry only, not actual descriptor-qualified negatives or end-to-end retrieval precision.'};out.write_text(json.dumps(result,indent=2)+'\n');print('pairs',len(rows),'filter8 false supports',result['false_supports_filter8'])
