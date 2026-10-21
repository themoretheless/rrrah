import json,base64,hashlib
from pathlib import Path
root=Path('.dedup-work/imagedup491-20261021');out=Path(__file__).resolve().parent
records=[]
for line in (root/'go.sum').read_text().splitlines():
    name,version,digest=line.split();assert digest.startswith('h1:');assert len(base64.b64decode(digest[3:],validate=True))==32
    records.append(dict(module=name,version=version,checksum=digest))
assert len({(r['module'],r['version']) for r in records})==len(records)
d=json.loads((root/'grafana/panel.json').read_text());panels=d['panels'];assert len({p['id'] for p in panels})==len(panels)
# Traverse every dashboard node, including layout, styles, mappings and thresholds.
counts={};leaves=[]
def walk(value,path=''):
    counts[type(value).__name__]=counts.get(type(value).__name__,0)+1
    if isinstance(value,dict):
        for k,v in value.items():walk(v,path+'/'+k)
    elif isinstance(value,list):
        for i,v in enumerate(value):walk(v,path+'/'+str(i))
    else:leaves.append((path,value))
walk(d)
queries=[dict(panel=p['title'],expression=t['expr']) for p in panels for t in p.get('targets',[])]
result=dict(lock_records=len(records),lock_modules=len({r['module'] for r in records}),lock_scope='Every line structurally parsed, SHA256-sized h1 digest decoded; not independent dependency source or checksum-server audit.',dashboard_panels=len(panels),dashboard_node_types=counts,dashboard_leaf_count=len(leaves),dashboard_queries=queries,selected_instance_referenced=any('$instance' in q['expression'] or '${instance}' in q['expression'] for q in queries),dashboard_scope='Every JSON node traversed; all query expressions and top-level settings manually inspected; presentation leaf values structurally assessed, not a running Grafana import.')
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
