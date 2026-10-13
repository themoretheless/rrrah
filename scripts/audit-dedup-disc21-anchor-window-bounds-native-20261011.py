#!/usr/bin/env python3
import ast,hashlib,json,math,re
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def windows(points,key,d,radius):
 result=[];size=2*radius+1
 for p in points:
  x=math.floor(p[key][0])-radius;y=math.floor(p[key][1])-radius
  if x<0 or y<0 or x+size>d[0] or y+size>d[1]:continue
  rect=[x,y,size,size]
  if any(x<q[0]+q[2] and q[0]<x+size and y<q[1]+q[3] and q[1]<y+size for q in result):continue
  result.append(rect)
 return result
def bounds(w):
 x=min(r[0] for r in w);y=min(r[1] for r in w)
 return [x,y,max(r[0]+r[2] for r in w)-x,max(r[1]+r[3] for r in w)-y]
def main():
 audit=D/'dedup-disc21-overlap-regions-native-v2-audit-20261011.json';a=json.loads(audit.read_text());assert all(sha(k)==v for k,v in a['pins'].items());p=D/'dedup-disc21-overlap-regions-native-v2-20261011.json';r=json.loads(p.read_text());e=json.loads(r['stdout'])['evidence'];rows=[]
 for call in e['pixel_attempts']:
  source=windows(call['witnesses'],'source',e['original_dimensions'][0],call['window_radius']);target=windows(call['witnesses'],'target',e['original_dimensions'][1],call['window_radius'])
  if min(len(source),len(target))<10:assert call['pixels']=='insufficient_geometry';continue
  text=call['pixels'];sb=ast.literal_eval(re.search(r'source_window_bounds: (\[[^]]+\])',text)[1]);tb=ast.literal_eval(re.search(r'target_window_bounds: (\[[^]]+\])',text)[1]);assert sb==bounds(source) and tb==bounds(target);assert int(re.search(r'forward_anchors: (\d+)',text)[1])==len(target) and int(re.search(r'reverse_anchors: (\d+)',text)[1])==len(source);rows.append({'bank':call['bank'],'radius':call['window_radius'],'source_window_bounds':sb,'target_window_bounds':tb})
 assert len(rows)==3;out=D/'dedup-disc21-anchor-window-bounds-native-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_actual_cpu_selected_window_bounds','pins':{str(x):sha(x) for x in [audit,p,Path(__file__)]},'rows':rows,'scope':'Independent integer window-selection replay from published native witnesses, including asymmetric greedy selection. Validates new fields on actual CPU negative-pixel results; does not change acceptance or qualify the dark copy.'},indent=2)+'\n');print('verified native window bounds',len(rows))
if __name__=='__main__':main()
