#!/usr/bin/env python3
"""Exact Fraction oracle for finite-f64 orientation; authored synthetic controls."""
import fractions, math, random, struct
from pathlib import Path
root=Path(__file__).resolve().parents[1]
rng=random.Random(20261020)
cases=[(0.,0.,1.,0.,0.,1.),(0.,0.,math.ulp(0.),0.,0.,math.ulp(0.)),
       (1e308,1e308,-1e308,1e308,0.,-1e308),(-0.,0.,0.,1.,1.,1.)]
for _ in range(128):
    x,y=[rng.uniform(-1.,1.) for _ in range(2)]
    a,b=rng.uniform(-1.,1.),rng.uniform(-1.,1.)
    t=rng.uniform(-2.,2.)
    cases.append((x,y,a,b,x+t*(a-x),math.nextafter(y+t*(b-y),rng.choice([-math.inf,math.inf]))))
    values=[math.ldexp(rng.uniform(-1.,1.),rng.randint(-1070,1020)) for _ in range(6)]
    if all(map(math.isfinite,values)):cases.append(tuple(values))
rows=[]
for values in cases:
    ax,ay,bx,by,cx,cy=map(fractions.Fraction.from_float,values)
    determinant=(bx-ax)*(cy-ay)-(by-ay)*(cx-ax)
    sign=(determinant>0)-(determinant<0)
    bits=[struct.unpack('>Q',struct.pack('>d',v))[0] for v in values]
    rows.append('    (['+', '.join(f'0x{x:016x}' for x in bits)+f'], {sign}),')
(root/'tests/fixtures/geometry/orientation-sign-cases.rs').write_text('[\n'+'\n'.join(rows)+'\n]\n')
