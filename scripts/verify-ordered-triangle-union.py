#!/usr/bin/env python3
"""Exact Fraction union of captured binary64 triangles inside the unit square.

Coverage only: no ownership, source-surface approximation or color oracle.
"""
import argparse
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path
import runpy
import struct
import sys


def cross(a,b,p):
    return (b[0]-a[0])*(p[1]-a[1])-(b[1]-a[1])*(p[0]-a[0])


def area(poly):
    return sum((a[0]*b[1]-b[0]*a[1] for a,b in zip(poly,poly[1:]+poly[:1])),F(0))/2


def clip(poly,a,b):
    if not poly:
        return []
    result=[];previous=poly[-1];f=cross(a,b,previous)
    for current in poly:
        g=cross(a,b,current)
        if (f>=0)!=(g>=0):
            t=f/(f-g)
            result.append(tuple(previous[k]+t*(current[k]-previous[k]) for k in range(2)))
        if g>=0:
            result.append(current)
        previous,f=current,g
    return result


def union(rows,max_pieces,progress=False):
    uncovered=[[(F(0),F(0)),(F(1),F(0)),(F(1),F(1)),(F(0),F(1))]]
    peak=1;processed=0
    for index,row in enumerate(rows):
        triangle=[tuple(F(struct.unpack('>d',struct.pack('>Q',v))[0]) for v in p) for p in row[3]]
        sign=cross(*triangle)
        if sign<0:
            triangle.reverse()
        if sign==0:
            continue
        lo=[min(p[k] for p in triangle) for k in range(2)]
        hi=[max(p[k] for p in triangle) for k in range(2)]
        next_uncovered=[]
        for subject in uncovered:
            if any(max(p[k] for p in subject)<=lo[k] or min(p[k] for p in subject)>=hi[k] for k in range(2)):
                next_uncovered.append(subject)
                continue
            inside=subject
            for a,b in zip(triangle,triangle[1:]+triangle[:1]):
                outside=clip(inside,b,a)
                if area(outside)>0:
                    next_uncovered.append(outside)
                inside=clip(inside,a,b)
                if not inside or area(inside)==0:
                    break
            if len(next_uncovered)>max_pieces:
                raise ValueError('exact proof piece limit exceeded; proof incomplete')
        uncovered=next_uncovered;peak=max(peak,len(uncovered));processed=index+1
        if len(uncovered)>max_pieces:
            raise ValueError('exact proof piece limit exceeded; proof incomplete')
        if progress and index%32==0:
            print(f'exact union processed={processed} uncovered={len(uncovered)}',file=sys.stderr,flush=True)
        if not uncovered:
            break
    value=1-sum((area(p) for p in uncovered),F(0))
    assert 0<=value<=1
    return dict(triangles=len(rows),processed=processed,uncovered_pieces=len(uncovered),peak_pieces=peak,
                union_area=float(value),union_bits=struct.unpack('>Q',struct.pack('>d',float(value)))[0],
                exact_numerator=str(value.numerator),exact_denominator=str(value.denominator))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input',type=Path)
    parser.add_argument('--capture-index',type=int,default=0)
    parser.add_argument('--max-pieces',type=int,default=100000)
    parser.add_argument('--progress',action='store_true')
    args=parser.parse_args();data=args.input.read_bytes()
    if data.lstrip().startswith(b'{'):
        rows=json.loads(data)['rows'];frame=None
    else:
        extract=runpy.run_path(str(Path(__file__).with_name('extract-ordered-cell.py')))['extract']
        captures=extract(data.decode());capture=captures[args.capture_index];rows=capture['rows'];frame=capture['frame_bits']
    result=union(rows,args.max_pieces,args.progress)
    result.update(input_sha256=hashlib.sha256(data).hexdigest(),frame_bits=frame,
                  scope='Exact rational complement subtraction of captured binary64 triangles; independent of native support IDs, priorities and floating-point clipping. Coverage only.')
    print(json.dumps(result,indent=2))
