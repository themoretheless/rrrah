#!/usr/bin/env python3
import pathlib,struct,subprocess,argparse,hashlib
parser=argparse.ArgumentParser(description='Reconstruct a channel from pinned HERO9 VC-5 using independent GoPro C oracles')
parser.add_argument('--work',type=pathlib.Path,required=True,help='HERO9.vc5 and all-bands/channel-0-band-N.oracle.i32le')
parser.add_argument('--channel',type=int,choices=range(4),default=0)
parser.add_argument('--spatial-oracle',type=pathlib.Path,required=True)
args=parser.parse_args()
root=args.work;data=(root/'HERO9.vc5').read_bytes();
assert hashlib.sha256(data).hexdigest()=='093c2055d8435f7776e415a318226330a22283faa025bfc28967bc562730533b','Pinned HERO9 essence required'
state={'channel':0,'band':0,'quant':0};blocks={}
def walk(data,o=0):
 while o<len(data):
  t,v=struct.unpack_from('>hH',data,o);o+=4;t=abs(t)
  if t&0x6000:
   n=((((t&255)<<16)|v) if t&0x2000 else v)*4;p=data[o:o+n];o+=n
   if t&0xff00 in [0x2400,0x2100,0x2000]:walk(p)
   elif t&0xff00==0x6000:blocks[state['channel'],state['band']]=(p,state['quant'])
  elif t==62:state['channel']=v
  elif t==48:state['band']=v
  elif t==53:state['quant']=v
walk(data,4)
channel=args.channel
low=blocks[channel,0][0];low=struct.pack('<'+'h'*(len(low)//2),*struct.unpack('>'+'h'*(len(low)//2),low))
for level in range(3):
 width=348<<level;height=261<<level;first=1+level*3;qs=[blocks[channel,i][1] for i in range(first,first+3)]
 high=[]
 for i in range(first,first+3):
  b=(root/'all-bands'/f'channel-{channel}-band-{i}.oracle.i32le').read_bytes();high.append(struct.pack('<'+'h'*(len(b)//4),*struct.unpack('<'+'i'*(len(b)//4),b)))
 source=root/f'channel-{channel}-level-{level}-oracle-input.bin';source.write_bytes(struct.pack('<8H',width,height,width*2,height*2,[2,2,0][level],*qs)+low+b''.join(high))
 output=root/f'channel-{channel}-level-{level}-oracle.i16le';subprocess.run([str(args.spatial_oracle.resolve()),str(source),str(output)],check=True);low=output.read_bytes();print(level,len(low)//2,flush=True)
