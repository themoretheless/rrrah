"""Probe original traversal on small task-owned acyclic directory fixtures."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-directory-controls-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    assert subprocess.check_output(['git','-C',str(S),'rev-parse','HEAD'],text=True).strip()=='db097a030150b3d4ea9f492b85e7e4e4bcb3f789'
    W.mkdir(exist_ok=False)
    pins={p:H(S/p)for p in ['core/directories.py','core/fs.py','core/scanner.py']}
    for p in pins:assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    sys.path.insert(0,str(S))
    from core.directories import Directories,DirectoryState
    from core.fs import filesdb
    from core.scanner import Scanner,ScanType
    from hscommon.jobprogress.job import Job,JobCancelled
    selected=W/'selected'; selected.mkdir()
    normal=selected/'normal'; normal.mkdir(); (normal/'payload.bin').write_bytes(b'ordinary duplicate fixture\n')
    (selected/'alias').symlink_to(normal,target_is_directory=True)
    d=Directories(); d.add_path(selected)
    files=list(d.get_files()); paths=sorted(str(f.path.relative_to(selected))for f in files)
    assert paths==['alias/payload.bin','normal/payload.bin']
    assert files[0].path.samefile(files[1].path)
    filesdb.connect(W/'cache.sqlite')
    scanner=Scanner();scanner.scan_type=ScanType.CONTENTS
    groups=scanner.get_dupe_groups(files)
    assert len(groups)==1 and len(groups[0])==2
    filesdb.close()
    missing=W/'gone';missing.mkdir(); stale=Directories();stale.add_path(missing);missing.rmdir()
    assert list(stale.get_files())==[]
    hidden=selected/'.hidden';hidden.mkdir();(hidden/'hidden.bin').write_bytes(b'hidden')
    state=Directories();state.add_path(selected);default_files=list(state.get_files());assert not any(f.path.name=='hidden.bin'for f in default_files)
    state.set_state(hidden,DirectoryState.NORMAL);assert any(f.path.name=='hidden.bin'for f in state.get_files())
    cancelled=False
    try:list(d.get_files(j=Job(1,lambda *args:False)))
    except JobCancelled:cancelled=True
    assert cancelled
    assert all(H(S/p)==pin for p,pin in pins.items())
    result={'status':'verified_original_acyclic_directory_controls','source_pins':pins,'directory_symlink':{'paths':paths,'same_physical_file':True,'scanner_publishes_group_of_two':True},'vanished_root_returns_empty_stream':True,'hidden_default_excluded_explicit_normal_included':True,'pre_cancel_raises':cancelled,'limits':['Task-owned synthetic acyclic symlink only; no external directory or cycle traversed.','Duplicate physical aliases are observed behavior, not two independently stored payloads.','Original iterator lacks a report-completeness diagnostic; empty stream alone does not prove complete traversal.','No GUI, native-platform or full repository qualification; no production adoption.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
