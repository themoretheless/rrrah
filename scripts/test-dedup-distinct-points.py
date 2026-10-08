"""Spatial-bucket checks against an independent all-pairs oracle."""
import math,random,json,hashlib
from pathlib import Path
from dedup_distinct_points import verify_distinct_points
rng=random.Random(314159);cases=[]
for count in [0,1,20,200]:
 for magnitude in [1.,100.,1e6,4294967295.]:cases.append([[[rng.random()*magnitude,rng.random()*magnitude],[rng.random()*magnitude,rng.random()*magnitude]] for _ in range(count)])
for x in [0.,2.,4.,1e6,4294967293.]:
 for delta in [math.nextafter(2.,0.),2.,math.nextafter(2.,math.inf),3.]:
  cases.append([[[x,0.],[0.,0.]],[[x+delta,0.],[10.,10.]]])
cases.append([[[4.,0.],[0.,0.]],[[math.nextafter(2.,0.),0.],[10.,10.]]]);cases.append([[[0.,0.],[1.,1.]],[[10.,10.],[1.,1.]]])
results=[]
for points in cases:
 expected=all(all(math.hypot(a[axis][0]-b[axis][0],a[axis][1]-b[axis][1])>2 for axis in [0,1]) for i,a in enumerate(points) for b in points[:i])
 try:verify_distinct_points(points);actual=True
 except AssertionError:actual=False
 assert actual==expected;results.append(actual)
paths=[Path(__file__),Path(__file__).with_name('dedup_distinct_points.py')];out=Path('docs/research/dedup-distinct-point-auditor-controls.json');assert not out.exists();out.write_text(json.dumps({'status':'passed_independent_exhaustive_equivalence','cases':len(cases),'accepted':sum(results),'rejected':len(results)-sum(results),'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},'scope':'Finite seeded/boundary oracle equivalence; geometric/pixel checks remain separate.'},indent=2)+'\n');print(len(cases))
