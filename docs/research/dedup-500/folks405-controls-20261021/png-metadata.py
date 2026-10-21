from pathlib import Path
import ctypes as ct,struct,zlib,json
C=Path('.dedup-work/folks405-20261021');lib=ct.CDLL(str((C/'stbi-shared.dylib').resolve()));lib.stbi_load_from_memory.argtypes=[ct.c_void_p,ct.c_int,ct.POINTER(ct.c_int),ct.POINTER(ct.c_int),ct.POINTER(ct.c_int),ct.c_int];lib.stbi_load_from_memory.restype=ct.c_void_p;lib.stbi_image_free.argtypes=[ct.c_void_p]
def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
def png(gamma):return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,6,0,0,0))+chunk(b'gAMA',struct.pack('>I',gamma))+chunk(b'IDAT',zlib.compress(b'\x00\x80\x40\x20\x00'))+chunk(b'IEND',b'')
def decode(b,n):
 x=ct.c_int();y=ct.c_int();comp=ct.c_int();p=lib.stbi_load_from_memory(b,len(b),ct.byref(x),ct.byref(y),ct.byref(comp),n);assert p
 result=list(ct.string_at(p,x.value*y.value*n));lib.stbi_image_free(p);return result
first=png(45455);second=png(100000);bad=bytearray(first);bad[32]^=1
assert decode(first,4)==decode(second,4)==decode(bytes(bad),4)==[128,64,32,0]
assert decode(first,3)==[128,64,32]
r={'original_decoder_executed':True,'synthetic_valid_pngs':'1x1RGBA8, complete correct CRC chunks before controlled checksum mutation','gamma_45455_rgba':decode(first,4),'gamma_100000_rgba':decode(second,4),'bad_IHDR_crc_accepted':True,'forced_RGB_from_zero_alpha':decode(first,3),'limits':['Byte-level PNG decode observations, not calibrated color appearance or full PNG conformance.','Gamma metadata ignored by this decoder route; not evidence equal display luminance.']};Path('docs/research/dedup-500/folks405-controls-20261021/png-metadata-result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
