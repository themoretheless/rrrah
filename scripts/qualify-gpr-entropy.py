#!/usr/bin/env python3
import pathlib,struct,subprocess,hashlib,json,time,argparse
parser=argparse.ArgumentParser(description='Qualify all HERO9 VC-5 highpass coefficients against an independent GoPro oracle')
parser.add_argument('--source',type=pathlib.Path,required=True)
parser.add_argument('--oracle',type=pathlib.Path,required=True)
parser.add_argument('--native',type=pathlib.Path,required=True)
parser.add_argument('--work',type=pathlib.Path,required=True)
parser.add_argument('--report',type=pathlib.Path,required=True)
args=parser.parse_args()
b=args.source.read_bytes()
assert hashlib.sha256(b).hexdigest()=='9249208a3482d6d71d39575210fe19ab3dd737e5e446a4d0718d4bd9d3c4358b','Pinned official HERO9 source required'
essence=b[14622:];work=args.work;work.mkdir(parents=True,exist_ok=True)
def records(data,offset=0):
 while offset<len(data):
  tag,value=struct.unpack_from('>hH',data,offset);offset+=4;tag=abs(tag)
  if tag&0x6000:
   length=((((tag&255)<<16)|value) if tag&0x2000 else value)*4
   payload=data[offset:offset+length];assert len(payload)==length;offset+=length
   yield tag,value,payload
  else:yield tag,value,None
rows=[];started=time.monotonic()
channels=[p for t,v,p in records(essence,4) if t&0xff00==0x2400]
for channel,data in enumerate(channels):
 waves=[p for t,v,p in records(data) if t&0xff00==0x2100]
 for level,wave in enumerate(waves):
  bands=[p for t,v,p in records(wave) if t&0xff00==0x2000]
  for local,band in enumerate(bands):
   if level==0 and local==0:continue
   index=local if level==0 else 4+(level-1)*3+local
   count=(348<<level)*(261<<level)
   block=next(p for t,v,p in records(band) if t&0xff00==0x6000)
   name=f'channel-{channel}-band-{index}';source=work/(name+'.block');source.write_bytes(block)
   oracle=work/(name+'.oracle.i32le');native=work/(name+'.native.i32le')
   subprocess.run([str(args.oracle.resolve()),str(source),str(count),str(oracle)],check=True)
   subprocess.run([str(args.native.resolve()),str(source),str(count),str(native)],check=True)
   a=native.read_bytes();r=oracle.read_bytes();assert len(a)==len(r)==count*4
   assert a==r,name
   rows.append({'channel':channel,'band':index,'coefficients':count,'block_bytes':len(block),'block_sha256':hashlib.sha256(block).hexdigest(),'coefficient_sha256':hashlib.sha256(a).hexdigest(),'exact':True})
   print(name,count,'exact',flush=True)
report={'source':{'sha256':hashlib.sha256(b).hexdigest(),'repository':'https://github.com/gopro/gpr','commit':'446c736a38fb14f51343605c0780d347dc602f89','path':'data/samples/HERO9/GOPR0002.GPR'},'scope':'All 36 highpass entropy bands, signed quantized coefficients before inverse companding and wavelet','bands':rows,'total_coefficients':sum(r['coefficients'] for r in rows),'differing_coefficients':0,'elapsed_seconds':time.monotonic()-started,'oracle':'Unmodified pinned GoPro GetRun/GetRlv/table17 plus test-only bitstream host adapter'}
args.report.write_text(json.dumps(report,indent=2)+'\n')
print('TOTAL',report['total_coefficients'],report['elapsed_seconds'],flush=True)
