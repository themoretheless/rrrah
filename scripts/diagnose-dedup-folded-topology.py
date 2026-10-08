"""Geometry-only diagonal alternatives; keeps every correspondence and face count."""
import json
from pathlib import Path
root=Path(__file__).resolve().parents[1]
base=root/'docs/research'
p=json.loads((base/'dedup-folded-qualified-landmarks.json').read_text())['rows']
faces=json.loads((base/'dedup-folded-triangulation.json').read_text())['triangles']
def area(f,key):
 a,b,c=[p[i][key] for i in f]
 return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
records=[]
for i,f in enumerate(faces):
 if area(f,'source')>0:continue
 for j,g in enumerate(faces):
  shared=set(f)&set(g)
  if len(shared)!=2:continue
  a,b=sorted(shared);c=next(v for v in f if v not in shared);d=next(v for v in g if v not in shared)
  proposed=[[c,d,a],[d,c,b]]
  for h in proposed:
   if area(h,'target')<0:h[0],h[1]=h[1],h[0]
  # A valid target flip covers precisely the same quadrilateral, with both
  # diagonals internal; all four areas strictly positive.
  ta=[area(h,'target') for h in proposed];sa=[area(h,'source') for h in proposed]
  valid=min(ta+sa)>1e-6 and abs(sum(ta)-area(f,'target')-area(g,'target'))<1e-8
  row={'original_face_indices':[i,j],'original_faces':[f,g],'alternative_faces':proposed,'target_twice_areas':ta,'source_twice_areas':sa,'local_orientation_and_target_area_valid':valid}
  if valid:
   changed=[h for h in faces];changed[i],changed[j]=proposed
   path=base/f'dedup-folded-topology-alternative-{i}-{j}.txt'
   path.write_text(''.join(' '.join(map(str,h))+'\n' for h in changed));row['faces_file']=str(path.relative_to(root))
  records.append(row)
(base/'dedup-folded-topology-alternatives.json').write_text(json.dumps({'scope':'local diagonal alternatives only; whole native source validation pending','alternatives':records},indent=2)+'\n')
print(json.dumps(records,indent=2))
