#!/usr/bin/env python3
from pathlib import Path
import hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-exact-windows-tests-strict-v1-20261015.json';d=json.loads(P.read_text());assert d['status']=='passed_pending_independent_audit' and d['returncode']==0 and d['source_unchanged'] and not d['dependency_changes']
F=Path(d['source']).parent
assert sha(R/'scripts/qualify-dedup-exact-windows-tests-strict-v1-20261015.py')==d['producer_sha256']
assert sha(R/'crates/rrrah-dedup/src/exact.rs')==sha(F/'exact.rs')==d['source_sha256']
for name,original in [('controls.rs','exact_large_files.rs'),('integration.rs','exact_files.rs')]:assert sha(R/'crates/rrrah-dedup/tests'/original)==sha(F/name)==d['fixture_pins'][name]
for name,h in d['dependency_pins'].items():assert sha(name)==h
for field in ['harness','artifact','log']:assert sha(d[field])==d[field+'_sha256']
cmd=d['command'];assert '--test' in cmd and cmd[cmd.index('--emit')+1]=='metadata' and cmd[cmd.index('-D')+1]=='warnings' and cmd[cmd.index('--target')+1]=='x86_64-pc-windows-gnu'
O=D/'dedup-exact-windows-tests-strict-v1-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_strict_windows_testcode_metadata_only',producer_report=str(P.relative_to(R)),producer_report_sha256=sha(P),auditor_sha256=sha(__file__),dependency_artifact_count=len(d['dependency_pins']),source_unchanged=True,scope='Current exact module and complete exact_files/exact_large_files testcode typechecked for WindowsGNU with -Dwarnings and real cached dependencies. Source/copies/dependency/harness/log/artifact pins independently checked. No linking, Windows execution, fsutil privilege/filesystem behavior, wholecrate/decode or performance proof.'),indent=2)+'\n');print('Windows exact testcode strict metadata independently verified')
