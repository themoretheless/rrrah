import pathlib,tempfile,json,contextlib,io,swarm_prepare,swarm_reduce
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-batch-owned-") as tmp:
 root=pathlib.Path(tmp);(root/'a.md').write_text('A'*20);(root/'b.md').write_text('B'*20)
 with contextlib.redirect_stdout(io.StringIO()):first=swarm_prepare.prepare(root,budget=5)
 (root/'b.md').unlink()
 with contextlib.redirect_stdout(io.StringIO()):second=swarm_prepare.prepare(root,budget=5)
 manifests=list((root/'.graph/swarm/manifests').glob('*.json'));assert first['total_batches']==2 and second['total_batches']==1 and len(manifests)==2
 results.append({'control':'reprepare-retains-obsolete-manifest','current_batch_count':1,'manifest_files':2})
 c=root/'.graph/swarm/classifications';c.mkdir(exist_ok=True);entry={'path':'a.md','proposed_type':'note'};(c/'a.jsonl').write_text(json.dumps(entry)+'\n'+json.dumps(entry)+'\n');rows=swarm_reduce.read_classifications(root/'.graph/swarm');freq=swarm_reduce.count_frequencies(rows)
 assert freq['total_classified']==2 and freq['type_frequency']['note']==2
 results.append({'control':'duplicate-classification-path-counted-twice','count':2,'distinct_paths':len({r['path'] for r in rows})})
 schema={k:{} for k in swarm_reduce.REQUIRED_SCHEMA_SECTIONS};schema.update(node_types={k:{'description':'x','required':[],'status':[]} for k in ['a','b','c']},decay={'rate':'invalid','floor':[]})
 errors=swarm_reduce.validate_schema(schema);assert errors==[]
 results.append({'control':'schema-type-validation-gap','errors':errors,'accepted_rate':'invalid','accepted_floor':[],'scope':'Validation function only; generated schema not applied'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original batch helpers on owned temporary vault; no model agents launched or user files changed'},indent=2))
