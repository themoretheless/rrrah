"""Qualify actual RAW development/fingerprints, with pinned source and sensor oracle."""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('corpus',type=Path);p.add_argument('--probe',type=Path,required=True);p.add_argument('--report',type=Path,required=True);p.add_argument('--budget-mib',type=int,default=1024);p.add_argument('--filename',action='append');p.add_argument('--timeout-seconds',type=int,default=180);p.add_argument('--include-local-cr3',action='store_true');p.add_argument('--verify-failure-retry',action='store_true');args=p.parse_args()
if args.budget_mib <= 0:p.error('--budget-mib must be positive')
if args.timeout_seconds <= 0:p.error('--timeout-seconds must be positive')
repo=Path(__file__).resolve().parent.parent
manifest_path=repo/'tests/fixtures/raw-color-corpus.json';manifest=json.loads(manifest_path.read_text())
report={'scope':'Pinned source hash and independent sensor dimensions; full native development, finite normalized pixels, exact digest, visual fingerprint and managed memory release. Does not certify independent rendered color or RAW-versus-raster equality.',
        'manifest_sha256':hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        'probe_sha256':hashlib.sha256(args.probe.read_bytes()).hexdigest(),'budget_bytes':args.budget_mib*1024*1024,'timeout_seconds':args.timeout_seconds,'status':'running','cases':[]}
def save():
    args.report.parent.mkdir(parents=True,exist_ok=True)
    temporary=args.report.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(args.report)
required_cases=manifest['cases']+(manifest.get('local_cases',[]) if args.include_local_cr3 else [])
cases=[c for c in required_cases if args.filename is None or c['filename'] in args.filename]
if args.filename and set(args.filename)-{c['filename'] for c in cases}:p.error('unknown selected filename')
report['selected_filenames']=[c['filename'] for c in cases]
report['manifest_required']=len(required_cases)
for case in cases:
    path=(repo if case.get('local_source') else args.corpus)/case['filename']
    item={'filename':case['filename'],'source_sha256':case['sha256'],'status':'failed'}
    try:
        digest=hashlib.sha256()
        with path.open('rb') as f:
            for block in iter(lambda:f.read(65536),b''):digest.update(block)
        if digest.hexdigest()!=case['sha256']:raise ValueError('source SHA256 mismatch')
        oracle=case['oracle']
        command=[str(args.probe),str(path),str(oracle['width']),str(oracle['height']),str(args.budget_mib)]
        if args.verify_failure_retry:command.append('--verify-failure-retry')
        result=subprocess.run(command,capture_output=True,text=True,timeout=args.timeout_seconds)
        if result.returncode:raise RuntimeError(f'probe exited {result.returncode}: {result.stderr.strip()}')
        item.update(json.loads(result.stdout))
        if args.verify_failure_retry and item.get('budget_refusal_verified') is not True:raise ValueError('probe did not verify requested budget refusal/retry')
        item['status']='passed'
    except Exception as error:item['error']=str(error)
    report['cases'].append(item);save();print(json.dumps(item),flush=True)
report['status']='finished'
report['passed']=sum(c['status']=='passed' for c in report['cases']);report['required']=len(cases);save()
raise SystemExit(0 if report['passed']==report['required'] else 1)
