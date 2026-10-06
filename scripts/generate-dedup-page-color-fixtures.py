"""CC0 authored pages; independent lcms ICC construction and TIFF/PNG encoding."""
from pathlib import Path
import ctypes as C, ctypes.util, hashlib, json, io
import numpy as np, tifffile
from PIL import Image, ImageCms, __version__
root=Path(__file__).resolve().parent.parent/'crates/rrrah-dedup/tests/fixtures/page-color';root.mkdir(parents=True,exist_ok=True)
lib=C.CDLL(ctypes.util.find_library('lcms2'))
class xyY(C.Structure):_fields_=[('x',C.c_double),('y',C.c_double),('Y',C.c_double)]
class Primaries(C.Structure):_fields_=[('r',xyY),('g',xyY),('b',xyY)]
lib.cmsBuildGamma.argtypes=[C.c_void_p,C.c_double];lib.cmsBuildGamma.restype=C.c_void_p
lib.cmsCreateRGBProfile.argtypes=[C.POINTER(xyY),C.POINTER(Primaries),C.POINTER(C.c_void_p)];lib.cmsCreateRGBProfile.restype=C.c_void_p
lib.cmsSaveProfileToMem.argtypes=[C.c_void_p,C.c_void_p,C.POINTER(C.c_uint32)];lib.cmsSaveProfileToMem.restype=C.c_int
lib.cmsFreeToneCurve.argtypes=[C.c_void_p];lib.cmsCloseProfile.argtypes=[C.c_void_p]
gamma=lib.cmsBuildGamma(None,1.);assert gamma
white=xyY(.3127,.3290,1.);primaries=Primaries(xyY(.64,.33,1.),xyY(.30,.60,1.),xyY(.15,.06,1.))
curves=(C.c_void_p*3)(gamma,gamma,gamma);profile=lib.cmsCreateRGBProfile(C.byref(white),C.byref(primaries),curves);assert profile
size=C.c_uint32();assert lib.cmsSaveProfileToMem(profile,None,C.byref(size));buffer=C.create_string_buffer(size.value);assert lib.cmsSaveProfileToMem(profile,buffer,C.byref(size))
linear=buffer.raw;lib.cmsCloseProfile(profile);lib.cmsFreeToneCurve(gamma)
srgb=ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
pixels=np.fromfunction(lambda y,x,c: (y*31+x*17+c*53+30)%220,(4,5,3),dtype=int).astype(np.uint8)
for name,icc in [('srgb',srgb),('linear',linear)]:
    Image.fromarray(pixels).save(root/f'{name}.png',icc_profile=icc)
for name,profiles in [('mixed',[srgb,linear]),('all-srgb',[srgb,srgb])]:
    with tifffile.TiffWriter(root/f'{name}.tif') as writer:
        for icc in profiles:writer.write(pixels,photometric='rgb',metadata=None,iccprofile=icc,compression='deflate')
    with tifffile.TiffFile(root/f'{name}.tif') as source:
        assert len(source.pages)==2
        for page,icc in zip(source.pages,profiles):
            assert np.array_equal(page.asarray(),pixels)
            assert page.tags[34675].value==icc
# Independent lcms/Pillow color conversion confirms identical samples have distinct colors.
a=ImageCms.profileToProfile(Image.fromarray(pixels),ImageCms.ImageCmsProfile(io.BytesIO(srgb)),ImageCms.createProfile('sRGB'))
b=ImageCms.profileToProfile(Image.fromarray(pixels),ImageCms.ImageCmsProfile(io.BytesIO(linear)),ImageCms.createProfile('sRGB'))
assert a.tobytes()!=b.tobytes()
# Independent float color oracle: lcms relative-colorimetric RGB float -> linear sRGB.
lib.cmsOpenProfileFromMem.argtypes=[C.c_void_p,C.c_uint32];lib.cmsOpenProfileFromMem.restype=C.c_void_p
lib.cmsCreateTransform.argtypes=[C.c_void_p,C.c_uint32,C.c_void_p,C.c_uint32,C.c_uint32,C.c_uint32];lib.cmsCreateTransform.restype=C.c_void_p
lib.cmsDoTransform.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_uint32];lib.cmsDeleteTransform.argtypes=[C.c_void_p]
lib.cmsGetEncodedCMMversion.restype=C.c_uint32
rgb_float=(1<<22)|(4<<16)|(3<<3)|4
for name,icc in [('srgb',srgb),('linear',linear)]:
    source_memory=C.create_string_buffer(icc);target_memory=C.create_string_buffer(linear)
    source=lib.cmsOpenProfileFromMem(source_memory,len(icc));target=lib.cmsOpenProfileFromMem(target_memory,len(linear));assert source and target
    transform=lib.cmsCreateTransform(source,rgb_float,target,rgb_float,1,0x140);assert transform
    incoming=(pixels.astype(np.float32)/255).copy();out=np.empty_like(incoming)
    lib.cmsDoTransform(transform,incoming.ctypes.data,out.ctypes.data,pixels.shape[0]*pixels.shape[1])
    assert np.isfinite(out).all()
    rgba=np.concatenate([out,np.ones((*out.shape[:2],1),dtype=np.float32)],axis=2)
    (root/f'{name}.rgba32le').write_bytes(rgba.astype('<f4').tobytes())
    lib.cmsDeleteTransform(transform);lib.cmsCloseProfile(source);lib.cmsCloseProfile(target)
(root/'manifest.json').write_text(json.dumps({'license':'CC0-1.0','pillow':__version__,'lcms_encoded_version':lib.cmsGetEncodedCMMversion(),'float_oracle':'relative colorimetric, float RGB to linear sRGB, cache/optimizations disabled','tifffile':tifffile.__version__,'dimensions':[5,4],'pages':2,'identical_encoded_samples_distinct_declared_colors':True,'sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.iterdir()) if p.suffix!='.json'}},indent=2)+'\n')
print('Four independent profile/page fixtures verified')
