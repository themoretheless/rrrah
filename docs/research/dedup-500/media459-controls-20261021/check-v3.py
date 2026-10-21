import pathlib,subprocess,ctypes,json,hashlib,io,struct
from PIL import Image,features
root=pathlib.Path('.dedup-work/media459-20261021').resolve();out=pathlib.Path('.dedup-work/media459-controls-20261021').resolve()
(out/'stb-shared.cpp').write_text('#define STB_IMAGE_IMPLEMENTATION\n#include "'+str(root/'stb_image.h')+'"\n')
p=subprocess.run(['c++','-std=c++17','-O1','-dynamiclib',str(out/'stb-shared.cpp'),'-o',str(out/'stb-shared.dylib')],capture_output=True,text=True);assert p.returncode==0,p.stderr
lib=ctypes.CDLL(str(out/'stb-shared.dylib'));lib.stbi_load_from_memory.argtypes=[ctypes.c_void_p,ctypes.c_int,*([ctypes.POINTER(ctypes.c_int)]*3),ctypes.c_int];lib.stbi_load_from_memory.restype=ctypes.c_void_p;lib.stbi_image_free.argtypes=[ctypes.c_void_p];lib.stbi_failure_reason.restype=ctypes.c_char_p
cases=[];im=Image.new('RGB',(2,2));im.putdata([(10,20,30),(40,50,60),(70,80,90),(100,110,120)])
formats=['PNG','BMP','TIFF']+(['WEBP'] if features.check('webp') else [])
for fmt in formats:
 stream=io.BytesIO();im.save(stream,format=fmt);b=stream.getvalue();buf=ctypes.create_string_buffer(b);x=ctypes.c_int();y=ctypes.c_int();c=ctypes.c_int();result=lib.stbi_load_from_memory(buf,len(b),ctypes.byref(x),ctypes.byref(y),ctypes.byref(c),3);ok=bool(result)
 row={'format':fmt,'input_bytes':len(b),'input_sha256':hashlib.sha256(b).hexdigest(),'decoded':ok}
 if result:row.update(width=x.value,height=y.value,channels_in_file=c.value,pixels_hex=ctypes.string_at(result,x.value*y.value*3).hex());lib.stbi_image_free(result)
 else:row['reason']=lib.stbi_failure_reason().decode()
 cases.append(row)
assert all(r['decoded']==(r['format'] in ['PNG','BMP']) for r in cases);assert any(r['format']=='WEBP' for r in cases)
r={'header_sha256':hashlib.sha256((root/'stb_image.h').read_bytes()).hexdigest(),'method':'Original complete stb2.30 header compiled unchanged as sharedlibrary; actualstbi_load_from_memory publicAPI called viactypes with ownedvalid2x2Pillowencoded syntheticimages. No fake decoder or malformedinput, no ASAN.','cases':cases,'finding':'PNG/BMPdecode, validTIFF/WebP rejected as unknown image type despite scannerallowlist/READMEadvertisement.','limits':'Headerreview incomplete, publicAPI format supportcontrols only; no allformatqualification, windowsbehavior, speed or productionchange.'};pathlib.Path('docs/research/dedup-500/media459-controls-20261021/result-v3.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
