#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-collection-fused-preverify-v1-20261015.json';d=json.loads(P.read_text());assert d['status']=='passed_pending_independent_audit';assert d['passed']==54 and d['failed']==d['ignored']==0
F=Path(d['source']);M=D/'dedup-collection-fused-preverify-v1-source-manifest-20261015.json';m=json.loads(M.read_text());assert sha(F/'exact.rs')==sha(R/m['candidate_source'])==m['candidate_sha256'];base=R/'crates/rrrah-dedup/src/exact.rs';assert sha(base)==m['baseline_source_sha256'];assert (F/'exact.rs').read_bytes().startswith(base.read_bytes()+b'\n')
for name,source in [('controls.rs','exact_large_files.rs'),('integration.rs','exact_files.rs')]:assert sha(F/name)==sha(R/'crates/rrrah-dedup/tests'/source)
for name,h in d['dependency_pins'].items():assert sha(name)==h
for name,h in d['output_pins'].items():assert sha(name)==h
assert sha(d['executable'])==d['executable_sha256'] and sha(F/'harness.rs')==d['harness_sha256'];assert sha(R/'scripts/qualify-dedup-collection-fused-preverify-v1-20261015.py')==d['producer_sha256'];assert all(d[k+'_returncode']==0 for k in ['build','inventory','tests'])
names=set(re.findall(r'^(.+): test$',(F/'inventory.log').read_text(),re.M));results=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored.*?)$',(F/'tests.log').read_text(),re.M));assert names==set(results) and len(names)==54 and set(results.values())=={'ok'}
B=D/'dedup-root-exact-windows-fix-audit-20261015.json';baseline=json.loads(B.read_text());assert set(baseline['complete_inventory'])<names;extra=names-set(baseline['complete_inventory']);assert len(extra)==2 and all('fused_preverify_candidate_controls::'in n for n in extra)
O=D/'dedup-collection-fused-preverify-v1-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified54candidate_exact_tests_not_collection_adoption',producer_report=str(P.relative_to(R)),producer_sha256=sha(P),auditor_sha256=sha(__file__),source_manifest_sha256=sha(M),baseline_audit_sha256=sha(B),passed=54,failed=0,ignored=0,extra_controls=sorted(extra),scope='All52previous exact tests and2candidate controls passed; current original/candidate/copies/dependency/executable/output pins verified independently. Producer v1source_pins indentation recorded only last baseline file; independent audit explicitly checks allthree source copies and candidate-prefix binding. No actual collection integration, performance, Windows execution or metadata-preserving rewrite proof.'),indent=2)+'\n');print('54candidate exact tests independently verified')
