"""Independent high precision anchors: PNG3 range extension and ST2084 EOTF."""
from decimal import Decimal as D, localcontext
import json

rows = []
with localcontext() as ctx:
    ctx.prec = 70
    for bits in [8, 16]:
        scale = 2 ** (bits - 8)
        for code in [0, 16 * scale, 64 * scale, 128 * scale, 235 * scale, (1 << bits) - 1]:
            x = (D(code) - D(16 * scale)) / D(219 * scale)
            p = abs(x) ** (D(32) / D(2523))
            value = (max(p - D(107) / D(128), D(0)) / (D(2413) / D(128) - D(2392) / D(128) * p)) ** (D(8192) / D(1305)) * D(100)
            if x < 0:
                value = -value
            rows.append(dict(bits=bits, code=code, linear_at_white_100=str(value)))
print(json.dumps(rows, indent=2))
