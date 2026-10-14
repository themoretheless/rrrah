#!/usr/bin/env python3
"""Original whole inventories plus exact permission-only reruns, no failure erasure."""
from pathlib import Path
import hashlib,json,re,subprocess,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
repair_path=D/'dedup-terminal-process-observers-20261014.json';repair=json.loads(repair_path.read_text());assert repair['status']=='passed_pending_audit'and len(repair['results'])==3
assert sha(R/'scripts/run-dedup-terminal-process-observers-20261014.py')==repair['producer_sha256']
corrected={}
for r in repair['results']:
 assert r['returncode']==0 and sha(r['log'])==r['log_sha256']and sha(r['command'][0])==r['executable_sha256']
 results=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored(?:,.*)?)$',Path(r['log']).read_text(),re.M));assert len(results)==r['expected']and all(v=='ok'for v in results.values());corrected[(r['label'],r['target'])]=results
F=Path('/private/tmp/rrrah-terminal-split-gates-20261014');F.mkdir(exist_ok=False);kept=D/'dedup-terminal-split-gates-20261014';kept.mkdir(exist_ok=False);summaries=[]
for label,expected_targets in [('runner',154),('collection_first',3)]:
 retained_path=D/f'dedup-{label}-terminal-binaries-20261014.json';retained=json.loads(retained_path.read_text());P=R/retained['report'];p=json.loads(P.read_text());assert sha(P)==retained['report_sha256']and p['returncode']==101 and sha(p['log'])==p['log_sha256']
 M=Path(p['source_manifest']);assert sha(M)==p['source_manifest_sha256'];m=json.loads(M.read_text());assert all(sha(Path(p['source'])/name)==h for name,h in m['source_sha256'].items())
 sections=list(re.finditer(r'Running (.*?) \(([^\n]+)\)\n(.*?)(?=\n\s*Running |\Z)',Path(p['log']).read_text(),re.S));assert len(sections)==len(retained['rows'])==expected_targets
 rows=[];original_failures=[];total_pass=total_ignore=0
 for index,section in enumerate(sections):
  target,original,body=section.groups();pin=next(r for r in retained['rows']if r['target']==target);assert pin['original']==original and sha(pin['retained'])==pin['sha256']
  inventory=F/f'{label}-{index}.list';result=subprocess.run([pin['retained'],'--list'],cwd=p['source'],capture_output=True,text=True);inventory.write_text(result.stdout+result.stderr);assert result.returncode==0
  names=re.findall(r'^(.+): test$',result.stdout,re.M);statuses=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored(?:,.*)?)$',body,re.M));summary=re.search(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',body);assert summary
  assert len(names)==len(set(names))==len(statuses)==sum(map(int,summary.groups()[1:]))and set(names)==set(statuses)
  assert sum(v=='ok'for v in statuses.values())==int(summary[2])and sum(v=='FAILED'for v in statuses.values())==int(summary[3])
  failed=[n for n,v in statuses.items()if v=='FAILED'];original_failures.extend(dict(target=target,test=n)for n in failed)
  fixes=corrected.get((label,target),{})
  for n in failed:assert fixes.get(n)=='ok',('Unresolved original failure',label,target,n)
  for n,v in fixes.items():assert n in statuses and v=='ok';statuses[n]='ok'
  assert all(v=='ok'or v.startswith('ignored')for v in statuses.values());passed=sum(v=='ok'for v in statuses.values());ignored=sum(v.startswith('ignored')for v in statuses.values());total_pass+=passed;total_ignore+=ignored
  copy=kept/inventory.name;shutil.copyfile(inventory,copy);assert sha(copy)==sha(inventory)
  rows.append(dict(target=target,retained_executable=pin['retained'],executable_sha256=pin['sha256'],inventory=str(copy.relative_to(R)),inventory_sha256=sha(copy),passed=passed,manual_ignored=ignored,original_failed_names=failed,permission_rechecked_names=sorted(fixes)))
 assert len(original_failures)==(6 if label=='runner'else 1)
 O=D/f'dedup-{label}-split-gate-audit-20261014.json';assert not O.exists();report=dict(status='verified_frozen_complete_inventory_in_split_permissioned_runs',label=label,original_report=str(P.relative_to(R)),original_report_sha256=sha(P),source_manifest=str(M),source_manifest_sha256=sha(M),process_observation_repair=str(repair_path.relative_to(R)),process_observation_repair_sha256=sha(repair_path),auditor_sha256=sha(__file__),targets=expected_targets,passed=total_pass,failed=0,ignored=total_ignore,original_sandbox_failures=original_failures,rows=rows,scope='Original full declared test inventory retained, all original passed/ignored results plus exact same binary permission-only process-observation reruns. Original failures are preserved in raw reports, not rewritten as a pristine one-shot pass. Frozen source6runner default+decode154targets or initial collection variant3targets only; not current root16, later v3/v4/v5collection, native learned corpus/default/UI/platform/workspace proof.');O.write_text(json.dumps(report,indent=2)+'\n');summaries.append(dict(label=label,passed=total_pass,ignored=total_ignore,targets=expected_targets))
print(summaries)
