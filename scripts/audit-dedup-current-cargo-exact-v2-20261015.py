#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,subprocess,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-current-cargo-exact-v2-20261015.json';d=json.loads(P.read_text());assert d['status']=='passed_pending_independent_audit' and d['returncode']==0 and not d['source_changes'];assert sha(R/'scripts/qualify-dedup-current-cargo-exact-v2-20261015.py')==d['producer_sha256'];assert sha(d['log'])==d['log_sha256']
for p,h in d['source_pins'].items():assert sha(R/p)==h
text=Path(d['log']).read_text();matches=list(re.finditer(r'Running (.+) \(([^)]+)\)',text));assert len(matches)==3;rows=[];Q=Path("/private/tmp/rrrah-current-cargo-exact-v2-qualified-20261015");Q.mkdir(exist_ok=False)
for i,m in enumerate(matches):
 chunk=text[m.end():matches[i+1].start()if i+1<len(matches)else len(text)];original=R/m[2];exe=Q/original.name;shutil.copyfile(original,exe);exe.chmod(0o755);before=sha(exe);assert before==sha(original);run=subprocess.run([str(exe),'--list'],capture_output=True,text=True);assert run.returncode==0;names=set(re.findall(r'^(.+): test$',run.stdout,re.M));results=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored.*?)$',chunk,re.M));summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',chunk);assert summary and summary[2]=='0'and names==set(results);passed=int(summary[1]);ignored=int(summary[3]);assert sum(v=='ok'for v in results.values())==passed and sum(v.startswith('ignored')for v in results.values())==ignored and sha(exe)==before;rows.append(dict(target=m[1],executable=str(exe),executable_sha256=before,passed=passed,ignored=ignored,inventory=sorted(names)))
assert [(r['passed'],r['ignored'])for r in rows]==[(47,2),(29,0),(3,0)]
assert any(name.endswith('every_byte_pattern_in_low_and_high_bits_matches_full_scan_at_every_radius')for name in rows[0]['inventory'])
O=D/'dedup-current-cargo-exact-v2-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_current_no_feature_cargo_exact_three_targets',producer_report=str(P.relative_to(R)),producer_report_sha256=sha(P),auditor_sha256=sha(__file__),passed=79,failed=0,ignored=2,rows=rows,scope=d['scope']+' All3complete executed inventories match fresh retained binary listings.79pass2manualbenchignored. No allintegrationtargets/fullworkspace/Windows/decode or performance proof.'),indent=2)+'\n');print('Current Cargo79tests verified,2manual benchmarks ignored')
