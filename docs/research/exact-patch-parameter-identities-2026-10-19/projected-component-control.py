#!/usr/bin/env python3
"""Independent exact-rational chord interpolation of the upper folded branch.

This1Dcontrol does not reproduce the adaptive stream's boundary fans. It tests
why parameter-linear source color need not remain affine after projection.
"""
from fractions import Fraction as F
from decimal import Decimal,localcontext
import json,math,struct
from pathlib import Path
with localcontext() as context:
 context.prec=80
 third=Decimal(1)/3
 expected=Decimal('0.5')+third*third.sqrt()
 rows=[]
 for count in [8,16,32,64,128,256]:
  total=F(0);bounds=[]
  for j in range(count//2,count):
   u0,u1=F(j,count),F(j+1,count)
   x0,x1=6*u0*(1-u0),6*u1*(1-u1)
   # C(x(u))-u=(u-u0)*(u1-u)/(1-u0-u1); its exact maximum
   # magnitude on this upper-branch cell is h^2/(4*(u0+u1-1)).
   bounds.append((u1-u0)**2/(4*(u0+u1-1)))
   left,right=max(F(1),x1),min(F(3,2),x0)
   if right<=left:continue
   slope=(u0-u1)/(x0-x1);constant=u1-slope*x1
   total+=constant*(right-left)+slope*(right*right-left*left)/2
  average=total*2
  value=Decimal(average.numerator)/Decimal(average.denominator)
  f32=struct.unpack('<f',struct.pack('<f',float(average)))[0]
  rows.append(dict(uniform_u_segments=count,exact_average_rational=str(average),average_decimal=str(value),error_rgb8_levels=str((value-expected)*255),rounded_rgb8=math.floor(f32*255+0.5),max_projected_component_residual=str(max(bounds)),cells_above_1_1024_component_error=sum(b>F(1,1024) for b in bounds)))
 result=dict(scope=__doc__,analytic_second_pixel_average=str(expected),analytic_rgb8_level=str(expected*255),analytic_rounded_rgb8=177,rows=rows)
Path(__file__).with_name('projected-component-control.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
