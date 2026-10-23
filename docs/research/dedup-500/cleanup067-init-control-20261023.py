"""Original initializer in disposable trees, including owned dangling symlink."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
script=BASE.parents[2]/'.dedup-work/cleanup067-20261023/run-benchmark-driven-rd/scripts/init_rd.py'
data=script.read_bytes()
pin=next(x for x in json.loads((EV/'tree.json').read_text())['tree'] if x['path']=='run-benchmark-driven-rd/scripts/init_rd.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
with TemporaryDirectory(prefix='rrrah-cleanup-init-') as directory:
    base=Path(directory);project=base/'project'
    def run(target,*args):
        r=subprocess.run([sys.executable,str(script),'--project',str(target),*args],capture_output=True,text=True,timeout=10)
        assert r.returncode==0,r.stderr
        return r.stdout
    assert 'dry_run=True' in run(project,'--dry-run') and not project.exists()
    assert 'created=8' in run(project)
    charter=project/'.rd/CHARTER.md';charter.write_text('unique existing work\n')
    assert 'created=0 skipped=8' in run(project)
    assert charter.read_text()=='unique existing work\n'
    second=base/'symlink-project';(second/'.rd').mkdir(parents=True)
    outside=base/'owned-outside.txt'
    (second/'.rd/CHARTER.md').symlink_to(outside)
    assert not outside.exists()
    run(second)
    assert outside.is_file() and outside.read_text().startswith('# R&D Charter')
print(json.dumps({'original_initializer_verified':True,'native_cli_runs':4,'dry_run_no_writes':True,'initial_eight_files_created':True,'repeat_preserves_existing_content':True,'dangling_symlink_writes_outside_target_within_owned_fixture':True,'real_project_or_external_resource_changed':False},indent=2))
