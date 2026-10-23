"""Whole original CLIs on owned synthetic directories; no outside audit/cleanup."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
CACHE=BASE.parents[2]/'.dedup-work/cleanup067-20261023'
pins={e['path']:e['sha'] for e in json.loads((EV/'tree.json').read_text())['tree'] if e['type']=='blob'}
with TemporaryDirectory(prefix='rrrah-cleanup-cli-') as directory:
    base=Path(directory);scripts=base/'scripts';scripts.mkdir();root=base/'target';root.mkdir()
    names=['audit.py','check_drift.py','check_links.py','check_sync.py','audit_core.py']
    for name in names:
        source=EV/'03-audit_core.py' if name=='audit_core.py' else CACHE/'code-cleanup-helper/scripts'/name
        data=source.read_bytes();assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pins['code-cleanup-helper/scripts/'+name]
        (scripts/name).write_bytes(data)
    def run(name,*args):
        return subprocess.run([sys.executable,str(scripts/name),*map(str,args)],capture_output=True,text=True,timeout=20)
    missing=base/'missing'
    assert run('audit.py',missing).returncode==2
    (root/'bad.py').write_text('def broken(:\n')
    normal=run('audit.py',root,'--format','json')
    assert normal.returncode==0 and json.loads(normal.stdout)['summary']['fail']>0
    assert run('audit.py',root,'--strict').returncode==1
    (root/'bad.py').unlink()
    empty=run('audit.py',root,'--strict','--format','json')
    assert empty.returncode==0 and json.loads(empty.stdout)['summary']['not_checked']>0
    assert run('check_sync.py',root).returncode==3
    assert run('check_links.py',missing).returncode==0
    assert run('check_drift.py',missing).returncode==0
    (root/'doc.md').write_text('## R1: First\n\n## R3: Third\n\nR1-R3\n')
    assert run('check_drift.py',root).returncode==0
print(json.dumps({'pinned_inputs_verified':5,'original_cli_processes_run':8,'missing_full_audit_target_exit':2,'non_strict_fail_exit':0,'strict_fail_exit':1,'strict_incomplete_exit':0,'unconfigured_sync_exit':3,'links_missing_target_exit':0,'drift_missing_target_exit':0,'missing_middle_id_not_reported':True},indent=2))
