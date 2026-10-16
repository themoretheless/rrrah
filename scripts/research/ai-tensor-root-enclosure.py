"""Test-only exact rational type-7 root/color certificate for the pinned fixture.
Certifies source root existence/uniqueness; excludes clipping and production output.
"""
from fractions import Fraction as F
from pathlib import Path
import hashlib, json, struct, sys
from pypdf import PdfReader
src, dest = map(Path, sys.argv[1:3])
assert not dest.exists()
assert hashlib.sha256(src.read_bytes()).hexdigest() == 'c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06'
sh = PdfReader(src).pages[0]['/Resources']['/Shading']['/Sh8'].get_object()
assert [int(sh[k]) for k in ['/ShadingType','/BitsPerFlag','/BitsPerCoordinate','/BitsPerComponent']] == [7,8,32,8]
assert '/Function' not in sh
raw = sh.get_data(); decode = [F(str(v)) for v in sh['/Decode']]
assert len(decode) == 12
patches=[]; at=0
while at<len(raw):
    flag=raw[at];at+=1;assert flag in range(4)
    if flag:
        prev, col=patches[-1];points=[prev[(3*flag+i)%12] for i in range(4)];colors=[col[flag],col[(flag+1)%4]]
    else:points=[];colors=[]
    while len(points)<16:
        assert at+8<=len(raw)
        xy=struct.unpack_from('>II',raw,at);at+=8
        points.append(tuple(decode[2*a]+F(v,4294967295)*(decode[2*a+1]-decode[2*a]) for a,v in enumerate(xy)))
    while len(colors)<4:
        assert at+4<=len(raw)
        colors.append(tuple(decode[4+2*a]+F(raw[at+a],255)*(decode[5+2*a]-decode[4+2*a]) for a in range(4)));at+=4
    patches.append((points,colors))
assert at==len(raw) and len(patches)==245

def split_curve(p,t=F(1,2)):
    left=[p[0]];right=[p[-1]]
    while len(p)>1:
        p=[tuple((1-t)*a+t*b for a,b in zip(x,y)) for x,y in zip(p,p[1:])]
        left.append(p[0]);right.append(p[-1])
    return left,list(reversed(right))

def split_grid(g):
    pairs=[split_curve(row) for row in g];left=[p[0] for p in pairs];right=[p[1] for p in pairs]
    out=[]
    for half in [left,right]:
        cols=list(zip(*half));vertical=[split_curve(list(col)) for col in cols]
        out.extend([list(map(list,zip(*(pair[k] for pair in vertical)))) for k in [0,1]])
    return out  # low-u/low-v, low-u/high-v, high-u/low-v, high-u/high-v

indices=[[0,11,10,9],[1,12,15,8],[2,13,14,7],[3,4,5,6]]
target=(F(295,2),F(-186));max_depth=20;nodes=0;excluded=0;leaves=[]
for index,(points,colors) in enumerate(patches):
    grid=[[points[i] for i in row] for row in indices]
    stack=[(grid,F(0),F(1),F(0),F(1),0)]
    while stack:
        g,u0,u1,v0,v1,depth=stack.pop();nodes+=1
        assert nodes<=100000,'diagnostic node allowance exhausted'
        # Bicubic tensor values are convex combinations of these controls.
        if any(target[a]<min(p[a] for row in g for p in row) or target[a]>max(p[a] for row in g for p in row) for a in range(2)):
            excluded+=1;continue
        if depth==max_depth:
            # Bilinear color extrema on a rectangle occur at its corners.
            values=[]
            for u in [u0,u1]:
                for v in [v0,v1]:
                    w=[(1-u)*(1-v),(1-u)*v,u*v,u*(1-v)]
                    values.append([sum(wi*c[a] for wi,c in zip(w,colors)) for a in range(4)])
            bounds=[[min(c[a] for c in values),max(c[a] for c in values)] for a in range(4)]
            leaves.append({'patch':index,'uv':[[u0,u1],[v0,v1]],'color':bounds});continue
        um=(u0+u1)/2;vm=(v0+v1)/2
        boxes=[(u0,um,v0,vm),(u0,um,vm,v1),(um,u1,v0,vm),(um,u1,vm,v1)]
        for child,box in zip(split_grid(g),boxes):stack.append((child,*box,depth+1))
