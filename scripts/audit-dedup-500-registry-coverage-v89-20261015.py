"""Inventory all500 registry entries and independently verify their stored evidence pins."""
from pathlib import Path
import json,hashlib,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
previous_rows=json.loads((D/'registry-coverage-audit-v64-20261015.json').read_text())['rows']
assert len(previous_rows)==500
rows=[];errors=[];pins=0;repositories=set()
for index in range(1,501):
 p=D/'reviews'/f'{index:03}.json';d=json.loads(p.read_text());assert d['index']==index and d['repository'] not in repositories and re.fullmatch('[0-9a-f]{40}',d['commit']);repositories.add(d['repository'])
 evidence=D/'evidence'/d['repository'].replace('/','--')
 for key in ['available_source_evidence','available_readme_license_evidence']:
  for item in d.get(key,[]):
   f=evidence/item['local'];pins+=1
   if not f.exists() or h(f)!=item['sha256']:errors.append({'index':index,'file':str(f.relative_to(R)),'problem':'missing_or_hash_mismatch'})
 refs=[]
 for key in ['complete_source_review','complete_runtime_audit']:
  if d.get(key):refs.append(d[key])
 for item in d.get('incremental_reviews',[])+d.get('incremental_source_reviews',[]):
  if isinstance(item,str):refs.append(item)
  elif isinstance(item,dict):refs.extend(item[k] for k in ['audit','controls'] if item.get(k))
 refs.extend(d.get('selected_runtime_reviews', []))
 for item in d.get('linked_review_artifacts', []):
  if isinstance(item,str): refs.append(item)
  elif isinstance(item,dict) and item.get('review_artifact'): refs.append(item['review_artifact'])
 refs.extend(item['path'] for item in previous_rows[index-1]['detailed_review_references'])
 resolved=[]
 for ref in dict.fromkeys(refs):
  candidates=[R/ref,D/ref,R/'docs/research'/ref];found=next((x for x in candidates if x.is_file()),None)
  if found is None:errors.append({'index':index,'reference':ref,'problem':'unresolved_review_reference'})
  else:
   content=json.loads(found.read_text());resolved.append({'path':str(found.relative_to(R)),'sha256':h(found),'status':content.get('status'),'whole_repo_complete':content.get('whole_repo_complete',content.get('whole_repository_complete')),'whole_implementation_complete':content.get('whole_implementation_complete',content.get('whole_implementation_read'))})
 rows.append({'index':index,'repository':d['repository'],'commit':d['commit'],'registry_sha256':h(p),'decision':d['decision'],'declared_scope':d['review_scope'],'declared_full_repository_read':any(d.get(k,False) for k in ['full_repository_read','full_repository_review_complete','whole_repo_complete','whole_repository_complete']),'detailed_review_references':resolved})
P=R/'docs/research/dedup-500/registry-coverage-audit-v89-20261015.json';assert not P.exists();P.write_text(json.dumps({'status':'verified_registry_inventory' if not errors else 'registry_inventory_with_evidence_errors','entries':len(rows),'unique_repositories':len(repositories),'stored_evidence_pins_checked':pins,'pin_or_reference_errors':errors,'entries_with_detailed_references':sum(bool(r['detailed_review_references']) for r in rows),'entries_declaring_full_repository_read':sum(r['declared_full_repository_read'] for r in rows),'rows':rows,'limits':['Registry inventory and byte-pin verification, not new source reading or runtime qualification','Detailed-reference count does not prove full repository review','Entries without explicit references may have older separate evidence; this report does not infer completion or absence of review','All500 full-source review remains unproven']},indent=2)+'\n');print(json.dumps({'entries':len(rows),'pins':pins,'errors':errors[:10],'error_count':len(errors),'detailed':sum(bool(r['detailed_review_references']) for r in rows)}))
