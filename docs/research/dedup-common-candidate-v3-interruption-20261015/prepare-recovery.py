from pathlib import Path
import json,hashlib,shutil,datetime,re,fcntl,os
R=Path.cwd();D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();old=D/'dedup-common-worker-fixed-candidate-gate-v3-20261015.json';data=json.loads(old.read_text());assert data['status']=='running' and data['producer_pid']==96607 and data['child_pid']==96610
for pid in [96607,96610]:
 try:os.kill(pid,0)
 except ProcessLookupError:pass
 else:raise AssertionError(('old process exists',pid))
E=D/'dedup-common-candidate-v3-interruption-20261015';assert not E.exists();E.mkdir();log=Path('/private/tmp/rrrah-common-worker-fixed-candidate-gate-v3-20261015/cargo.log');shutil.copy2(old,E/'original-producer-report.json');shutil.copy2(log,E/'cargo.log');assert h(old)==h(E/'original-producer-report.json') and h(log)==h(E/'cargo.log');headers=re.findall(r'^\s*Running .+ \((/[^\n]+)\)$',log.read_text(),re.M);assert not headers
manifest=Path(data['manifest']);assert h(manifest)==data['manifest_sha256'];prepared=json.loads(manifest.read_text());snap=next(s for s in prepared['variants'] if s['variant']=='candidate');assert len(snap['source_files'])==3378
for p in snap['source_files']:assert h(Path(snap['source'])/p['path'])==p['sha256']
lockpath=Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug/.cargo-lock');lock=lockpath.open('rb');fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB);fcntl.flock(lock.fileno(),fcntl.LOCK_UN);lock.close()
report=dict(status='verified_stopped_incomplete_candidate_v3',observed_at_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),missing_tool_session=16305,missing_processes=[96607,96610],old_report_status='running_stale',original_report=str(old.relative_to(R)),original_report_sha256=h(old),archived_report=str((E/'original-producer-report.json').relative_to(R)),archived_report_sha256=h(E/'original-producer-report.json'),original_log=str(log),original_log_sha256=h(log),archived_log=str((E/'cargo.log').relative_to(R)),archived_log_sha256=h(E/'cargo.log'),manifest_sha256=h(manifest),source_files_verified=3378,executed_target_headers=0,target_cargo_lock_acquired=True,before_resume_free_bytes=shutil.disk_usage('/private/tmp').free,scope='Sessionmissing plus nativepsconfirmedno producer/Cargo/descendants and kill0missingguard; no terminalstatus/returncode or completeinventory available. Reasonunknown; no successfulgate/performance proof. Preserveoriginalreport/log and source3378, resumecompleteunfilteredCargo withretainedbuildcache undernewv4report; not repeatbecauseobservationtimeout.')
(E/'result.json').write_text(json.dumps(report,indent=2)+'\n')
p=R/'scripts/qualify-dedup-common-worker-fixed-candidate-v3-20261015.py';s=p.read_text().replace('fixed-{variant}-gate-v3-','fixed-{variant}-gate-v4-');needle="data = dict(status='starting', variant=variant, producer_pid=os.getpid(),";insert="""interruption_path = directory / 'dedup-common-candidate-v3-interruption-20261015/result.json'
interruption = json.loads(interruption_path.read_text())
assert interruption['status'] == 'verified_stopped_incomplete_candidate_v3'
assert interruption['manifest_sha256'] == sha(manifest)
assert sha(root / interruption['original_report']) == interruption['original_report_sha256']
assert sha(interruption['original_log']) == interruption['original_log_sha256']
for prior_pid in interruption['missing_processes']:
    try:
        os.kill(prior_pid, 0)
    except ProcessLookupError:
        pass
    else:
        raise AssertionError('previous producer/Cargo PID exists; recheck ownership before resume')

""";assert needle in s;s=s.replace(needle,insert+needle);s=s.replace("environment_overrides=dict(TMPDIR=str(owned_tmp), CARGO_INCREMENTAL='0'),","environment_overrides=dict(TMPDIR=str(owned_tmp), CARGO_INCREMENTAL='0'),\n            interruption_sha256=sha(interruption_path),");s=s.replace("Candidate incremental codegen disabled only", "V4resumesafterverifiedstoppedincompleteV3 usingretainedcompileroutputs; completegate result isnewrunonly, oldincompleteevidencenotgrafted. Candidate incremental codegen disabled only");out=R/'scripts/qualify-dedup-common-worker-fixed-candidate-v4-20261015.py';assert not out.exists();out.write_text(s)
p=R/'scripts/audit-dedup-common-worker-fixed-candidate-v3-20261015.py';s=p.read_text().replace('fixed-{variant}-gate-v3-','fixed-{variant}-gate-v4-').replace('qualify-dedup-common-worker-fixed-candidate-v3-','qualify-dedup-common-worker-fixed-candidate-v4-').replace('fixed-{variant}-audit-v3-','fixed-{variant}-audit-v4-');needle="assert sha(data['manifest']) == data['manifest_sha256']";s=s.replace(needle,needle+"\ninterruption_path = directory / 'dedup-common-candidate-v3-interruption-20261015/result.json'\nassert sha(interruption_path) == data['interruption_sha256']\ninterruption = json.loads(interruption_path.read_text())\nassert interruption['status'] == 'verified_stopped_incomplete_candidate_v3'\nassert interruption['manifest_sha256'] == data['manifest_sha256']");out=R/'scripts/audit-dedup-common-worker-fixed-candidate-v4-20261015.py';assert not out.exists();out.write_text(s)
p=R/'scripts/check-dedup-actual-collection-benchmark-admission-v2-20261015.py';s=p.read_text().replace('dedup-common-worker-fixed-candidate-audit-v3-','dedup-common-worker-fixed-candidate-audit-v4-').replace('dedup-common-worker-fixed-candidate-gate-v3-','dedup-common-worker-fixed-candidate-gate-v4-').replace('audit-dedup-common-worker-fixed-candidate-v3-','audit-dedup-common-worker-fixed-candidate-v4-');out=R/'scripts/check-dedup-actual-collection-benchmark-admission-v3-20261015.py';assert not out.exists();out.write_text(s);print('3378candidatepins verified; oldprocessesmissing; v4completegate/audit/admission prepared')
