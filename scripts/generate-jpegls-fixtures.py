"""CC0 scalar/RGB samples; FFmpeg encodings and CharLS variants with FFmpeg readback."""
from pathlib import Path
import struct,subprocess,tempfile,hashlib
root=Path('tests/fixtures/raster');encoder=Path('target/debug/examples/jpegls_fixture_encode').resolve();rows=[]
w,h=9,5
with tempfile.TemporaryDirectory() as tmp:
 tmp=Path(tmp)
 cases=[('ffmpeg',8,1,0,0),('ffmpeg',16,1,0,0),('ffmpeg',8,3,1,0)]
 for bits in (2,4,8,10,12,16):
  cases.append(('charls',bits,1,0,0))
  for ilv in (0,1,2):cases.append(('charls',bits,3,ilv,0))
 cases.extend([('charls',8,3,1,2),('charls',16,1,0,2)])
 for source,bits,components,ilv,near in cases:
  maximum=(1<<bits)-1
  values=[((p*19+c*37)%(maximum+1)) if p%3 else min(maximum,maximum//2+(p+c)%3) for p in range(w*h) for c in range(components)]
  native=values if ilv else [values[p*components+c] for c in range(components) for p in range(w*h)]
  raw=tmp/'source.raw';raw.write_bytes(bytes(native) if bits<=8 else struct.pack('<'+str(len(native))+'H',*native))
  name=f'jls-{source}-{bits}-{components}-{ilv}-{near}.jls';p=root/name
  pix=('rgb24' if bits<=8 else 'rgb48le') if components==3 else 'gray' if bits<=8 else 'gray16le'
  if source=='ffmpeg':
   subprocess.run(['/opt/homebrew/bin/ffmpeg','-hide_banner','-loglevel','error','-y','-f','rawvideo','-pixel_format',pix,'-video_size',f'{w}x{h}','-i',str(raw),'-frames:v','1','-c:v','jpegls','-f','image2',str(p)],check=True)
  else:subprocess.run([str(encoder),str(raw),str(p),str(w),str(h),str(bits),str(components),str(ilv),str(near)],check=True)
  decoded=tmp/'decoded.raw'
  result=subprocess.run(['/opt/homebrew/bin/ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(p),'-frames:v','1','-f','rawvideo','-pix_fmt',pix,str(decoded)],capture_output=True,text=True)
  if result.returncode==0 and (components==1 or (bits==8 and ilv!=2)):
   data=decoded.read_bytes();actual=list(data) if bits<=8 else list(struct.unpack('<'+str(len(data)//2)+'H',data));kind='ffmpeg'
   shift=(8 if bits<=8 else 16)-bits
   if shift:
    assert all(v & ((1<<shift)-1)==0 for v in actual),(name,'unexpected output scaling')
    actual=[v>>shift for v in actual]
   if near==0:assert actual==values,(name,actual[:9],values[:9])
   else:assert all(abs(a-b)<=near for a,b in zip(actual,values))
  else:
   if near:raise RuntimeError(result.stderr)
   actual=values;kind='authored-lossless';print(name,'independent decoder unqualified:',result.stderr.strip() or 'RGB precision/sample-interleave readback not qualified')
  out=[]
  for pixel in range(w*h):
   for c in range(3):
    v=actual[pixel*components+(c if components==3 else 0)];scale=255 if bits<=8 else 65535;out.append(((v*scale+maximum//2)//maximum)*(257 if bits<=8 else 1))
   out.append(65535)
  oracle=name+'.rgba16';(root/oracle).write_bytes(struct.pack('<'+str(len(out))+'H',*out))
  rows.append('\t'.join(map(str,[name,w,h,bits,components,ilv,near,kind,oracle,hashlib.sha256(p.read_bytes()).hexdigest()])))
(root/'jpegls-manifest.tsv').write_text('# source\twidth\theight\tbits\tcomponents\tilv\tnear\toracle_kind\toracle\tsha256\n'+'\n'.join(rows)+'\n')
