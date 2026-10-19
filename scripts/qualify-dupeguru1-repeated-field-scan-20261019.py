"""Check repeated original filename-field scans over real task-owned files."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-repeated-field-scan-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    W.mkdir(exist_ok=False)
    pins={p:H(S/p)for p in ['core/scanner.py','core/engine.py','core/fs.py']}
    for p in pins:assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    sys.path.insert(0,str(S))
    from core import fs
    from core.scanner import Scanner,ScanType
    paths=[W/'The White Stripes - Little Ghost.bin',W/'Little Ghost - The White Stripes.bin']
    for path in paths:path.write_bytes(b'ordinary fixture\n')
    scanner=Scanner();scanner.scan_type=ScanType.FIELDSNOORDER
    def run(s):
        return sorted(sorted(f.name for f in group) for group in s.get_dupe_groups([fs.File(p)for p in paths]))
    configured=scanner.scan_type;first=run(scanner);after_first=scanner.scan_type;second=run(scanner)
    fresh=Scanner();fresh.scan_type=ScanType.FIELDSNOORDER;fresh_result=run(fresh)
    assert len(first)==1 and len(first[0])==2 and second==[] and fresh_result==first
    assert configured==ScanType.FIELDSNOORDER and after_first==ScanType.FIELDS
    assert all(H(S/p)==pin for p,pin in pins.items())
    result={'status':'reproduced_stateful_repeated_no_field_order_scan_difference','source_pins':pins,'configured_scan_type':configured,'scan_type_after_first_run':after_first,'first_run_groups':first,'second_run_same_scanner_groups':second,'fresh_scanner_groups':fresh_result,'limits':['Original Scanner/engine/File modules with real ordinary synthetic files; no fake file-existence or digest provider.','Filename-field mode only, not exact byte matching or image pipeline.','No production/library or foreign source changed; this is a reference behavior counterexample.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