assert leaves,'no possible source root cells'
union=[[min(r['color'][a][0] for r in leaves),max(r['color'][a][1] for r in leaves)] for a in range(4)]
def rational(x):return {'n':str(x.numerator),'d':str(x.denominator)}
# Prove a unique root exists in a rectangle enclosing every remaining cell.
assert set(r['patch'] for r in leaves)=={1}
h=F(1,2**max_depth)
u0=min(r['uv'][0][0] for r in leaves)-2*h;u1=max(r['uv'][0][1] for r in leaves)+2*h
v0=min(r['uv'][1][0] for r in leaves)-2*h;v1=max(r['uv'][1][1] for r in leaves)+2*h
assert 0<u0<u1<1 and 0<v0<v1<1
def restrict_curve(p,l,r):
    return split_curve(split_curve(p,r)[0],l/r)[1]
def evaluate(p,t):
    while len(p)>1:p=[tuple((1-t)*a+t*b for a,b in zip(x,y)) for x,y in zip(p,p[1:])]
    return p[0]
original=[[patches[1][0][i] for i in row] for row in indices]
g=[restrict_curve(row,u0,u1) for row in original]
g=list(map(list,zip(*(restrict_curve(list(col),v0,v1) for col in zip(*g)))))
for u,v in [(F(0),F(0)),(F(1,2),F(1,2)),(F(1),F(1))]:
    assert evaluate([evaluate(row,u) for row in g],v)==evaluate([evaluate(row,u0+(u1-u0)*u) for row in original],v0+(v1-v0)*v)
du=evaluate([evaluate([tuple(3*(b-a) for a,b in zip(p,q)) for p,q in zip(row,row[1:])],F(1,2)) for row in g],F(1,2))
rows=[evaluate(row,F(1,2)) for row in g]
dv=evaluate([tuple(3*(b-a) for a,b in zip(p,q)) for p,q in zip(rows,rows[1:])],F(1,2))
det=du[0]*dv[1]-dv[0]*du[1];assert det
matrix=[[dv[1]/det,-dv[0]/det],[-du[1]/det,du[0]/det]]
hg=[[tuple(sum(matrix[a][b]*(p[b]-target[b]) for b in range(2)) for a in range(2)) for p in row] for row in g]
faces=[max(row[0][0] for row in hg),min(row[-1][0] for row in hg),max(p[1] for p in hg[0]),min(p[1] for p in hg[-1])]
assert faces[0]<0<faces[1] and faces[2]<0<faces[3], 'Miranda boundary sign proof failed'
norms=[]
for a in range(2):
    uder=[3*(row[i+1][a]-row[i][a]) for row in hg for i in range(3)]
    vder=[3*(hg[j+1][i][a]-hg[j][i][a]) for j in range(3) for i in range(4)]
    norms.append(sum(max(abs(x-F(int(a==axis))) for x in values) for axis,values in enumerate([uder,vder])))
assert max(norms)<1, 'Injectivity derivative bound failed'
certificate={'patch':1,'box_exact':[[rational(u0),rational(u1)],[rational(v0),rational(v1)]],'existence':'Poincare-Miranda: exact strict opposite signs of preconditioned F components on corresponding parameter faces.','face_extrema_exact':[rational(x) for x in faces],'uniqueness':'Exact Bernstein derivative hulls bound ||I-DH||_infinity below1; H is injective on convex box. All other parameter cells/patches excluded by exact hulls.','derivative_norm_upper_exact':rational(max(norms)),'derivative_norm_upper_float_display':float(max(norms))}
result={'source_sha256':hashlib.sha256(src.read_bytes()).hexdigest(),'stream_sha256':hashlib.sha256(raw).hexdigest(),'target_source':['295/2','-186'],'patches':len(patches),'depth':max_depth,'visited_nodes':nodes,'excluded_nodes':excluded,'possible_root_cells':len(leaves),'possible_patches':sorted(set(r['patch'] for r in leaves)),'cells':[{'patch':r['patch'],'uv_exact':[[rational(x) for x in b] for b in r['uv']]} for r in leaves],'cmyk_bounds_exact':[[rational(x) for x in b] for b in union],'cmyk_bounds_float_display':[[float(x) for x in b] for b in union],'root_certificate':certificate,'scope':'Exact rational decoding/hull exclusion plus Poincare-Miranda existence and derivative-norm injectivity prove exactly one source root across all245 parameter squares. Bounds enclose its bilinear CMYK. No clipping, triangle ownership, ICC conversion or production certification. Exact fractions are authoritative.'}
dest.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['cells','cmyk_bounds_exact']},indent=2))
