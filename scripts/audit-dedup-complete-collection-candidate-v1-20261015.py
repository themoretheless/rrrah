#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,re,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-complete-collection-candidate-v1-20261015.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='terminal_pending_inventory_audit' and d['returncode']==0
assert sha(R/'scripts/qualify-dedup-complete-collection-candidate-v1-20261015.py')==d['producer_sha256']
M=Path(d['source_manifest']);assert sha(M)==d['source_manifest_sha256'];m=json.loads(M.read_text());overlay_path=Path(d['overlay_manifest']);assert sha(overlay_path)==d['overlay_manifest_sha256'];overlay=json.loads(overlay_path.read_text());expected={r['path']:r['sha256']for r in m['source_files']};expected.update({r['path']:r['candidate_sha256']for r in overlay['changed']});assert sha(d['baseline_audit'])==d['baseline_audit_sha256']
for row in m['source_files']:assert sha(Path(d['source'])/row['path'])==expected[row['path']]
assert sha(d['cargo_log'])==d['cargo_log_sha256'];body=Path(d['cargo_log']).read_text();headers=list(re.finditer(r'^\s*Running .+ \((/[^\n]+)\)$',body,re.M));assert headers
retained={r['original']:r for r in d['retained_binaries']};assert set(retained)=={h[1]for h in headers};out=[]
for i,h in enumerate(headers):
 row=retained[h[1]];assert sha(row['copy'])==row['sha256']and sha(row['inventory'])==row['inventory_sha256']and row['inventory_returncode']==0
 inv=Path(row['inventory']).read_text();names=set(re.findall(r'^(.+): test$',inv,re.M));assert not re.findall(r'^(.+): benchmark$',inv,re.M)
 fresh=subprocess.run([row['copy'],'--list'],capture_output=True,text=True);assert fresh.returncode==0 and set(re.findall(r'^(.+): test$',fresh.stdout,re.M))==names
 section=body[h.end():headers[i+1].start()if i+1<len(headers)else len(body)];results=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored[^\n]*)$',section,re.M));assert set(results)==names,(h[1],names-set(results),set(results)-names)
 summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',section);assert summary and summary[2]=='0'and summary[4]=='0'and summary[5]=='0'
 passed=sum(v=='ok'for v in results.values());ignored=sum(v.startswith('ignored')for v in results.values());assert passed==int(summary[1])and ignored==int(summary[3])and len(names)==passed+ignored
 out.append(dict(binary=row['copy'],sha256=row['sha256'],passed=passed,ignored=ignored,complete_inventory=sorted(names),ignored_tests={k:v for k,v in results.items()if v.startswith('ignored')}))
baseline=json.loads(Path(d['baseline_audit']).read_text());old_names={name for binary in baseline['binaries']for name in binary['complete_inventory']};new_names={name for binary in out for name in binary['complete_inventory']};assert old_names.issubset(new_names)
def target_name(row):
 name=Path(row['binary']).name
 assert name.startswith('binary-')
 return name.split('-',2)[2].rsplit('-',1)[0]
old_targets={target_name(row):set(row['complete_inventory'])for row in baseline['binaries']};new_targets={target_name(row):set(row['complete_inventory'])for row in out};assert len(old_targets)==len(baseline['binaries'])and len(new_targets)==len(out);assert set(old_targets)==set(new_targets);assert all(names.issubset(new_targets[target])for target,names in old_targets.items())
assert any(name.endswith('actual_collection_benchmark::complete_guarded_engine')for name in new_names)
O=D/'dedup-complete-collection-candidate-v1-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_complete_executed_inventory_for_both_packages',producer_report=str(P),producer_report_sha256=sha(P),auditor_sha256=sha(__file__),source_manifest_sha256=sha(M),binaries=out,passed=sum(r['passed']for r in out),ignored=sum(r['ignored']for r in out),failed=0,scope=d['scope']+' Every named lib/integration test from each fresh retained binary inventory matched executed or explicit ignored results, no filtered/measured cases. Passing tests may contain optional external-corpus earlyreturns: no claim of native458/corpus execution merely from passing harness results.'),indent=2)+'\n');print('Verified',len(out),'complete binaries;',sum(r['passed']for r in out),'passed;',sum(r['ignored']for r in out),'ignored')
