"""Run unchanged upstream tests with only their application-data location isolated."""
from pathlib import Path
import hashlib
import json
import os
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-core-suite-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    W.mkdir(exist_ok=False)
    (W/'tmp').mkdir()
    names=sorted(str(p.relative_to(S)) for root in ['core','hscommon'] for p in (S/root).rglob('*') if p.is_file() and p.suffix in {'.py','.c','.h','.pyi'})
    pins={p:H(S/p)for p in names}
    for p in pins:assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    plugin=W/'private_appdata_plugin.py'
    plugin.write_text('''import pytest
import hashlib
from pathlib import Path
from hscommon import desktop
@pytest.fixture(autouse=True)
def isolated_appdata(monkeypatch, request):
    path=Path(__file__).parent/'private-appdata'/hashlib.sha256(request.node.nodeid.encode()).hexdigest()
    path.mkdir(parents=True)
    monkeypatch.setattr(desktop, '_special_folder_path', lambda *args, **kwargs: str(path))
''')
    py=R/'.dedup-work/dupeguru1-20261019/venv/bin/python'
    command=[str(py),'-m','pytest','-p','private_appdata_plugin','core/tests','-q','--basetemp',str(W/'pytest-temp'),'--junitxml',str(W/'junit.xml')]
    with (W/'tests.log').open('wb')as output:
        result=subprocess.run(command,cwd=S,env={**os.environ,'PYTHONPATH':str(W)+os.pathsep+str(S),'PYTEST_DISABLE_PLUGIN_AUTOLOAD':'1','TMPDIR':str(W/'tmp')},stdout=output,stderr=subprocess.STDOUT)
    report={'exit_code':result.returncode,'command':command,'source_pins':pins,'source_drift':[p for p in pins if H(S/p)!=pins[p]],'plugin_sha256':H(plugin),'log_sha256':H(W/'tests.log'),'scope':'Original complete test modules unchanged; only Desktop special-folder lookup redirected to per-test task-owned appdata outside scanned test fixture roots; TMPDIR also task-owned. No matching, grouping, persistence or exclusion logic replaced.'}
    (W/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));print((W/'tests.log').read_text()[-3000:])

if __name__=='__main__':main()
