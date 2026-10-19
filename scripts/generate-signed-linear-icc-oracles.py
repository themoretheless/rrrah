#!/usr/bin/env python3
"""Independent LittleCMS double references for finite signed linear ICC RGB/gray."""
from pathlib import Path
import ctypes as c,ctypes.util,struct,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
lib=c.CDLL(ctypes.util.find_library('lcms2'))
def bind(n,r,*args):
 f=getattr(lib,n);f.restype=r;f.argtypes=list(args);return f
class xyY(c.Structure):_fields_=[('x',c.c_double),('y',c.c_double),('Y',c.c_double)]
class Primaries(c.Structure):_fields_=[('r',xyY),('g',xyY),('b',xyY)]
build=bind('cmsBuildGamma',c.c_void_p,c.c_void_p,c.c_double)
create=bind('cmsCreateRGBProfile',c.c_void_p,c.POINTER(xyY),c.POINTER(Primaries),c.POINTER(c.c_void_p))
openp=bind('cmsOpenProfileFromMem',c.c_void_p,c.c_void_p,c.c_uint32)
transform=bind('cmsCreateTransform',c.c_void_p,c.c_void_p,c.c_uint32,c.c_void_p,c.c_uint32,c.c_uint32,c.c_uint32)
do=bind('cmsDoTransform',None,c.c_void_p,c.c_void_p,c.c_void_p,c.c_uint32)
delete=bind('cmsDeleteTransform',None,c.c_void_p);close=bind('cmsCloseProfile',c.c_int,c.c_void_p);free=bind('cmsFreeToneCurve',None,c.c_void_p)
version=bind('cmsGetEncodedCMMversion',c.c_uint32)
gamma=build(None,1.);assert gamma
curves=(c.c_void_p*3)(gamma,gamma,gamma);white=xyY(.3127,.3290,1.);primaries=Primaries(xyY(.64,.33,1.),xyY(.30,.60,1.),xyY(.15,.06,1.));destination=create(c.byref(white),c.byref(primaries),curves);assert destination
rgbfmt=(1<<22)|(4<<16)|(3<<3);grayfmt=(1<<22)|(3<<16)|(1<<3)
def reference(profile,rgba,gray=False):
 incoming=[v for row in rgba for v in (row[:1] if gray else row[:3])]
 data=(root/profile).read_bytes();memory=c.create_string_buffer(data);source=openp(memory,len(data));assert source
 t=transform(source,grayfmt if gray else rgbfmt,destination,rgbfmt,1,0x140);assert t
 source_values=(c.c_double*len(incoming))(*incoming);output=(c.c_double*(len(rgba)*3))();do(t,source_values,output,len(rgba));delete(t);close(source)
 return struct.pack('<'+'d'*len(output),*output)
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
rgb=[[f32(v) for v in x] for x in [[-2,.25,16,.25],[1.00001,-.1,4,.5],[0,-.25,2,1],[16,1,-2,0]]]
gray=[[f32(v),f32(v),f32(v),i/8] for i,v in enumerate([-2,-.25,0,.25,1.00001,2,16])]
for name,rows in [('signed-linear-rgb.rgba32fle',rgb),('signed-linear-gray.rgba32fle',gray)]: (root/name).write_bytes(b''.join(struct.pack('<4f',*r) for r in rows))
profiles=[('linear-identity-rgb.icc',False),('psd-tools-4x4_32bit_rgb.psd.icc',False),('animation-linear-float.icc',False),('linear-identity-grayscale.icc',True),('psd-tools-4x4_32bit_grayscale.psd.icc',True)]
files=[]
for profile,is_gray in profiles:
 name=profile+'.signed-linear.rgb64le';(root/name).write_bytes(reference(profile,gray if is_gray else rgb,is_gray));files.extend([profile,name])
for index in range(3):
 data=(root/f'animation-linear-float.jxl.{index}.rgba32fle').read_bytes();rgba=[struct.unpack_from('<4f',data,o) for o in range(0,len(data),16)];name=f'animation-linear-float.jxl.{index}.linear-rgb64le';(root/name).write_bytes(reference('animation-linear-float.icc',rgba));files.append(name)
close(destination);free(gamma)
(root/'signed-linear-icc.json').write_text(json.dumps(dict(license='CC0',lcms_version=version(),scope='same ICC byte profiles, f64 source/destination, relative-colorimetric, no optimization/cache; linear sRGB D65 output; normalized alpha excluded from transform',sha256={name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in files}),indent=2)+'\n')
