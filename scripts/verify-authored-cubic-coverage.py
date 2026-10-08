#!/usr/bin/env python3
"""Independent Decimal area oracle for the authored monotone cubic fixture."""
import argparse
from decimal import Decimal, localcontext, ROUND_HALF_UP
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    data = args.fixture.read_bytes()
    case = json.loads(data)
    with localcontext() as ctx:
        ctx.prec = 60
        points = [[Decimal(str(v)) for v in p] for p in case["independent_cubic_area"]["control_points"]]
        assert len(points) == 4
        assert all(points[i][0] >= points[i+1][0] for i in range(3))
        assert all(points[i][1] <= points[i+1][1] for i in range(3))
        def coefficients(v):
            return [v[0],3*(v[1]-v[0]),3*(v[2]-2*v[1]+v[0]),v[3]-3*v[2]+3*v[1]-v[0]]
        x = coefficients([p[0] for p in points])
        y = coefficients([p[1] for p in points])
        px, py = map(Decimal, case["pixel"])
        y[0] -= py
        def evaluate(poly, t):
            result = Decimal(0)
            for v in reversed(poly):
                result = result*t+v
            return result
        def parameter(target):
            assert points[-1][0] < target < points[0][0]
            a,b = Decimal(0),Decimal(1)
            for _ in range(200):
                middle = (a+b)/2
                if evaluate(x,middle) > target:
                    a = middle
                else:
                    b = middle
            return (a+b)/2
        lo,hi = parameter(px+1),parameter(px)
        assert 0 < evaluate(y,lo) < evaluate(y,hi) < 1
        product = [Decimal(0)]*6
        for i,a in enumerate(y):
            for j in range(3):
                product[i+j] += a*(j+1)*x[j+1]
        primitive = [Decimal(0)]+[v/(i+1) for i,v in enumerate(product)]
        area = -(evaluate(primitive,hi)-evaluate(primitive,lo))
        alpha = int((area*255).to_integral_value(rounding=ROUND_HALF_UP))
        assert abs(area-Decimal(str(case["independent_cubic_area"]["coverage"]))) < Decimal("1e-10")
        report = {"scope":"Own cubic pixel coverage, independent high precision area oracle",
                  "source_sha256":hashlib.sha256(data).hexdigest(),"decimal_digits":60,
                  "root_bisection_steps":200,"coverage":str(area),"rounded_alpha":alpha,
                  "whole_alpha":case["whole_coverage"],"tile_alpha":case["tile_coverage"],
                  "whole_exact":case["whole_coverage"]==alpha,"tile_exact":case["tile_coverage"]==alpha,
                  "limitations":"One monotone cubic crossing this pixel; not a general path/clip/stroke oracle."}
    args.report.write_text(json.dumps(report,indent=2)+"\n")
    raise SystemExit(0 if report["whole_exact"] and report["tile_exact"] else 1)


if __name__ == "__main__":
    main()
