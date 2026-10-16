#!/usr/bin/env python3
"""Independent GEOS coverage audit of a source-hash-guarded AI diagnostic.
Needs Shapely2.1.2; input inventory contains original PDF path operators.
This is a bounded diagnostic, never a production AI qualification gate.
"""
import argparse
import hashlib
import json
import pathlib
import re
import shapely
from shapely.geometry import LineString, Polygon, box
from shapely.ops import polygonize, unary_union

def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()

def curve(points, subdivisions):
    for i in range(1, subdivisions + 1):
        t = i / subdivisions
        weights = [(1-t)**3, 3*t*(1-t)**2, 3*t*t*(1-t), t**3]
        yield tuple(sum(w*p[d] for w,p in zip(weights,points)) for d in (0,1))

def contours(path, subdivisions):
    rings, current, point = [], [], None
    for entry in path:
        op, points = entry['op'], entry['points']
        if op in ('m','re'):
            if current: rings.append(current)
            current = points[:]; point = points[-1]
        elif op == 'l':
            current += points; point = points[-1]
        elif op in ('c','v','y'):
            controls = [point] + (points if op == 'c' else [point]+points if op == 'v' else points+[points[-1]])
            assert len(controls) == 4
            current += list(curve(controls, subdivisions)); point = controls[-1]
        elif op == 'h':
            if current: rings.append(current)
            current, point = [], None
        else:
            raise ValueError('unsupported original path operator: '+op)
    if current: rings.append(current)
    return rings

def winding(ring, point):
    count = 0
    for a,b in zip(ring,ring[1:]+ring[:1]):
        cross=(b[0]-a[0])*(point[1]-a[1])-(point[0]-a[0])*(b[1]-a[1])
        if a[1] <= point[1] < b[1] and cross > 0: count += 1
        if b[1] <= point[1] < a[1] and cross < 0: count -= 1
    return count

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report',type=pathlib.Path,required=True)
    parser.add_argument('--output',type=pathlib.Path,required=True)
    args=parser.parse_args()
    report=json.loads(args.report.read_text())
    for item in report['historical_evidence_source_audit']:
        if item['path'].startswith('vendor/'):
            assert digest(item['path']) == item['current_sha256'], 'source changed since capture'
    assert digest(report['current_code_finding']['source']) == report['current_code_finding']['source_sha256']
    inventory_path=pathlib.Path('docs/research/ai-original-path-inventory-2026-10-13.json')
    inventory=json.loads(inventory_path.read_text())
    assert digest(report['corpus']) == inventory['source_sha256']
    log=args.report.parent/'diagnostic.log'; text=log.read_text()
    assert 'REAL_AI_SPATIAL_FINAL_CREDIT=0' in text and 'completed=true' in text
    assert report['point'] == [149,56]
    triangles=[]
    for row in re.findall(r'REAL_AI_CAPTURED_MESH_TRIANGLE event=2670 index=\d+ points=(\[[^\n]+\])',text):
        values=json.loads(row); assert len(values)==6
        triangle=Polygon([(values[i],values[i+1]) for i in (0,2,4)])
        assert triangle.is_valid
        triangles.append(triangle)
    assert triangles
    mesh=unary_union(triangles)
    paths=[p for p in inventory['paths'] if p['operator_index']==1181 and p['stream']=='page0']
    assert len(paths)==1 and paths[0]['clip_rule']=='W'
    rows=[]
    for subdivisions in [512,2048,8192]:
        rings=contours(paths[0]['path'],subdivisions)
        lines=unary_union([LineString(ring+[ring[0]]) for ring in rings if len(ring)>2])
        faces=[]
        for face in polygonize(lines):
            point=face.representative_point()
            if sum(winding(ring,(point.x,point.y)) for ring in rings): faces.append(face)
        clip=unary_union(faces).intersection(box(149,56,150,57))
        union=mesh.intersection(clip)
        rows.append({'subdivisions':subdivisions,'original_clip_area':clip.area,
            'triangle_union_inside_original_clip':union.area,'missing_support_area':clip.difference(mesh).area,
            'union_vs_composited_alpha':abs(union.area-report['current_result']['composited_alpha'])})
    result={'scope':'current original VectorApple source clip and captured mesh at149,56',
        'triangles':len(triangles),'shapely':shapely.__version__,'geos':shapely.geos_version_string,
        'rows':rows,'last_clip_area_change':abs(rows[-1]['original_clip_area']-rows[-2]['original_clip_area']),
        'sha256':{str(p):digest(p) for p in [args.report,log,inventory_path,pathlib.Path(__file__)]},
        'limitations':['one pixel/event; not whole page fidelity','original curves numerically subdivided, not interval-certified',
                       'triangle generation is native; only union/intersection independently evaluated','no independent color or mask oracle']}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))

if __name__=='__main__': main()
