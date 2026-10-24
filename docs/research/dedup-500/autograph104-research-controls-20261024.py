import pathlib,tempfile,json,research,swarm_reduce
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-research-owned-") as tmp:
 root=pathlib.Path(tmp);files=[]
 for i in range(120):
  p=root/f'n{i:03d}.md';p.write_text('body');files.append(p)
 plan=research.cmd_plan(root);selected=[p for m in plan['manifests'] for p in m['files']]
 assert plan['gate']['files_total']==120 and len(selected)==100 and len(set(selected))==100
 results.append({'control':'research-plan-is-bounded-sample','total_files':120,'manifest_files':100,'scope':'Explicit max5agents x20files, not full coverage promise; no agents launched'})
 obs=root/'obs.json';ans=root/'answers.json';obs.write_text(json.dumps({'entity_types':['note','contact','project'],'folder_semantics':{'notes':'facts'}}));ans.write_text(json.dumps({'domain_map':{'notes/':'knowledge'},'domains':['personal']}))
 schema=research.cmd_reduce(root,obs,ans);assert schema['domain_inference']['notes/']=='personal'
 errors=swarm_reduce.validate_schema(schema);assert any("missing 'description'" in e for e in errors) and any("missing 'required'" in e for e in errors)
 results.append({'control':'declared-folder-map-overwritten','observed_domain':'personal','requested_domain':'knowledge'})
 results.append({'control':'research-schema-vs-finalize-validator','errors':errors,'scope':'Different generation pipeline contracts; no schema applied'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original research/common/reducer, temporary owned fixture; no user vault or agent calls'},indent=2))
