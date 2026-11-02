#!/usr/bin/env python3
"""Independent exact-rational check of convex represented regions in a native log.

This proves recorded polygon intersections only, not source-surface accuracy.
"""
import argparse
import ast
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path
import re


def cross(a, b, c):
    return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])


def area(poly):
    return sum((a[0]*b[1]-b[0]*a[1] for a, b in zip(poly, poly[1:]+poly[:1])), F(0))/2


def clip(poly, a, b):
    if not poly:
        return []
    result = []
    previous = poly[-1]
    f = cross(a, b, previous)
    for current in poly:
        g = cross(a, b, current)
        if (f >= 0) != (g >= 0):
            t = f/(f-g)
            result.append(tuple(previous[k]+t*(current[k]-previous[k]) for k in range(2)))
        if g >= 0:
            result.append(current)
        previous, f = current, g
    return result


def hull(poly):
    points = sorted(set(poly))
    lower, upper = [], []
    for p in points:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(points):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def simple(poly):
    for i, (a, b) in enumerate(zip(poly, poly[1:]+poly[:1])):
        for j, (c, d) in enumerate(zip(poly, poly[1:]+poly[:1])):
            if j <= i or j == i+1 or (i == 0 and j == len(poly)-1):
                continue
            if any(max(a[k], b[k]) < min(c[k], d[k]) or max(c[k], d[k]) < min(a[k], b[k]) for k in range(2)):
                continue
            if cross(a,b,c)*cross(a,b,d) <= 0 and cross(c,d,a)*cross(c,d,b) <= 0:
                return False
    return True


def verify(data, use_hulls=False):
    regions = {}
    for m in re.finditer(r'^region (\d+) patch=(\d+) points=(.*?) tags=', data.decode(), re.M):
        index = int(m[1])
        if index in regions:
            raise ValueError('multiple snapshots in log; select one first')
        points = ast.literal_eval(m[3])
        if len(points) < 3 or any(len(p) != 2 for p in points):
            raise ValueError('invalid polygon')
        poly = [tuple(F(v) for v in p) for p in points]
        if area(poly) < 0:
            poly.reverse()
        regions[index] = poly
    if not regions:
        raise ValueError('no recorded regions')
    nonconvex = [i for i, p in regions.items() if any(cross(p[k-2], p[k-1], p[k]) < 0 for k in range(len(p)))]
    convex = {i: hull(p) for i, p in regions.items()} if use_hulls else {i: p for i, p in regions.items() if i not in nonconvex}
    defects = {i: area(convex[i])-area(p) for i, p in regions.items() if i in convex}
    simplicity = {i: simple(p) for i, p in regions.items()} if use_hulls else {}
    overlaps = []
    for i, p in convex.items():
        for j, q in convex.items():
            if j >= i or any(max(v[k] for v in p) <= min(v[k] for v in q) or max(v[k] for v in q) <= min(v[k] for v in p) for k in range(2)):
                continue
            intersection = p
            for a, b in zip(q, q[1:]+q[:1]):
                intersection = clip(intersection, a, b)
            value = area(intersection)
            if value > F(1, 10**10):
                entry = dict(regions=[i, j], area=float(value), exact_numerator=str(value.numerator), exact_denominator=str(value.denominator))
                if use_hulls:
                    entry['hull_area_defects'] = [float(defects[i]),float(defects[j])]
                    if simplicity[i] and simplicity[j] and defects[i]>=0 and defects[j]>=0:
                        lower = value-defects[i]-defects[j]
                        entry['original_overlap_lower_bound'] = float(lower)
                        entry['lower_bound_numerator'] = str(lower.numerator)
                        entry['lower_bound_denominator'] = str(lower.denominator)
                overlaps.append(entry)
    return dict(scope=('Exact hull intersections; lower bounds for simple original polygons subtract exact hull-area defects.' if use_hulls else 'Exact Fraction clipping of convex represented polygons; nonconvex inputs excluded.') + ' No source approximation or whole-file claim.',
                log_sha256=hashlib.sha256(data).hexdigest(), recorded_regions=len(regions),
                nonconvex_regions=nonconvex, overlaps_above_1e_10=overlaps)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log', type=Path)
    parser.add_argument('--hulls', action='store_true', help='Bound overlap of simple nonconvex originals via exact hull-area defects')
    args = parser.parse_args()
    print(json.dumps(verify(args.log.read_bytes(),args.hulls), indent=2))
