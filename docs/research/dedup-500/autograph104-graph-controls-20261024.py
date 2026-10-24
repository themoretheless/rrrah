import pathlib,tempfile,json
from unittest.mock import patch
import graph
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-graph-owned-") as tmp:
 root=pathlib.Path(tmp);p=root/'a.md';p.write_text('# A')
 with patch.object(pathlib.Path,'read_text',side_effect=OSError('owned failure')):g=graph.build_graph(root,{})
 assert g['stats']['total_files']==0 and g['stats']['health_score']==100.0
 results.append({'control':'all-read-errors-healthy-empty-graph','total_files':0,'health_score':100.0})
 p.unlink();d=root/'folder.md';d.mkdir();(d/'child.md').write_text('# Child');(root/'source.md').write_text('[[folder.md/child]]')
 g=graph.build_graph(root,{})
 assert 'folder/child' in g['nodes'] and g['nodes']['source']['outgoing']==['folder.md/child'] and not g['nodes']['folder/child']['incoming'] and not g['broken_link_list']
 results.append({'control':'md-in-parent-directory-graph-key-mismatch','nodes':list(g['nodes']),'outgoing':g['nodes']['source']['outgoing'],'incoming':g['nodes']['folder/child']['incoming'],'broken_links':len(g['broken_link_list'])})
 assert graph.is_raw_path('daily/a.md',['daily/']) and not graph.is_raw_path('daily-old/a.md',['daily/'])
 results.append({'control':'raw-prefix-directory-boundary-positive','daily':True,'daily-old':False})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Unchanged original graph/common, temporary owned inputs and injected read errors; no user vault'},indent=2))
