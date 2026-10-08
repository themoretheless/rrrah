"""Exact distance>2 admission using conservative spatial buckets, no changed threshold."""
import math

def verify_distinct_points(points):
    for axis in [0,1]:
        cells={}
        for point in points:
            x,y=point[axis];key=(math.floor(x/2),math.floor(y/2))
            # +/-2 also covers floating-point subtraction at a bucket boundary.
            for dx in range(-2,3):
                for dy in range(-2,3):
                    for px,py in cells.get((key[0]+dx,key[1]+dy),()):
                        assert math.hypot(x-px,y-py)>2
            cells.setdefault(key,[]).append((x,y))
