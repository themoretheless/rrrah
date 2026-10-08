"""Frozen current API prefix/terminal recipe and exact prior geometry parity."""
import argparse,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');args=p.parse_args();assert not args.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(args.report.read_text());digest=h(args.report);assert r['required_pairs']==3 and (r['status']=='complete' or args.checkpoint and r['status']=='running')
for path,pin in r['pins'].items():assert h(path)==pin,path
expected=['201303.jpg','207002.jpg','210202.jpg'];assert [x['query'] for x in r['results']]==expected[:len(r['results'])]
if not args.checkpoint:assert len(r['results'])==3
old=json.loads(Path('docs/research/dedup-fallback-release-original-full.json').read_text());counts={'supported':0,'pixel_rejected':0,'no_geometry':0,'refused':0,'native_error':0};rows=[]
for row in r['results']:
 if row['returncode']!=0:counts['native_error']+=1;continue
 e=row['evidence'];assert e==json.loads(row['stdout']);prior=next(x for x in old['results'] if x['query']==row['query']);assert row['original']==prior['original']
 if e['status']=='ok':
  v=prior['evidence'];assert e['matrix']==v['matrix'] and e['points']==v['correspondences'];assert e['attempted_recipes']==v['attempted_recipes'] and e['smoothing_radii']==v['smoothing_radii'];assert 1<=e['attempted_recipes']<=3 and e['smoothing_radii']==[[2,2],[4,0],[0,4]][e['attempted_recipes']-1]
  assert len(e['anchors'])==len(e['directions'])==2 and all(a>=10 for a in e['anchors']);predicates=[]
  for a,d in zip(e['anchors'],e['directions']):
   assert d['sites']==25*a and 0<=d['agreeing_pairs']<=d['informative_pairs']<=8*d['valid_sites']<=8*d['sites'];predicates.append(d['informative_pairs']>=1000 and d['valid_sites']/d['sites']>=.3 and d['agreeing_pairs']/d['informative_pairs']>=.9)
  assert e['supported']==all(predicates);counts['supported' if e['supported'] else 'pixel_rejected']+=1
 elif e['status']=='no_geometry':assert not e['supported'];counts['no_geometry']+=1
 else:assert e['status'].startswith('refusal:') and not e['supported'];counts['refused']+=1
 rows.append({'query':row['query'],'status':e['status'],'supported':e['supported']})
assert sum(counts.values())==len(r['results']) and h(args.report)==digest
snapshot=json.loads(Path('docs/research/dedup-anchor-fallback-real-source.json').read_text());assert h(snapshot['binary'])==snapshot['binary_sha256']
for rel,pin in snapshot['file_hashes'].items():assert h(Path(snapshot['snapshot'])/rel)==pin
args.output.write_text(json.dumps({'status':'verified_actual_fallback_anchor_recovered_prefix' if args.checkpoint else 'verified_actual_fallback_anchor_recovered_terminal','pairs':len(r['results']),'counts':counts,'rows':rows,'pins':{str(p):h(p) for p in [args.report,Path(__file__),Path('docs/research/dedup-anchor-fallback-real-source.json')]},'scope':'Immutable actual-file native results, exact prior complete original point/model/recipe parity for successful calls, native count bounds/support predicates. Pixel counts not independently recomputed here; finite recovered pairs, no unrelated precision/collection/Linux/full goal proof.'},indent=2)+'\n');print('verified',len(r['results']),counts)
