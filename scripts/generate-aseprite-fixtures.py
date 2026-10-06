"""CC0 Aseprite arrays and layered frame contracts; independent asefile readback."""
from pathlib import Path
from PIL import Image
import hashlib,struct,zlib,subprocess,tempfile
root=Path('tests/fixtures/raster');rows=[]
project=Path(tempfile.gettempdir())/'rrrah-aseprite-oracle'
(project/'src').mkdir(parents=True,exist_ok=True)
def write_changed(path,data):
 if not path.exists() or path.read_bytes()!=data:path.write_bytes(data)
write_changed(project/'Cargo.toml',('[package]\nname="rrrah-aseprite-oracle"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nasefile="=0.3.8"\n').encode())
write_changed(project/'src/main.rs',Path(__file__).with_name('aseprite-fixture-oracle.rs').read_bytes())
subprocess.run(['cargo','build','--manifest-path',str(project/'Cargo.toml')],check=True)
oracle=project/'target/debug/rrrah-aseprite-oracle'
def string(s):
 b=s.encode();return struct.pack('<H',len(b))+b
def chunk(kind,b):return struct.pack('<IH',len(b)+6,kind)+b
def layer(name,flags=1,opacity=255,group=False,level=0,blend=0):
 return chunk(0x2004,struct.pack('<HHHHHHB',flags,int(group),level,0,0,blend,opacity)+bytes(3)+string(name))
def cel(index,pixels,w,h,x=0,y=0,opacity=255,compressed=True,z=0,link=None):
 typ=1 if link is not None else (2 if compressed else 0)
 b=struct.pack('<HhhBHh',index,x,y,opacity,typ,z)+bytes(5)
 if link is not None:b+=struct.pack('<H',link)
 else:b+=struct.pack('<HH',w,h)+(zlib.compress(bytes(pixels),9) if compressed else bytes(pixels))
 return chunk(0x2005,b)
def palette(colors,kind=0x2019,first=0,size=None):
 if kind==0x2019:
  b=struct.pack('<III',size or len(colors),first,first+len(colors)-1)+bytes(8)
  for i,c in enumerate(colors):b+=struct.pack('<H',1)+bytes(c)+string('Color '+str(i+first))
 else:
  b=struct.pack('<HBB',1,first,0 if len(colors)==256 else len(colors))+b''.join(bytes(c[:3]) for c in colors)
 return chunk(kind,b)
def profile(kind=1,linear=False,icc=None):
 b=struct.pack('<HHI',kind,int(linear),65536 if linear else 0)+bytes(8)
 if kind==2:b+=struct.pack('<I',len(icc))+icc
 return chunk(0x2007,b)
def tags():return chunk(0x2018,struct.pack('<H',1)+bytes(8)+struct.pack('<HHBH',0,2,2,3)+bytes(10)+string('bounce'))
def file(frames,depth=32,flags=1,w=5,h=3):
 h1=bytearray(128);struct.pack_into('<HHHHHIH',h1,4,0xa5e0,len(frames),w,h,depth,flags,60);h1[34:36]=bytes([1,1])
 body=b''
 for i,parts in enumerate(frames):
  data=b''.join(parts);body+=struct.pack('<IHHH2xI',16+len(data),0xf1fa,len(parts),[60,125,250][i],0)+data
 struct.pack_into('<I',h1,0,128+len(body));return bytes(h1)+body

def mul(a,b):
 t=a*b+128;return (t+(t>>8))>>8
