"""Independent Gaussian RGB/XYZ/Bradford oracle; no Rrrah dependencies.
Writes analytical RGBA32F RGBW(2.0) and records double vs f32 cone precision.
"""
import argparse, json, struct
from pathlib import Path

def multiply(a,b):
    return [[sum(x*y for x,y in zip(row,col)) for col in zip(*b)] for row in a]
def inverse(a):
    m=[list(row)+[float(i==j) for j in range(3)] for i,row in enumerate(a)]
    for i in range(3):
        pivot=max(range(i,3),key=lambda j:abs(m[j][i]));m[i],m[pivot]=m[pivot],m[i]
        d=m[i][i]
        if abs(d)<1e-15: raise ValueError('singular basis')
        m[i]=[v/d for v in m[i]]
        for j in range(3):
            if j!=i:
                d=m[j][i];m[j]=[v-d*w for v,w in zip(m[j],m[i])]
    return [row[3:] for row in m]
def xyz_basis(xy):
    basis=[[xy[2*c] for c in range(3)],[xy[2*c+1] for c in range(3)],
           [1-xy[2*c]-xy[2*c+1] for c in range(3)]]
    white=[[xy[6]/xy[7]],[1],[(1-xy[6]-xy[7])/xy[7]]]
    scale=multiply(inverse(basis),white)
    return [[row[c]*scale[c][0] for c in range(3)] for row in basis],white

def matrix(xy, rounded_cones, adopted_neutral=None):
    source,white=xyz_basis(xy)
    if adopted_neutral is not None:
        x,y=adopted_neutral
        white=[[x/y],[1],[(1-x-y)/y]]
    target,d65=xyz_basis([.64,.33,.30,.60,.15,.06,.3127,.3290])
    cone=[[.8951,.2664,-.1614],[-.7502,1.7135,.0367],[.0389,-.0685,1.0296]]
    if rounded_cones:
        cone=[[struct.unpack('<f',struct.pack('<f',v))[0] for v in row] for row in cone]
    a=multiply(cone,white);b=multiply(cone,d65)
    diagonal=[[b[i][0]/a[i][0] if i==j else 0 for j in range(3)] for i in range(3)]
    adaptation=multiply(multiply(inverse(cone),diagonal),cone)
    return multiply(multiply(inverse(target),adaptation),source)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--xy',type=float,nargs=8,required=True)
    p.add_argument('--neutral',type=float,nargs=2)
    p.add_argument('--report',type=Path,required=True)
    p.add_argument('--rgba32',type=Path,required=True)
    a=p.parse_args();reference=matrix(a.xy,False,a.neutral);portable=matrix(a.xy,True,a.neutral)
    error=max(abs(x-y) for r,s in zip(reference,portable) for x,y in zip(r,s))
    assert error<1e-6
    values=[v for c in range(3) for v in [*(2*portable[r][c] for r in range(3)),1.0]]+[*(2*sum(row) for row in portable),1.0]
    a.rgba32.write_bytes(struct.pack('<16f',*values))
    a.report.write_text(json.dumps({'xy':a.xy,'adopted_neutral':a.neutral,'matrix_double_Bradford':reference,
        'matrix_f32_Bradford_cones':portable,'max_matrix_precision_delta':error,
        'method':'Independent partial-pivot Gaussian inversion; no production code/matrices imported. f32 cone variant supplies strict portable pixel reference; double-cone variant bounds precision loss.',
        'rgba32':str(a.rgba32)},indent=2)+'\n')
