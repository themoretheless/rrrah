from pathlib import Path
import struct,json,hashlib,zlib,math
from fractions import Fraction
from psd_tools import PSDImage
from psd_tools.constants import Compression
from importlib.metadata import version
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent;fixtures=root/'tests/fixtures/raster';records=[]
w,h=9,3
for depth in [16,32]:
 for mode in ['RGBA','LA']:
  channels=4 if mode=='RGBA' else 2
  planes=[]
  alphas=[(0.,.25,.5,1.)[(x+2*y)%4] for y in range(h) for x in range(w)]
  palette=[0.,.125,.25,.5,.75,1.] if depth==16 else [-.5,-0.,1e-30,.125,.5,2.,4.,16.]
  for channel in range(channels):
   values=[]
   for y in range(h):
    for x in range(w):
     a=alphas[y*w+x];c=palette[(x+3*y+2*channel)%len(palette)]
     value=a if channel==channels-1 else c if a==1 else c*a+1-a
     values.append(value)
   planes.append(b''.join(struct.pack('>H',round(v*65535)) if depth==16 else struct.pack('>f',v) for v in values))
  raw=b''.join(planes);stored=struct.unpack('>'+str(w*h*channels)+('H' if depth==16 else 'f'),raw)
  native=bytearray()
  for pixel in range(w*h):
   alpha=stored[(channels-1)*w*h+pixel]
   for channel in range(4):
    src=channels-1 if channel==3 else 0 if channels==2 else channel;v=stored[src*w*h+pixel]
    if channel<3 and 0<alpha<(65535 if depth==16 else 1):
     if depth==16:
      n=Fraction((v+alpha-65535)*65535,alpha);v=max(0,min(65535,n.numerator//n.denominator))
     else:v=(v+alpha-1)/alpha
    native+=struct.pack('<H',v) if depth==16 else struct.pack('<f',v)
  for file_version in [1,2]:
   for compression in range(4):
    image=PSDImage.new(mode,(w,h),depth=depth,compression=Compression(compression));image._record.header.version=file_version;image._record.image_data.set_data(planes,image._record.header)
    ext='psd' if file_version==1 else 'psb';name=f'psd-tools-alpha-multi-{mode.lower()}-{depth}-v{file_version}-c{compression}.{ext}';path=fixtures/name;image.save(path);b=path.read_bytes();at=26
    for section in range(3):
     size=8 if file_version==2 and section==2 else 4;n=int.from_bytes(b[at:at+size],'big');at+=size+n
    assert int.from_bytes(b[at:at+2],'big')==compression;at+=2;payload=b[at:]
    if compression==0:decoded=payload
    elif compression==1:
     size=4 if file_version==2 else 2;lengths=[int.from_bytes(payload[i*size:(i+1)*size],'big') for i in range(channels*h)];pos=size*len(lengths);decoded=bytearray()
     for n in lengths:
      block=payload[pos:pos+n];pos+=n;i=0;row=bytearray()
      while i<len(block):
       code=block[i];i+=1
       if code<128:row+=block[i:i+code+1];i+=code+1
       elif code>128:row+=bytes([block[i]])*(257-code);i+=1
      assert len(row)==w*(depth//8);decoded+=row
     assert pos==len(payload);decoded=bytes(decoded)
    else:
     z=zlib.decompressobj();decoded=z.decompress(payload)+z.flush();assert z.eof and not z.unused_data and not z.unconsumed_tail
     if compression==3:
      restored=bytearray();row_bytes=w*(depth//8)
      for offset in range(0,len(decoded),row_bytes):
       row=bytearray(decoded[offset:offset+row_bytes]);previous=0
       if depth==16:
        for x in range(w):previous=(previous+int.from_bytes(row[2*x:2*x+2],'big'))&65535;row[2*x:2*x+2]=previous.to_bytes(2,'big')
       else:
        for x in range(len(row)):previous=(previous+row[x])&255;row[x]=previous
        row=bytearray(row[c*w+x] for x in range(w) for c in range(4))
       restored+=row
      decoded=bytes(restored)
    assert decoded==raw,(name,'independent compressed-plane mismatch')
    values=PSDImage.open(path).numpy();assert values.shape==(h,w,channels)
    maximum=0.
    for pixel in range(w*h):
     for c in range(channels):
      rgba=3 if c==channels-1 else c;pos=(pixel*4+rgba)*(depth//8);actual=struct.unpack_from('<H' if depth==16 else '<f',native,pos)[0];actual=actual/65535 if depth==16 else actual
      error=abs(actual-float(values.reshape(-1,channels)[pixel,c]));maximum=max(maximum,error);assert math.isfinite(actual) and error<=(1/65535 if depth==16 else 2e-6),(name,pixel,c,actual,values.reshape(-1,channels)[pixel,c])
    oracle=name+('.rgba16le' if depth==16 else '.rgba32f');(fixtures/oracle).write_bytes(native)
    records.append({'fixture':name,'oracle':oracle,'width':w,'height':h,'channels':channels,'depth':depth,'version':file_version,'compression':compression,'source_sha256':hashlib.sha256(b).hexdigest(),'oracle_sha256':hashlib.sha256(native).hexdigest(),'max_normalized_error_vs_psdtools':maximum})
(out/'manifest.json').write_text(json.dumps({'producer':'psd-tools'+version('psd-tools'),'scope':'authored multirow alpha producer controls, not original Photoshop files; native validation pending','cases':records},indent=2)+'\n')
(fixtures/'psd-tools-alpha-multi-manifest.tsv').write_text('# source\toracle\twidth\theight\tdepth\tchannels\tcompression\tsource_sha256\n'+'\n'.join('\t'.join(str(r[k]) for k in ['fixture','oracle','width','height','depth','channels','compression','source_sha256']) for r in records)+'\n')
print(json.dumps({'cases':len(records),'status':'independent raw-plane/compression/oracle controls generated; native validation pending'}))
