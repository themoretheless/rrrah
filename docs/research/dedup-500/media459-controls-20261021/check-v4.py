import pathlib,ctypes,struct,zlib,json,hashlib,subprocess
root=pathlib.Path('.dedup-work/media459-20261021').resolve();out=pathlib.Path('.dedup-work/media459-controls-20261021').resolve();header=root/'stb_image.h';prior=json.load(open('docs/research/dedup-500/media459-controls-20261021/result-v3.json'));assert prior['header_sha256']==hashlib.sha256(header.read_bytes()).hexdigest()
lib=ctypes.CDLL(str(out/'stb-shared.dylib'))
for name in ['stbi_load_from_memory','stbi_load_16_from_memory']:
 f=getattr(lib,name);f.argtypes=[ctypes.c_void_p,ctypes.c_int,*([ctypes.POINTER(ctypes.c_int)]*3),ctypes.c_int];f.restype=ctypes.c_void_p
lib.stbi_image_free.argtypes=[ctypes.c_void_p]
def paeth(a,b,c):
 p=a+b-c;d=[abs(p-a),abs(p-b),abs(p-c)];return [a,b,c][d.index(min(d))]
def chunk(name,b):return struct.pack('>I',len(b))+name+b+struct.pack('>I',zlib.crc32(name+b)&0xffffffff)
count=0
for depth in [1,2,4,8,16]:
 for w in [1,2,3,7,8,9,17]:
  for h in [1,2,3]:
   values=[(x*37+y*71+x*y*13)% (1<<depth) for y in range(h) for x in range(w)]
   rows=[]
   for y in range(h):
    row=values[y*w:(y+1)*w]
    if depth==16:packed=b''.join(struct.pack('>H',v) for v in row)
    elif depth==8:packed=bytes(row)
    else:
     bits=''.join(format(v,f'0{depth}b') for v in row);bits+='0'*(-len(bits)%8);packed=bytes(int(bits[i:i+8],2) for i in range(0,len(bits),8))
    rows.append(packed)
   for kind in range(5):
    encoded=b'';previous=bytes(len(rows[0]));bpp=2 if depth==16 else 1
    for row in rows:
     filtered=[]
     for i,value in enumerate(row):
      a=row[i-bpp] if i>=bpp else 0;b=previous[i];c=previous[i-bpp] if i>=bpp else 0
      predictor=[0,a,b,(a+b)//2,paeth(a,b,c)][kind];filtered.append((value-predictor)%256)
     encoded+=bytes([kind])+bytes(filtered);previous=row
    png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,depth,0,0,0,0))+chunk(b'IDAT',zlib.compress(encoded))+chunk(b'IEND',b'')
    buf=ctypes.create_string_buffer(png);x=ctypes.c_int();y=ctypes.c_int();c=ctypes.c_int();f=lib.stbi_load_16_from_memory if depth==16 else lib.stbi_load_from_memory;p=f(buf,len(png),ctypes.byref(x),ctypes.byref(y),ctypes.byref(c),1);assert p,(depth,w,h,kind)
    actual=list((ctypes.c_uint16*(w*h)).from_address(p)) if depth==16 else list(ctypes.string_at(p,w*h));lib.stbi_image_free(p);expected=values if depth==16 else [v*255//((1<<depth)-1) for v in values];assert actual==expected and (x.value,y.value)==(w,h),(depth,w,h,kind);count+=1
code='#define STB_IMAGE_IMPLEMENTATION\n#include "'+str(header)+'"\n#include <cstdio>\nint main(){unsigned n=0;for(int a=0;a<256;a++)for(int b=0;b<256;b++)for(int c=0;c<256;c++){int p=a+b-c;int da=abs(p-a),db=abs(p-b),dc=abs(p-c);int oracle=da<=db&&da<=dc?a:db<=dc?b:c;if(stbi__paeth(a,b,c)!=oracle)return 2;n++;}printf("%u\\n",n);}\n'
(out/'paeth.cpp').write_text(code);p=subprocess.run(['c++','-std=c++17','-O2',str(out/'paeth.cpp'),'-o',str(out/'paeth')],capture_output=True,text=True);assert p.returncode==0,p.stderr
p=subprocess.run([str(out/'paeth')],capture_output=True,text=True,check=True);triplets=int(p.stdout);assert triplets==256**3
r={'header_sha256':prior['header_sha256'],'method':'Original complete header unchanged, actual public8/16bitload APIs viactypes. Owned valid grayscalePNG bytes independently encoded with allfive PNGfilters. OriginalstaticPaeth from fullheader compiled unchanged and compared against abs-distance/tie-order oracle for every8bittriple. No fake decoder, ASAN or malformed input.','png_cases':count,'depths':[1,2,4,8,16],'widths':[1,2,3,7,8,9,17],'heights':[1,2,3],'filters':[0,1,2,3,4],'exact_pixels_and_dimensions':True,'paeth_triplets':triplets,'paeth_mismatches':0,'limits':'No color/palette/Adam7/transparency/CRC/metadata/cancellation/allocator qualification and no benchmark or productionchange.'};pathlib.Path('docs/research/dedup-500/media459-controls-20261021/result-v4.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