def over(dst,src,opacity):
 alpha=mul(src[3],opacity)
 if dst[3]==0:return list(src[:3])+[alpha]
 if src[3]==0:return dst
 out=alpha+dst[3]-mul(dst[3],alpha)
 def trunc(n,d):return (abs(n)//d)*(1 if n>=0 else -1)
 return [dst[c]+trunc((src[c]-dst[c])*alpha,out) for c in range(3)]+[out]
def render(cels):
 out=[[0,0,0,0] for _ in range(15)]
 for pixels,w,h,x,y,opacity in cels:
  for sy in range(h):
   for sx in range(w):
    dx,dy=x+sx,y+sy
    if 0<=dx<5 and 0<=dy<3:
     out[dy*5+dx]=over(out[dy*5+dx],pixels[sy*w+sx],opacity)
 return out
def save(name,b,expected,independent=True):
 p=root/name;p.write_bytes(b)
 for index,pixels in enumerate(expected):
  raw=bytes(sum(pixels,[]));kind='authored-native-sample-composition'
  out=name+f'.frame-{index}.rgba'
  if independent:
   tmp=root/(out+'.tmp');result=subprocess.run([str(oracle),str(p),str(index),str(tmp)],capture_output=True,text=True)
   assert result.returncode==0,(name,index,result.stderr)
   assert tuple(map(int,result.stdout.split()))==(5,3,len(expected),[60,125,250][index])
   actual=tmp.read_bytes();tmp.unlink();assert actual==raw,(name,index,next((i,a,e) for i,(a,e) in enumerate(zip(actual,raw)) if a!=e))
   raw=actual;kind='independent-asefile'
  (root/out).write_bytes(raw)
  rows.append('\t'.join(map(str,[name,index,5,3,len(expected),[60,125,250][index],kind,out,hashlib.sha256(b).hexdigest()])))

def variants(depth,compressed,kind=0x2019,flags=1):
 colors=[[120,10,30,255],[255,20,0,255],[0,180,30,255],[0,20,230,255],[190,30,200,255],[25,170,200,255]]
 if kind==0x11:colors=[[i,63-i,(i*3)%64,255] for i in (0,8,16,31,32,63)]
 resolved=[[((v<<2)|(v>>4)) if kind==0x11 else v for v in c[:3]]+[255] for c in colors]
 def source(version,w=5,h=3):
  raw=[];rgba=[]
  for y in range(h):
   for x in range(w):
    if depth==32:
     p=[(23+x*41+version*7)%256, (91+y*37)%256, (211-x*13)%256, [0,77,128,255,33][x%5]];raw+=p
    elif depth==16:
     v=(x*41+y*19+version*7)%256;p=[v,v,v,[0,77,128,255,33][x%5]];raw += [v,p[3]]
    else:
     index=(x+y+version)%6;p=resolved[index];raw += [index]
    rgba.append(p)
  return raw,rgba
 frames=[];expected=[]
 opacity=87 if flags&1 else 255
 for i in range(3):
  base,b_rgba=source(0 if i<2 else i)
  overlay,o_rgba=source(i+1,3,2)
  if depth==8:
   overlay[0]=0;o_rgba[0]=resolved[0][:3]+[0]
   o_rgba=[p[:3]+[0] if overlay[n]==0 else p for n,p in enumerate(o_rgba)]
  parts=[]
  if i==0:
   parts=[layer('Background',flags=9),layer('Overlay',opacity=87),layer('Hidden',flags=0),profile(),tags()]
   if depth==8:parts+=[palette(colors,kind)]
  parts += [cel(0,base,5,3,compressed=compressed,link=0 if i==1 else None),cel(1,overlay,3,2,-1,1,153,compressed),cel(2,[255,0,255,255] if depth==32 else ([255,255] if depth==16 else [1]),1,1,compressed=compressed)]
  frames.append(parts)
  expected.append(render([(b_rgba,5,3,0,0,255),(o_rgba,3,2,-1,1,mul(opacity,153))]))
 name=f'ase-{depth}-{int(compressed)}-{kind:x}-{flags}.aseprite'
 save(name,file(frames,depth,flags),expected,independent=bool(flags&1))
for depth in (32,16,8):
 for compressed in (False,True):variants(depth,compressed)
for kind in (0x4,0x11):
 for compressed in (False,True):variants(8,compressed,kind)
variants(32,True,flags=0)
# Z-order tie: orders coincide, then smaller cel z comes first.
red=[[255,0,0,255]]*15;green=[[0,255,0,255]]*15;blue=[[0,0,255,255]]*15
frames=[]
for i in range(3):
 parts=[layer('Red'),layer('Green'),layer('Blue'),profile(),tags()] if i==0 else []
 parts += [cel(0,sum(red,[]),5,3,z=2),cel(1,sum(green,[]),5,3,z=1),cel(2,sum(blue,[]),5,3,z=0)]
 frames.append(parts)
save('ase-z-order.aseprite',file(frames),[red]*3,False)
# Linked cel placement and opacity come from the selected cel, not its source.
pixels=[[40,210,90,200]]*6;frames=[];expected=[]
for i,(x,y,opacity) in enumerate([(1,0,255),(-1,1,153),(2,-1,87)]):
 parts=[layer('Moved link'),profile(),tags()] if i==0 else []
 parts += [cel(0,sum(pixels,[]),3,2,x,y,opacity,link=i-1 if i else None)]
 frames.append(parts);expected.append(render([(pixels,3,2,x,y,opacity)]))
save('ase-moving-linked-cel.aseprite',file(frames),expected,False)
# Isolated normal groups, partial group opacity and hidden descendants.
frames=[];expected=[]
for i in range(3):
 parts=[layer('Base'),layer('Group',group=True,opacity=173),layer('Group child',level=1),layer('Hidden group',flags=0,group=True),layer('Hidden child',level=1),profile(),tags()] if i==0 else []
 fg=[[180,20+i*40,200,128]]*6
 parts += [cel(0,sum(blue,[]),5,3),cel(2,sum(fg,[]),3,2,1,1,153),cel(4,sum(red,[]),5,3)]
 group=render([(fg,3,2,1,1,153)])
 expected.append(render([(blue,5,3,0,0,255),(group,5,3,0,0,173)]));frames.append(parts)
save('ase-isolated-groups.aseprite',file(frames,flags=3),expected,False)
# Selected-frame palette changes also recolor linked indexed storage.
colors=[[120,10,30,255],[255,0,0,255],[0,255,0,255],[0,0,255,255]]
indices=[1,2,3,0,1]*3;frames=[];expected=[]
for i in range(3):
 current=colors.copy();current[1]=[30,50+i*70,210,255]
 parts=[layer('Indexed link'),profile(),tags(),palette(current)] if i==0 else [palette([current[1]],first=1,size=4)]
 parts += [cel(0,indices,5,3,link=i-1 if i else None)];frames.append(parts)
 rgba=[current[n][:3]+[0 if n==0 else current[n][3]] for n in indices]
 expected.append(render([(rgba,5,3,0,0,255)]))
save('ase-changing-palette.aseprite',file(frames,depth=8),expected,False)
# Color metadata raw samples are compared independently; transform is tested in Rust.
for name,p in [('linear',profile(linear=True)),('icc',profile(2,icc=Image.open(root/'pattern.profiled.png').info['icc_profile'])),('assumed',profile(0))]:
 frames=[];expected=[]
 for i in range(3):
  pixels=[[20+i*30,87,180,128]]*15
  frames.append(([layer('Profile'),p,tags()] if i==0 else [])+[cel(0,sum(pixels,[]),5,3)])
  expected.append(render([(pixels,5,3,0,0,255)]))
 save('ase-profile-'+name+'.aseprite',file(frames),expected,independent=name=='assumed')
# Modern palettes override old chunks even when the legacy chunk comes later.
colors=[[120,10,30,255],[255,0,0,255],[0,255,0,255],[0,0,255,255]]
indices=[1,2,3,0,1]*3;frames=[];expected=[]
for i in range(3):
 parts=[layer('Palette'),profile(),tags(),palette(colors),palette([[12,34,56,255]]*4,kind=4)] if i==0 else []
 parts += [cel(0,indices,5,3)];frames.append(parts)
 rgba=[colors[n][:3]+[0 if n==0 else 255] for n in indices];expected.append(render([(rgba,5,3,0,0,255)]))
save('ase-palette-precedence.aseprite',file(frames,depth=8),expected)
# Legacy zero packet count encodes all 256 entries.
colors=[[i,255-i,(i*7)%256,255] for i in range(256)]
indices=list(range(15));frames=[];expected=[]
for i in range(3):
 parts=[layer('Palette background',flags=9),profile(),tags(),palette(colors,kind=4)] if i==0 else []
 parts += [cel(0,indices,5,3)];frames.append(parts)
 expected.append(render([([colors[n] for n in indices],5,3,0,0,255)]))
save('ase-palette-256.aseprite',file(frames,depth=8),expected)
# Forward linked storage and a link chain are legal; all placement is identical.
frames=[];expected=[];pixels=[[34,200,87,153]]*15
for i in range(3):
 parts=[layer('Forward link'),profile(),tags()] if i==0 else []
 parts += [cel(0,sum(pixels,[]),5,3,link=2 if i==0 else (0 if i==1 else None))]
 frames.append(parts);expected.append(render([(pixels,5,3,0,0,255)]))
save('ase-forward-linked-cels.aseprite',file(frames),expected,False)
(root/'aseprite-manifest.tsv').write_text('# source\tindex\twidth\theight\tframes\tdelay_ms\toracle_kind\toracle\tsha256\n'+'\n'.join(rows)+'\n')
print(f'{len(rows)} selected frame contracts')
