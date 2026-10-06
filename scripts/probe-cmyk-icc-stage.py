#!/usr/bin/env python3
"""External diagnostic: pinned ICC input curves/4D linear CLUT vs LCMS A2B0."""
import argparse, ctypes as c, hashlib, itertools, json, math, struct
from pathlib import Path
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--profile', type=Path, required=True)
p.add_argument('--library', type=Path, required=True)
p.add_argument('--operands', type=Path, required=True)
p.add_argument('--report', type=Path, required=True)
a = p.parse_args()
b = a.profile.read_bytes()
assert hashlib.sha256(b).hexdigest() == '73e1ba37d2bad5bab2a964f40a9eed96209666efc067c3322626214bbef234a0'
colors = json.loads(a.operands.read_text())['normalized_f32_cmyk']
# Exact pinned mft2 layout, with bounded extent checked before interpreting tables.
o = 240
assert b[o:o+4] == b'mft2' and b[o+8:o+11] == bytes([4,3,6])
assert struct.unpack_from('>2H', b, o+48) == (48,2)
curves = [struct.unpack_from('>48H', b, o+52+i*96) for i in range(4)]
clut = struct.unpack_from('>3888H', b, o+52+384)
assert o+52+384+7776+12 == len(b)
def curve(table, v):
    x = min(1,max(0,v))*47
    lo = min(46,math.floor(x)); t = x-lo
    return ((1-t)*table[lo]+t*table[lo+1])/65535

def linear4(v):
    x = [min(1,max(0,t))*5 for t in v]
    lo = [min(4,math.floor(t)) for t in x]
    frac = [t-i for t,i in zip(x,lo)]
    out = [0.0]*3
    for corner in itertools.product((0,1),repeat=4):
        weight = math.prod(f if bit else 1-f for bit,f in zip(corner,frac))
        indices = [i+bit for i,bit in zip(lo,corner)]
        base = (((indices[0]*6+indices[1])*6+indices[2])*6+indices[3])*3
        for j in range(3): out[j] += weight*clut[base+j]/65535
    return out
def hybrid4(v):
    # Linear interpolation along C; tetrahedral interpolation within M/Y/K.
    x = [min(1,max(0,t))*5 for t in v]
    lo = [min(4,math.floor(t)) for t in x]
    frac = [t-i for t,i in zip(x,lo)]
    order = sorted(range(1,4),key=lambda i:frac[i],reverse=True)
    weights = [1-frac[order[0]],frac[order[0]]-frac[order[1]],
               frac[order[1]]-frac[order[2]],frac[order[2]]]
    output = [0.0]*3
    for c_side in (0,1):
        indices = list(lo); indices[0] += c_side
        outer = frac[0] if c_side else 1-frac[0]
        for step,weight in enumerate(weights):
            base = (((indices[0]*6+indices[1])*6+indices[2])*6+indices[3])*3
            for j in range(3): output[j] += outer*weight*clut[base+j]/65535
            if step < 3: indices[order[step]] += 1
    return output

lcms = c.CDLL(str(a.library.resolve()))
def bind(name, result, *args):
    f = getattr(lcms,name); f.restype=result; f.argtypes=list(args); return f
open_profile=bind('cmsOpenProfileFromMem',c.c_void_p,c.c_void_p,c.c_uint32)
read_tag=bind('cmsReadTag',c.c_void_p,c.c_void_p,c.c_uint32)
evaluate=bind('cmsPipelineEvalFloat',None,c.POINTER(c.c_float),c.POINTER(c.c_float),c.c_void_p)
close=bind('cmsCloseProfile',c.c_int,c.c_void_p)
buffer=c.create_string_buffer(b); profile=open_profile(buffer,len(b)); assert profile
rows=[]
try:
    pipeline=read_tag(profile,int.from_bytes(b'A2B0','big')); assert pipeline
    for v in colors:
        output=(c.c_float*3)(); evaluate((c.c_float*4)(*v),output,pipeline)
        shaped=[curve(table,value) for table,value in zip(curves,v)]
        quad=linear4(shaped)
        hybrid=hybrid4(shaped)
        rows.append({'cmyk':v,'after_input_curves':shaped,'quadlinear_normalized_lab':quad, 'hybrid_normalized_lab':hybrid,
                     'lcms_a2b0_normalized_lab':list(output),
                     'max_normalized_difference':max(abs(x-y) for x,y in zip(quad,output))})
finally: close(profile)
a.report.write_text(json.dumps({'profile_sha256':hashlib.sha256(b).hexdigest(),
    'library_sha256':hashlib.sha256(a.library.read_bytes()).hexdigest(),
    'scope':'Raw A2B0 pipeline; no sRGB destination, physical Lab scaling or RGB quantization',
    'results':rows},indent=2)+'\n')
print('max normalized stage difference:',max(r['max_normalized_difference'] for r in rows))
