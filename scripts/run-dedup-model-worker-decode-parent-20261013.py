"""Actual isolated inference from a decode-only parent, without Torch dependencies."""
import pathlib, json, subprocess, shutil, os, hashlib

R = pathlib.Path(__file__).resolve().parents[1]
T = pathlib.Path('/Users/themoretheless/.codex/tmp')
W = T / 'rrrah-model-worker-decode-parent-20261013'
H = W / 'harness'
P = R / 'docs/research/dedup-model-worker-decode-parent-20261013.json'
assert not W.exists() and not P.exists()
(H / 'src').mkdir(parents=True)
old = T / 'rrrah-model-worker-real-controls-20261013/harness'
manifest = (old / 'Cargo.toml').read_text()
manifest = '\n'.join(line for line in manifest.splitlines() if not line.startswith(('loftr =', 'tch =')))
manifest = manifest.replace('dedup-model-worker-real-controls', 'dedup-model-worker-decode-parent').replace('features = ["loftr"]', 'features = ["decode"]')
(H / 'Cargo.toml').write_text(manifest + '\n')
source = (old / 'src/main.rs').read_text().replace(',serve_loftr_worker_once', '')
source = source.replace('tch::set_num_threads(2);tch::manual_seed(173);', '')
a = source.index(' let count;')
b = source.index(' let isolated=compare_model_files_with', a)
source = source[:a] + source[b:]
source = source.replace(' assert_eq!(format!("{direct:?}"),format!("{isolated:?}"));', '')
start = source.index(' println!(')
source = source[:start] + ''' assert!(isolated.supported());
 println!("{}",serde_json::json!({"supported":isolated.supported(),"timeout_is_provider_error":true,"managed_used":budget.used(),"evidence":format!("{isolated:?}")}));Ok(())
}
'''
(H / 'src/main.rs').write_text(source)
def sha(p):
    return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
worker = json.loads((R / 'docs/research/dedup-model-worker-20261013.json').read_text())
self_report = json.loads((R / 'docs/research/dedup-model-worker-self-control-20261013.json').read_text())
reference = json.loads(json.loads(pathlib.Path(self_report['case']).read_text())['stdout'])
assert sha(worker['binary']) == worker['binary_sha256']
d = dict(status='building', producer_pid=os.getpid(), pins={str(p): sha(p) for p in [H / 'Cargo.toml', H / 'src/main.rs', pathlib.Path(worker['binary']), pathlib.Path(self_report['case']), pathlib.Path(__file__)]})
def save():
    P.write_text(json.dumps(d, indent=2) + '\n')
save()
env = dict(os.environ)
env.pop('DYLD_LIBRARY_PATH', None)
env.pop('LIBTORCH_USE_PYTORCH', None)
target = T / 'rrrah-cache-interpretation-fix-20261012/target'
cmd = ['cargo', 'build', '--offline', '--target-dir', str(target)]
d['build_command'] = cmd
with (W / 'build.log').open('x') as log:
    child = subprocess.Popen(cmd, cwd=H, env=env, stdout=log, stderr=subprocess.STDOUT)
    d['child_pid'] = child.pid
    save()
    code = child.wait()
d['build_returncode'] = code
if code:
    d['status'] = 'terminal_build_failure'
    save()
    raise SystemExit(code)
binary = W / 'probe'
shutil.copy2(target / 'debug/dedup-model-worker-decode-parent', binary)
tree = subprocess.check_output(['cargo', 'tree', '--offline', '--locked'], cwd=H, env=env, text=True)
assert not any(x in tree for x in ['tch v', 'torch-sys v', 'loftr v'])
links = subprocess.check_output(['otool', '-L', str(binary)], text=True)
assert 'libtorch' not in links.lower() and 'libc10' not in links.lower()
(W / 'tree.txt').write_text(tree)
(W / 'links.txt').write_text(links)
cmd = list(self_report['command'])
cmd[0] = str(binary)
for p in [binary, H / 'Cargo.lock', W / 'tree.txt', W / 'links.txt', W / 'build.log', pathlib.Path(cmd[1]), pathlib.Path(cmd[3])]:
    d['pins'][str(p)] = sha(p)
d.update(status='running', command=cmd, parent_torch_dependencies=False)
save()
child = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
d['child_pid'] = child.pid
save()
out, err = child.communicate()
case = W / 'case.json'
case.write_text(json.dumps(dict(returncode=child.returncode, stdout=out, stderr=err), indent=2) + '\n')
d['pins'][str(case)] = sha(case)
d['returncode'] = child.returncode
if child.returncode == 0:
    result = json.loads(out)
    assert result['evidence'] == reference['direct_evidence']
    assert result['supported'] and result['timeout_is_provider_error'] and result['managed_used'] == 0
    assert all(sha(p) == value for p, value in d['pins'].items())
    d.update(status='verified_decode_only_parent_identity_control', full_evidence_equal=True, managed_used=0)
else:
    d['status'] = 'terminal_execution_failure'
save()
print(d['status'])
