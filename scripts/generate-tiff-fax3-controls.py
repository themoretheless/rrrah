"""Independent LibTIFF Group3/4 encodings; Pillow appearance oracles where supported."""
from pathlib import Path
import ctypes as C,ctypes.util,json,hashlib,sys,tempfile
from PIL import Image,__version__
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
lib=C.CDLL(ctypes.util.find_library('tiff'))
lib.TIFFOpen.argtypes=[C.c_char_p,C.c_char_p];lib.TIFFOpen.restype=C.c_void_p
lib.TIFFSetField.argtypes=[C.c_void_p,C.c_uint32];lib.TIFFSetField.restype=C.c_int
lib.TIFFClose.argtypes=[C.c_void_p]
lib.TIFFWriteScanline.argtypes=[C.c_void_p,C.c_void_p,C.c_uint32,C.c_uint16];lib.TIFFWriteScanline.restype=C.c_int
lib.TIFFReadScanline.argtypes=[C.c_void_p,C.c_void_p,C.c_uint32,C.c_uint16];lib.TIFFReadScanline.restype=C.c_int
lib.TIFFRawStripSize.argtypes=[C.c_void_p,C.c_uint32];lib.TIFFRawStripSize.restype=C.c_ssize_t
lib.TIFFReadRawStrip.argtypes=[C.c_void_p,C.c_uint32,C.c_void_p,C.c_ssize_t];lib.TIFFReadRawStrip.restype=C.c_ssize_t
lib.TIFFReadEncodedStrip.argtypes=[C.c_void_p,C.c_uint32,C.c_void_p,C.c_ssize_t];lib.TIFFReadEncodedStrip.restype=C.c_ssize_t
lib.TIFFWriteRawStrip.argtypes=[C.c_void_p,C.c_uint32,C.c_void_p,C.c_ssize_t];lib.TIFFWriteRawStrip.restype=C.c_ssize_t
long_runs='--long-runs' in sys.argv
group4='--group4' in sys.argv
word_aligned='--group2w' in sys.argv
single_row_strips='--single-row-strips' in sys.argv
assert not single_row_strips or word_aligned
group2='--group2' in sys.argv or word_aligned
assert not (group2 and group4)
dense='--dense' in sys.argv
assert not (dense and long_runs)
compression=(32771 if word_aligned else 2) if group2 else (4 if group4 else 3)
family=('fax2w' if word_aligned else 'fax2') if group2 else ('fax4' if group4 else 'fax3')
rows=[];w,h=(1025,129) if dense else ((2049,11) if long_runs else (13,7))
prefix=family+('-single' if single_row_strips else '')+('-dense' if dense else ('-long' if long_runs else ''))
rows_per_strip=1 if single_row_strips else (h if dense else (5 if long_runs else 3))
def black_at(x,y):
 if dense:return (x+y)%2==0
 if not long_runs:return (x*3+y*5+x*y)%7<3
 return [False,True,x<w//2,x%513==0,x>w-4][y%5]

for mode in (['wl'] if dense else ['wl','wb','w8l','w8b']):
 for fill in ([1] if dense else [1,2]):
  for options in ([0] if group2 or group4 else [0,1,4,5]):
   for orientation in ([1] if dense or long_runs or single_row_strips else range(1,9)):
    path=root/f'{prefix}-{mode}-fill{fill}-opt{options}-ori{orientation}.tiff'
    tif=lib.TIFFOpen(str(path).encode(),mode.encode());assert tif
    try:
     tags=[(256,w),(257,h),(258,1),(259,compression),(262,0),(266,fill),(277,1),(278,rows_per_strip),(284,1),(274,orientation)]
     if not group2:tags.append((293 if group4 else 292,options))
     for tag,value in tags:
      assert lib.TIFFSetField(tif,tag,C.c_int(value))==1
     strips={}
     for y in range(h):
      data=bytearray((w+7)//8)
      for x in range(w):
       if black_at(x,y):data[x//8]|=1<<(7-x%8)
      buf=C.create_string_buffer(bytes(data))
      if not word_aligned:
       assert lib.TIFFWriteScanline(tif,buf,y,0)==1
      else:
       # The LibTIFF RLEW encode/decode roundtrip disagrees on multi-row
       # strips. Encode each row with RLE and roundtrip it independently,
       # then assemble explicitly word-padded raw strips. Full-strip
       # disagreement is recorded below, never counted as oracle success.
       with tempfile.TemporaryDirectory() as temporary:
        single=str(Path(temporary)/'row.tiff').encode()
        row_tif=lib.TIFFOpen(single,b'wl');assert row_tif
        try:
         for tag,value in [(256,w),(257,1),(258,1),(259,2),(262,0),(266,fill),(277,1),(278,1)]:
          assert lib.TIFFSetField(row_tif,tag,C.c_int(value))==1
         assert lib.TIFFWriteScanline(row_tif,buf,0,0)==1
        finally:lib.TIFFClose(row_tif)
        row_tif=lib.TIFFOpen(single,b'r');assert row_tif
        try:
         count=lib.TIFFRawStripSize(row_tif,0);assert count>0
         reference=C.create_string_buffer((w+7)//8);assert lib.TIFFReadScanline(row_tif,reference,0,0)==1
         assert reference.raw==bytes(data)
         raw=C.create_string_buffer(count);assert lib.TIFFReadRawStrip(row_tif,0,raw,count)==count
         encoded=raw.raw+bytes(count%2)

        finally:lib.TIFFClose(row_tif)
       strips.setdefault(y//rows_per_strip,bytearray()).extend(encoded)
     if word_aligned:
      for index,encoded in strips.items():
       raw=C.create_string_buffer(bytes(encoded));assert lib.TIFFWriteRawStrip(tif,index,raw,len(encoded))==len(encoded)
    finally:lib.TIFFClose(tif)
    # Validate actual compressed scanlines independently before appearance mapping.
    tif=lib.TIFFOpen(str(path).encode(),b'r');assert tif
    rlew_mismatches=0
    try:
     decoded={}
     for y in range(h):
      if word_aligned:
       index=y//rows_per_strip
       if index not in decoded:
        size=min(rows_per_strip,h-index*rows_per_strip)*((w+7)//8)
        strip=C.create_string_buffer(size);assert lib.TIFFReadEncodedStrip(tif,index,strip,size)==size
        decoded[index]=strip.raw
       start=(y%rows_per_strip)*((w+7)//8);raw=decoded[index][start:start+(w+7)//8]
      else:
       buf=C.create_string_buffer((w+7)//8);assert lib.TIFFReadScanline(tif,buf,y,0)==1;raw=buf.raw
      for x in range(w):
       if word_aligned:rlew_mismatches+=int(((raw[x//8]>>(7-x%8))&1)!=int(black_at(x,y)))
       else:assert ((raw[x//8]>>(7-x%8))&1)==int(black_at(x,y)),(path.name,x,y)
    finally:lib.TIFFClose(tif)
    ow,oh=(w,h) if orientation<=4 else (h,w)
    rgba=bytearray(ow*oh*4)
    for y in range(h):
     for x in range(w):
      dx,dy={1:(x,y),2:(w-1-x,y),3:(w-1-x,h-1-y),4:(x,h-1-y),5:(y,x),6:(h-1-y,x),7:(h-1-y,w-1-x),8:(y,w-1-x)}[orientation]
      value=0 if black_at(x,y) else 255
      at=(dy*ow+dx)*4;rgba[at:at+4]=bytes([value,value,value,255])
    if single_row_strips:assert rlew_mismatches==0
    status=('LibTIFF full RLEW image exact via one-row strips; independent RLE row encoding and authored padding/appearance' if single_row_strips else ('LibTIFF independent RLE row pixels exact; authored 16-bit row padding and orientation; LibTIFF RLEW full-strip decoder disagreement, not an oracle' if word_aligned else 'LibTIFF scanlines exact; authored orientation mapping'))
    if mode!='w8b' and (not word_aligned or single_row_strips):
     im=Image.open(path);im.load();im=im.convert('RGBA')
     assert im.size==(ow,oh),(path.name,im.size,(ow,oh))
     assert im.tobytes()==rgba,path.name
     status+='; Pillow'+__version__+' pixels exact'
    oracle=root/'fax-dense.rgba8' if dense else (root/'fax3-long-runs.rgba8' if long_runs else (root/f'fax3-wl-fill1-opt0-ori{orientation}.rgba8' if group2 or group4 else path.with_suffix('.rgba8')))
    if (group2 or group4) and not dense:assert oracle.read_bytes()==rgba
    else:oracle.write_bytes(rgba)
    rows.append({'name':path.name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
                 **({'libtiff_rlew_decoder_mismatched_pixels':rlew_mismatches} if word_aligned else {}),
                 'width':ow,'height':oh,'rgba8_file':oracle.name,'rrrah_expected':'decoded_group2' if group2 else ('decoded_group4' if group4 else ('decoded_2d' if options&1 else 'decoded_1d')),'oracle_status':status,'rgba8_sha256':hashlib.sha256(rgba).hexdigest()})
scope=('LibTIFF full-image RLEW exact using one-row strips; independent row encoding and authored padding/appearance' if single_row_strips else 'Word-aligned raw-strip assembly from independently decoded LibTIFF RLE rows; full-strip RLEW LibTIFF disagreement retained separately; authored appearance' if word_aligned else 'Independent LibTIFF Group'+str(compression)+' encoder/scanline roundtrip; Pillow full-image checks except declared BigTIFF BE orientation controls')
(root/(prefix+'-libtiff-reference.json')).write_text(json.dumps({'scope':scope,'rows':rows},indent=2)+'\n')
print('Generated and independently checked',len(rows),family+' controls.')
