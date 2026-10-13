"""Retag independent float EXR without changing channel chunks; Gaussian color oracle."""
from pathlib import Path
import struct, hashlib, json, runpy
root=Path(__file__).resolve().parents[1];fixtures=root/'tests/fixtures/raster'
base=(fixtures/'float-none.exr').read_bytes();at=8
while base[at]:
    at=base.index(0,at)+1;at=base.index(0,at)+1
    at+=4+struct.unpack_from('<I',base,at)[0]
at+=1;first=struct.unpack_from('<Q',base,at)[0]
assert (first-at)%8==0

def attr(name,kind,value):
    return name.encode()+b'\0'+kind.encode()+b'\0'+struct.pack('<I',len(value))+value

def encode(attrs):
    extra=b''.join(attr(*a) for a in attrs)
    offsets=b''.join(struct.pack('<Q',v+len(extra)) for (v,) in struct.iter_unpack('<Q',base[at:first]))
    return base[:at-1]+extra+b'\0'+offsets+base[first:]

matrix=runpy.run_path(str(root/'scripts/probe-linear-color-oracle.py'))['matrix']
samples=struct.unpack('<24f',(fixtures/'float-none.exr.f32').read_bytes())
manifest={'source_sha256':hashlib.sha256(base).hexdigest(),'cases':[],'files':{}}
for name,xy,ident in [('custom',[.64,.33,.30,.60,.15,.06,.34567,.35850],None),
    ('ap1',[.713,.293,.165,.830,.128,.044,.32168,.33767],'lin_ap1_scene'),
    ('ap0',[.7347,.2653,0,1,.0001,-.0770,.32168,.33767],'lin_ap0_scene')]:
    rounded=list(struct.unpack('<8f',struct.pack('<8f',*xy)))
    c=('chromaticities','chromaticities',struct.pack('<8f',*xy))
    n=('adoptedNeutral','v2f',struct.pack('<2f',*xy[6:]))
    flag=('acesImageContainerFlag','int',struct.pack('<i',1))
    cases={'chroma':([c],'accepted',rounded), 'neutral-match':([c,n],'accepted',rounded),
           'neutral-other':([c,('adoptedNeutral','v2f',struct.pack('<2f',.3333,.3333))],'accepted',rounded),
           'unknown-id':([c,('colorInteropID','string',b'unknown')],'unknown',None),
           'unknown-id-neutral':([c,n,('colorInteropID','string',b'unknown')],'unknown',None),
           'invalid-neutral':([c,('adoptedNeutral','v2f',struct.pack('<2f',0,-.1))],'refused',None),
           'id-conflict':([c,('colorInteropID','string',b'lin_rec709_scene')],'refused',None),
           'singular':([('chromaticities','chromaticities',struct.pack('<8f',.64,.33,.64,.33,.64,.33,.3127,.329))],'refused',None),
           'aces':([c,flag],'accepted' if name=='ap0' else 'refused',rounded if name=='ap0' else None)}
    if ident:
        i=('colorInteropID','string',ident.encode())
        cases.update({'id':([i],'accepted',xy),'both':([c,i],'accepted',xy),
                      'id-neutral':([i,('adoptedNeutral','v2f',struct.pack('<2f',.3333,.3333))],'accepted',xy)})
        if name=='ap0':cases['aces-id']=([i,flag],'accepted',xy)
    for case,(attrs,outcome,coords) in cases.items():
        path=fixtures/f'arbitrary-{name}-{case}.exr';data=encode(attrs);path.write_bytes(data)
        manifest['files'][path.name]=hashlib.sha256(data).hexdigest()
        row={'name':path.name,'outcome':outcome}
        if outcome=='accepted':
            adopted=next((list(struct.unpack('<2f',value)) for key,kind,value in attrs if key=='adoptedNeutral'),None)
            m=matrix(coords,True,adopted);values=[]
            for pixel in range(0,24,4):
                values.extend(sum(r[c]*samples[pixel+c] for c in range(3)) for r in m)
                values.append(samples[pixel+3])
            oracle=path.with_suffix('.linear-srgb.f32');oracle.write_bytes(struct.pack('<24f',*values))
            manifest['files'][oracle.name]=hashlib.sha256(oracle.read_bytes()).hexdigest()
            double=matrix(coords,False,adopted)
            row.update({'xy':coords,'adopted_neutral':adopted,'matrix':m,'double_cone_delta':max(abs(x-y) for r,s in zip(m,double) for x,y in zip(r,s)),
                        'oracle':oracle.name})
        manifest['cases'].append(row)
(root/'tests/fixtures/exr-arbitrary-color.json').write_text(json.dumps(manifest,indent=2)+'\n')
