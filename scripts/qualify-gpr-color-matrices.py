#!/usr/bin/env python3
"""Independent double-precision matrix diagnostic; not a rendered-color oracle."""
import argparse,json,pathlib,struct
p=argparse.ArgumentParser();p.add_argument('--source',type=pathlib.Path,required=True);p.add_argument('--native',type=pathlib.Path,required=True);p.add_argument('--libraw',type=pathlib.Path,required=True);p.add_argument('--report',type=pathlib.Path,required=True);a=p.parse_args()
def mul(a,b):return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def inv(a):
 rows=[r[:]+[float(i==j) for j in range(3)] for i,r in enumerate(a)]
 for i in range(3):
  pivot=max(range(i,3),key=lambda j:abs(rows[j][i]));rows[i],rows[pivot]=rows[pivot],rows[i]
  scale=rows[i][i];assert abs(scale)>1e-12;rows[i]=[v/scale for v in rows[i]]
  for j in range(3):
   if j!=i:
    scale=rows[j][i];rows[j]=[x-scale*y for x,y in zip(rows[j],rows[i])]
 return [r[3:] for r in rows]
def rgb(cm):
 matrix=mul(cm,[[.4124564,.3575761,.1804375],[.2126729,.7151522,.0721750],[.0193339,.1191920,.9503041]])
 return inv([[v/sum(r) for v in r] for r in matrix])
def error(a,b):return max(abs(x-y) for r,s in zip(a,b) for x,y in zip(r,s))
b=a.source.read_bytes();assert b[:2]==b'II';off=struct.unpack_from('<I',b,4)[0];tags={}
for i in range(struct.unpack_from('<H',b,off)[0]):
 tag,t,n,v=struct.unpack_from('<HHII',b,off+2+i*12);tags[tag]=(t,n,v)
t,n,v=tags[50722];assert t==10 and n==9
flat=[x/y for x,y in (struct.unpack_from('<ii',b,v+8*i) for i in range(9))];cm=[flat[i:i+3] for i in (0,3,6)];assert tags[50779]==(3,1,23)
bradford=[[.8951,.2664,-.1614],[-.7502,1.7135,.0367],[.0389,-.0685,1.0296]]
def vec(m,v):return [sum(x*y for x,y in zip(r,v)) for r in m]
d65=[.95047,1.,1.08883];d50=[.34567/.35850,1.,(1-.34567-.35850)/.35850]
from_lms=vec(bradford,d65);to_lms=vec(bradford,d50)
diag=[[to_lms[i]/from_lms[i] if i==j else 0. for j in range(3)] for i in range(3)]
adapted=mul(cm,mul(inv(bradford),mul(diag,bradford)))
n=json.loads(a.native.read_text());o=json.loads(a.libraw.read_text())
report={'scope':__doc__,'unadapted_cm2_rgb':rgb(cm),'bradford_d65_to_d50_cm2_rgb':rgb(adapted),'native_adapted_matrix_error':error(adapted,n['xyz_to_camera'][:3]),'native_adapted_rgb_error':error(rgb(adapted),n['camera_to_rgb']),'libraw_unadapted_rgb_error':error(rgb(cm),o['camera_to_rgb']),'native_vs_libraw_rgb_error':error(n['camera_to_rgb'],o['camera_to_rgb'])}
assert report['native_adapted_matrix_error']<1e-6
assert report['native_adapted_rgb_error']<1e-6
print(json.dumps(report,indent=2));a.report.write_text(json.dumps(report,indent=2)+'\n')
