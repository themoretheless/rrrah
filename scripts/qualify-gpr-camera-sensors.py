#!/usr/bin/env python3
"""Compare pinned official-camera sensors with staged independent GoPro C functions.
This is not an independent complete SDK parser or rendered-color qualification.
"""
import argparse, hashlib, json, pathlib, struct, subprocess
p=argparse.ArgumentParser()
p.add_argument('--work',type=pathlib.Path,required=True)
p.add_argument('--manifest',type=pathlib.Path,required=True)
p.add_argument('--entropy-oracle',required=True)
p.add_argument('--spatial-oracle',required=True)
p.add_argument('--bayer-oracle',required=True)
p.add_argument('--report',type=pathlib.Path,required=True)
a=p.parse_args(); rows=[]
for pinned in json.loads(a.manifest.read_text()):
 name=pinned['source']; source=a.work/(name+'.GPR'); b=source.read_bytes()
 assert hashlib.sha256(b).hexdigest()==pinned['sha256']
 off=struct.unpack_from('<I',b,4)[0]; count=struct.unpack_from('<H',b,off)[0]; tags={}
 for i in range(count):
  tag,kind,n,value=struct.unpack_from('<HHII',b,off+2+i*12); tags[tag]=(kind,n,value)
 assert tags[324][:2]==(4,1) and tags[325][:2]==(4,1)
 essence=b[tags[324][2]:tags[324][2]+tags[325][2]];assert essence[:4]==b'VC-5'
 state={'channel':0,'band':0,'q':0};blocks={}
 def walk(data,o=0):
  while o<len(data):
   t,v=struct.unpack_from('>hH',data,o);o+=4;t=abs(t)
   if t&0x6000:
    n=((((t&255)<<16)|v) if t&0x2000 else v)*4; payload=data[o:o+n];assert len(payload)==n;o+=n
    if t&0xff00 in (0x2400,0x2100,0x2000):walk(payload)
    elif t&0xff00==0x6000:blocks[state['channel'],state['band']]=(payload,state['q'])
   elif t==62:state['channel']=v
   elif t==48:state['band']=v
   elif t==53:state['q']=v
   elif t in (20,21,109):state[t]=v
 walk(essence,4); w,h=state[20],state[21];assert w%2==h%2==0
 cw,ch=w//2,h//2; dims=[((cw+d-1)//d,(ch+d-1)//d) for d in (8,4,2)]
 work=a.work/(name+'-oracle');work.mkdir(exist_ok=True);channels=[]
 for c in range(4):
  low=blocks[c,0][0];low=struct.pack('<'+'h'*(len(low)//2),*struct.unpack('>'+'h'*(len(low)//2),low))
  for level,(bw,bh) in enumerate(dims):
   first=1+3*level;high=[];qs=[]
   for band in range(first,first+3):
    payload,q=blocks[c,band]; block=work/'block';block.write_bytes(payload); out=work/'entropy.i32le'
    subprocess.run([a.entropy_oracle,str(block),str(bw*bh),str(out)],check=True)
    values=out.read_bytes();high.append(struct.pack('<'+'h'*(len(values)//4),*struct.unpack('<'+'i'*(len(values)//4),values)));qs.append(q)
   ow,oh=dims[level+1] if level<2 else (cw,ch)
   scale=(state[109] >> (14-(2-level)*2))&3
   inp=work/'spatial.bin';inp.write_bytes(struct.pack('<8H',bw,bh,ow,oh,scale,*qs)+low+b''.join(high))
   out=work/f'channel-{c}.i16le';subprocess.run([a.spatial_oracle,str(inp),str(out)],check=True);low=out.read_bytes()
  channels.append(str(out))
 oracle=work/'sensor.u16le';subprocess.run([a.bayer_oracle,str(cw),str(ch),*channels,str(oracle)],check=True)
 native=(a.work/(name+'.u16le')).read_bytes();expected=oracle.read_bytes();assert native==expected,name
 rows.append({'source':name,'source_sha256':pinned['sha256'],'sensor':[w,h],'samples':len(native)//2,'sensor_sha256':hashlib.sha256(native).hexdigest(),'exact':True})
 print(name,'full sensor exact',flush=True)
a.report.write_text(json.dumps({'scope':__doc__,'sources':rows,'total_samples':sum(r['samples'] for r in rows)},indent=2)+'\n')
