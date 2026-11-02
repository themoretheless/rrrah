import ast,bisect,json,logging,pathlib,typing,types
b=pathlib.Path(__file__).parent;s=b.parent/'evidence/WSH032--image-deduplicate-cluster-webui/03-deduplicate_fn.py';tree=ast.parse(s.read_text());names={'confirm_cluster','split_str_by_comma','all_select','get_files_to_remove'};nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names]
for n in nodes:n.decorator_list=[]
calls=[]
ns={'List':typing.List,'Dict':typing.Dict,'Tuple':typing.Tuple,'Optional':typing.Optional,'logging':logging,'bisect':bisect,'gr':types.SimpleNamespace(update=lambda **kw:kw),'toml':types.SimpleNamespace(loads=json.loads,dumps=json.dumps),'CLUSTER_DIR_PREFIX':'cluster','PROCESS_CLUSTERS_METHOD_CHOICES':['更名选中图片（推荐全部选择）','移动选中图片（推荐此方式）','删除选中图片（推荐自动选择）'],'operate_images_file':lambda **kw:calls.append(kw)}
exec(compile(ast.Module(body=nodes,type_ignores=[]),str(s),'exec'),ns)
results=[]
for label,selection,expected in [('negative-index',{'-1':[-1]},[['D']]),('repeated-index',{'0':[0,0]},[['A','A']])]:
 ns.update(cluster_list=[['A','B'],['C','D']],confirmed_images_dir='/owned/synthetic')
 ns['confirm_cluster'](json.dumps(selection),2,'','');assert calls[-1]['clustered_images_list']==expected
 results.append({'case':label,'planned_groups':calls[-1]['clustered_images_list'],'operation':calls[-1]['operation']})
ns['cluster_list']=[['A','B'],['C','D']];all_selection=json.loads(ns['all_select']());assert all_selection=={'0':[0,1],'1':[0,1]}
results.append({'case':'all-select','selection':all_selection,'scope':'Explicit select-all feature, not auto-selection defect; no per-group keeper guard in confirm_cluster.'})
(b/'selection-control-results.json').write_text(json.dumps({'scope':'AST-extracted original functions, decorators removed; JSON substitutes only TOML serialization, gr.update returns dictionary, filesystem operation is recording stub. No files touched, no full UI/model runtime.','cases':results},indent=2)+'\n')
print('3 selection controls passed;0 filesystem operations')
