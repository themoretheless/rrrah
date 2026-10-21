import pathlib,ctypes,json,hashlib,struct,sys,io
from PIL import Image
root=pathlib.Path('.dedup-work/media459-20261021');prior=json.load(open('docs/research/dedup-500/media459-controls-20261021/result-v3.json'));assert hashlib.sha256((root/'stb_image.h').read_bytes()).hexdigest()==prior['header_sha256']
lib=ctypes.CDLL(str(pathlib.Path('.dedup-work/media459-controls-20261021/stb-shared.dylib').resolve()));lib.stbi_image_free.argtypes=[ctypes.c_void_p]
for name in ['stbi_load_from_memory','stbi_load_16_from_memory']:
 f=getattr(lib,name);f.argtypes=[ctypes.c_void_p,ctypes.c_int,*([ctypes.POINTER(ctypes.c_int)]*3),ctypes.c_int];f.restype=ctypes.c_void_p
def load(b,bits,channels):
 f=lib.stbi_load_16_from_memory if bits==16 else lib.stbi_load_from_memory;buf=ctypes.create_string_buffer(b);x=ctypes.c_int();y=ctypes.c_int();c=ctypes.c_int();p=f(buf,len(b),ctypes.byref(x),ctypes.byref(y),ctypes.byref(c),channels);assert p
 count=x.value*y.value*channels;v=list((ctypes.c_uint16*count).from_address(p)) if bits==16 else list(ctypes.string_at(p,count));lib.stbi_image_free(p);return v
values=[1,256,0x1234,65535];pgm=b'P5\n4 1\n65535\n'+b''.join(struct.pack('>H',v) for v in values);decoded=load(pgm,16,1);expected=values
assert sys.byteorder=='little' and decoded==[256,1,0x3412,65535] and decoded!=expected
count=0
for w in [1,7,8,9,17]:
 for h in [1,2,3]:
  im=Image.new('1',(w,h));im.putdata([255 if (x+y)%3 else 0 for y in range(h) for x in range(w)]);b=io.BytesIO();im.save(b,format='BMP');assert load(b.getvalue(),8,3)==list(im.convert('RGB').tobytes());count+=1
r={'header_sha256':prior['header_sha256'],'method':'Actual originalstb2.30public8/16memoryload API viactypes, priorunchangedsharedlibrary. OwnedvalidsyntheticPGM andPillow1-bitBMP. No malformedinput, fake decoder, GUI, deletion or ASAN.','host_byteorder':sys.byteorder,'pgm16':{'input_hex':pgm.hex(),'expected_samples':expected,'actual_samples':decoded,'matches':False,'oracle':'NetpbmPGMspec: two-byte samples have most-significantbyte first; independently encoded withstruct.pack >H.','source':'https://netpbm.sourceforge.net/doc/pgm.html'},'bmp1bit':{'cases':count,'widths':[1,7,8,9,17],'heights':[1,2,3],'exact_rgb_pixels':True},'limits':['PGM16byteorder failure observed onlittleendianmacOSarm64; not universalallplatformclaim.','Application459scannerdoesnotincludePGMextension, so helperformat defect not claimed reachablethroughdefaultscanner.','No GIFdisposal/allocator/corruptinputqualification.']};pathlib.Path('docs/research/dedup-500/media459-controls-20261021/result-v5.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
