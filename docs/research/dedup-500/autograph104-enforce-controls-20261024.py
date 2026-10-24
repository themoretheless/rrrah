import pathlib,tempfile,json
import enforce
results=[]
schema={'node_types':{'note':{'required':['description','tags'],'status':['active']}}}
base='---\ntype: note\nstatus: active\ndescription: A valid description\ntags: [one]\n'
with tempfile.TemporaryDirectory(prefix="autograph104-enforce-owned-") as tmp:
 root=pathlib.Path(tmp);p=root/'a.md'
 p.write_text(base+'priority: 1\n---\nBody')
 try:enforce.enforce(root,schema)
 except AttributeError as e:results.append({'control':'numeric-priority-crashes-enforce','exception':type(e).__name__})
 else:raise AssertionError('expected observed AttributeError')
 p.write_text(base.replace('tags: [one]','tags: invalid-scalar')+'---\nBody')
 stats,_=enforce.enforce(root,schema,apply=True);issues=enforce.validate_cards(root,schema)
 assert stats['needs_review']==0 and any(e['issue']=='tags must be a list of strings' for e in issues)
 results.append({'control':'enforce-vs-validator-tags-shape','enforce_needs_review':0,'validator_issue':'tags must be a list of strings','wrote_inferred_fields':stats['fixed']==1})
 p.write_text(base+'---\nBody')
 with p.open('ab') as f:f.truncate(enforce.ENFORCE_MAX_FILE_BYTES+1)
 stats,groups=enforce.enforce(root,schema)
 assert stats['skipped_oversize']==1 and groups=={}
 results.append({'control':'oversize-excluded-positive','skipped':1,'duplicate_groups':len(groups)})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Unchanged enforce/common, temporary owned cards only; sparse oversized fixture cleaned; no user vault'},indent=2))
