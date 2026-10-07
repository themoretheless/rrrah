#!/usr/bin/env python3
"""Authored synthetic HDR RPF workload; not a rendered producer oracle."""
import argparse
import pathlib
import struct
p=argparse.ArgumentParser();p.add_argument('output',type=pathlib.Path);p.add_argument('--width',type=int,default=1024);p.add_argument('--height',type=int,default=512);p.add_argument('--pattern',choices=['constant','varying'],default='constant');a=p.parse_args()
if not (1 <= a.width <= 16383 and 1 <= a.height <= 32768):p.error('dimensions exceed fixture header/float-record bounds')
sizes=[4,1,2,8,4,4,1,3,2,3,3,8,3,2]
h=bytearray(740)
for at,v in [(2,a.width-1),(6,a.height-1),(10,a.width-1),(14,a.height-1),(20,3),(22,1),(26,0xfffd),(658,32),(662,32)]:struct.pack_into('>H',h,at,v)
program=b'3ds max : ( Z E O U N R C B I G T V W M L A )';h[400:400+len(program)]=program
aspect=f'{a.width/a.height:.5f}'.encode()
if len(aspect)>7:p.error('fixture aspect does not fit bounded header field')
h[572:572+len(aspect)]=aspect
row=bytearray()
for value in [0.25,1.,4.,0.5]:
 payload=struct.pack('>f',value)*a.width;row+=struct.pack('>H',len(payload))+payload
for channel,size in enumerate(sizes):
 for plane in reversed(range(size)):
  value=(channel*13+plane*17)&255;payload=bytearray();remaining=a.width
  while remaining:
   run=min(128,remaining)
   if a.pattern=='constant':payload+=bytes([run-1,value])
   else:payload+=bytes([(-run)&255])+bytes((value+(a.width-remaining+i)*37)&255 for i in range(run))
   remaining-=run
  row+=struct.pack('>H',len(payload))+payload
row+=struct.pack('<i',0)
base=740+4*a.height
with a.output.open('wb') as f:
 f.write(h)
 for y in reversed(range(a.height)):f.write(struct.pack('>I',base+y*len(row)))
 for _ in range(a.height):f.write(row)
print(a.output.stat().st_size)
