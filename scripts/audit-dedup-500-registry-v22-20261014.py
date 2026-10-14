#!/usr/bin/env python3
import json,hashlib
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def references(value, allowed=False):
 if isinstance(value,str):return [value] if allowed and value.endswith('.json') else []
 found=[]
 if isinstance(value,dict):
  for k,v in value.items():found+=references(v, 'audit' in k or 'review' in k)
 elif isinstance(value,list):
  for v in value:found+=references(v,allowed)
 return found
rows=[];errors=[];pin_count=0
for p in sorted((D/'reviews').glob('*.json')):
 d=json.loads(p.read_text());repo=d['repository'];evidence=D/'evidence'/repo.replace('/','--');stored=0
 for key in ['available_source_evidence','available_readme_license_evidence']:
  for item in d.get(key,[]):
   local=evidence/item['local'];expected=item['sha256']
   if not local.is_file() or sha(local)!=expected:errors.append(dict(index=d['index'],kind='stored_evidence_pin',path=str(local)))
   stored+=1;pin_count+=1
 refs=[]
 for name in sorted(set(references(d))):
  local=R/name if name.startswith('docs/') else D/name
  if not local.is_file():errors.append(dict(index=d['index'],kind='review_reference_missing',path=str(local)));continue
  content=json.loads(local.read_text());refs.append(dict(path=str(local.relative_to(R)),sha256=sha(local),status=content.get('status'),whole_repo_complete=content.get('whole_repo_complete'),whole_implementation_complete=content.get('whole_implementation_complete')))
 flags={k:d[k] for k in ['full_repository_review_complete','full_repository_read','whole_repo_complete'] if k in d};conflict=any(v is True for v in flags.values()) and any(v is False for v in flags.values())
 rows.append(dict(index=d['index'],repository=repo,commit=d['commit'],registry_sha256=sha(p),declared_scope=d['review_scope'],stored_evidence_pins=stored,detailed_review_references=refs,completion_flags=flags,conflicting_completion_flags=conflict))
assert len(rows)==len({r['repository'] for r in rows})==500 and sorted(r['index'] for r in rows)==list(range(1,501))
out=D/'registry-coverage-audit-v22-20261014.json';assert not out.exists();report=dict(status='verified_registry_inventory_not500_complete_source_reviews' if not errors else 'registry_inventory_errors',entries=500,unique_repositories=500,stored_evidence_pins_checked=pin_count,pin_or_reference_errors=errors,entries_with_detailed_references=sum(bool(r['detailed_review_references']) for r in rows),entries_declaring_any_full_repository_flag=sum(any(v is True for v in r['completion_flags'].values()) for r in rows),conflicting_completion_flag_indices=[r['index'] for r in rows if r['conflicting_completion_flags']],rows=rows,limits='Registry files, stored excerpt hashes and detailed reference existence/content pins verified. Flags and review statuses are declarations, not proof of full source/runtime/platform coverage. Older full-repository and newer selected-review scopes can conflict; keep explicit rather than infer completion. V22 adds original463 MediaDeduplicationTool complete six-file source review and 18 synthetic original-module controls with progress-only adapter; actual MJPG videos included. Full original CLI and Python3.9 environment not qualified. No implementation adoption. Prior artifacts preserved. Supplemental SSCD research is outside the original500 and does not replace any entry.');out.write_text(json.dumps(report,indent=2)+'\n');print({k:v for k,v in report.items() if k not in ['rows','limits']})
