#!/usr/bin/env python3
import hashlib,json,math,statistics,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def score(e):
 vals=[]
 for p in e['patches']:
  pairs=p['pairs'];assert len(pairs)==81;a,b=zip(*pairs);ma=statistics.mean(a);mb=statistics.mean(b);va=math.fsum((x-ma)**2 for x in a);vb=math.fsum((y-mb)**2 for y in b);co=math.fsum((x-ma)*(y-mb) for x,y in pairs);v=co/math.sqrt(va*vb);assert math.isclose(v,p['correlation'],rel_tol=1e-10,abs_tol=1e-12);vals.append(v)
 value=statistics.mean(vals);assert math.isclose(value,e['mean_correlation'],rel_tol=1e-10,abs_tol=1e-12);return value

def footprint(centers,matrices,target):
 pixels=set()
 for m in matrices:
  for center in centers:
   for y in range(-4,5):
    for x in range(-4,5):
     p=[center[0]+x/2,center[1]+y/2]
     taps=[(fx/2,fy/2) for fy in range(-2,3) for fx in range(-2,3)] if target else [(0,0)]
     for dx,dy in taps:
      a,b=p[0]+dx,p[1]+dy
      if target:
       z=m[2][0]*a+m[2][1]*b+m[2][2];a,b=(m[0][0]*a+m[0][1]*b+m[0][2])/z,(m[1][0]*a+m[1][1]*b+m[1][2])/z
      ix,iy=math.floor(a),math.floor(b);pixels.update([(ix,iy),(ix+1,iy),(ix,iy+1),(ix+1,iy+1)])
 return pixels

def main():
 out=D/'dedup-refinement-replay-audit-20261011.json';assert not out.exists();parent=D/'dedup-aperture-replay-audit-20261011.json';p=json.loads(parent.read_text());assert all(sha(f)==h for f,h in p['pins'].items())
 ip=D/'dedup-exposure-replay-input-20261011.json';inp=json.loads(ip.read_text());op=D/'dedup-refinement-replay-output-20261011.json';obs=json.loads(op.read_text());assert obs['managed_used']==0
 baseline=json.loads((D/'dedup-aperture-replay-output-20261011.json').read_text());summaries=[]
 for row,case in zip(obs['evidence']['rows'],inp['cases']):
  assert row['case']==case['name'];centers=[p['source'] for p in case['witnesses']];mid=statistics.median(p[0] for p in centers);assert row['split_x']==mid
  fit=[p for p in centers if p[0]<=mid-8];check=[p for p in centers if p[0]>=mid+8];assert row['fit_centers']==fit and row['check_centers']==check;assert row['enough_groups']==(len(fit)>=3 and len(check)>=3)
  offsets=[[0.,0.]]+[[dx,dy]for dy in [-1.,-.5,0.,.5,1.]for dx in [-1.,-.5,0.,.5,1.]if dx!=0 or dy!=0];assert len(row['trials'])==25
  values=[]
  for trial,offset in zip(row['trials'],offsets):
   assert trial['offset']==offset
   zero=row['trials'][0]['matrix']
   for zr,ir in zip(zero,case['transform']):
    for a,b in zip(zr,ir):assert abs(a-b)<=2*max(math.ulp(a),math.ulp(b))
   m=[r[:]for r in zero]
   for r in m:r[2]+=r[0]*offset[0]+r[1]*offset[1]
   for actual,expected in zip(trial['matrix'],m):
    for a,b in zip(actual,expected):assert abs(a-b)<=2*max(math.ulp(a),math.ulp(b))
   assert [p['center']for p in trial['fit']['patches']]==fit;values.append(score(trial['fit']))
  selected=max(range(25),key=lambda i:values[i]);assert row['selected']==selected
  before=score(row['check_before']);after=score(row['check_after']);assert [p['center']for p in row['check_before']['patches']]==check and [p['center']for p in row['check_after']['patches']]==check
  old=next(r for r in baseline['evidence']['rows'] if r['case']==case['name'] and r['source_radius']==2 and r['target_aperture_source_radius']==1)
  for p in row['trials'][0]['fit']['patches']+row['check_before']['patches']:
   expected=next(v for v in old['patches'] if v['source_center']==p['center']);assert p['pairs']==expected['pairs'] and p['correlation']==expected['correlation']
  fs=footprint(fit,[case['transform']],False);cs=footprint(check,[case['transform']],False);assert fs.isdisjoint(cs)
  ft=footprint(fit,[t['matrix']for t in row['trials']],True);ct=footprint(check,[row['trials'][0]['matrix'],row['trials'][selected]['matrix']],True);assert ft.isdisjoint(ct)
  summaries.append({'case':case['name'],'fit_patches':len(fit),'check_patches':len(check),'enough_groups':row['enough_groups'],'offset':offsets[selected],'fit_before':values[0],'fit_after':values[selected],'check_before':before,'check_after':after,'shared_source_pixels':0,'shared_reference_pixels_all_fit_trials_vs_checks':0})
 h=T/'rrrah-refinement-replay-harness-20261011';assert sha(h/'src/main.rs')==sha(R/'scripts/dedup-refinement-replay/src/main.rs');log=D/'dedup-refinement-replay-build-20261011.log';assert 'Finished `dev` profile' in log.read_text()
 saved=T/'rrrah-refinement-replay-qualified-20261011';assert not saved.exists();saved.mkdir();binary=T/'rrrah-opposite-native-snapshot-20261011/target/debug/dedup-refinement-replay';shutil.copy2(binary,saved/binary.name)
 paths=[parent,ip,op,h/'src/main.rs',h/'Cargo.toml',h/'Cargo.lock',log,D/'dedup-refinement-replay-runtime-20261011.log',saved/binary.name,Path(__file__)]
 report={'status':'verified_spatial_fit_check_translation_experiment_no_adoption','pins':{str(p):sha(p) for p in paths},'summaries':summaries,'scope':'Fixed25 global source-coordinate translations, original geometry retained within2 ULP across JSON parsers; all shift equations independently checked within2 ULP. Only left group selects translation, right group evaluated afterward. Independent arithmetic replay and zero-trial exact baseline parity; projected bilinear pixel-index footprints confirm no source/reference pixel reuse between all fit trials and both check evaluations for these cases. This does not establish statistical independence or held-out policy qualification: image/candidates/recipe were used during development. Bank6 has only2 check patches, below declared3. Native samples are not independently decoded/recomputed by auditor. No automatic recovery, new acceptance gate or general precision/performance claim; diagnostic scratch external.'}
 out.write_text(json.dumps(report,indent=2)+'\n');print(report['status']);print(summaries)
if __name__=='__main__':main()
