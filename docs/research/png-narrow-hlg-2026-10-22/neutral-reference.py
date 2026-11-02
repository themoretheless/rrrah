"""Independent neutral anchors: narrow RGB, signed HLG, white100/peak1000/gamma1.2."""
from decimal import Decimal as D, localcontext
import json

rows = []
with localcontext() as ctx:
    ctx.prec = 70
    for bits in [8, 16]:
        scale = 2 ** (bits - 8)
        for code in [0, 16*scale, 64*scale, 128*scale, 235*scale, (1 << bits)-1]:
            v = (D(code)-16*scale)/D(219*scale)
            if v < 0:
                scene = -v*v/D(3)
            elif v <= D('0.5'):
                scene = v*v/D(3)
            else:
                scene = (((v-D('0.55991073'))/D('0.17883277')).exp()+D('0.28466892'))/D(12)
            value = abs(scene)**D('1.2')*D(10)
            if scene < 0:
                value = -value
            rows.append(dict(bits=bits, code=code, linear=str(value)))
print(json.dumps(rows, indent=2))
