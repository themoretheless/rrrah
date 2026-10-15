from pathlib import Path
import hashlib,json,subprocess,datetime,ast
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-durable-pair-v5-20261015';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dedup-durable-source-pair-v5-prepared-20261015.json';m=json.loads(p.read_text());old=json.loads((R/m['legacy_manifest']).read_text());orig=json.loads((R/m['original_git_manifest']).read_text())
assert h(R/m['legacy_manifest'])==m['legacy_manifest_sha256'];assert h(R/m['original_git_manifest'])==m['original_git_manifest_sha256']
for a,b in zip(m['variants'],old['variants']):
 assert a['variant']==b['variant'] and a['source_files']==b['source_files'] and len(a['source_files'])==3378
 for row in a['source_files']:assert h(Path(a['source'])/row['path'])==row['sha256']
for row in orig['source_files']:
 p=R/'.dedup-work/20261015-common-v5/original415'/row['path'];raw=p.read_bytes();assert h(p)==row['sha256'] and len(raw)==row['bytes'];assert hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==row['git_blob_sha1'];assert p.stat().st_mode&0o777==(0o755 if row['mode']=='100755' else 0o644)
for row in m['overlay_artifacts']+m['transform_scripts']:assert h(R/row['path'])==row['sha256']
setup=json.loads((E/'setup-controls.json').read_text())
for row in setup['script_pins']:assert h(R/row['path'])==row['sha256'];ast.parse((R/row['path']).read_text())
assert setup['admission_returncode']!=0 and h(E/'admission-refuses-unqualified-pair.log')==setup['admission_log_sha256']
assert subprocess.run(['git','check-ignore','-q',m['variants'][0]['source']],cwd=R).returncode==0
loss=json.loads((E/'v4-loss-observation.json').read_text());assert loss['status']=='verified_v4_job_stopped_tmp_inputs_and_runtime_artifacts_unavailable'
out=dict(status='verified_byte_exact_durable_reconstruction_and_setup_not_test_qualification',at_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_files_per_variant=3378,original_git_files=3378,manifest_sha256=h(D/'dedup-durable-source-pair-v5-prepared-20261015.json'),setup_sha256=h(E/'setup-controls.json'),loss_observation_sha256=h(E/'v4-loss-observation.json'),scope='Independent filesystem hashes, original Git blob SHA1/size/mode, legacy row equality, overlay/producer pins and unqualified-admission refusal. No Cargo pass, runtime correctness or performance credit.')
p=E/'reconstruction-audit.json';assert not p.exists();p.write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
