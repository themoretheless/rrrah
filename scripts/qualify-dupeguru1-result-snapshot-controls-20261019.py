"""Exercise original snapshot loading and CSV roundtrip on ordinary task-owned data."""
from pathlib import Path
import csv
import hashlib
import os
import subprocess
import sys
import json

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-result-controls-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    assert subprocess.check_output(['git','-C',str(S),'rev-parse','HEAD'],text=True).strip()=='db097a030150b3d4ea9f492b85e7e4e4bcb3f789'
    W.mkdir(exist_ok=False)
    names=['core/results.py','core/export.py','core/engine.py','core/fs.py','hscommon/conflict.py']
    pins={p:H(S/p)for p in names}
    for p in pins:assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    sys.path.insert(0,str(S))
    from core import engine,fs,export
    from core.results import Results
    fs.filesdb.connect(W/'cache.sqlite')
    a=W/'original.bin';b=W/'copy.bin'
    a.write_bytes(b'ordinary snapshot content\n');b.write_bytes(a.read_bytes())
    def files():
        output=[fs.File(p)for p in [a,b]]
        for f in output:f.is_ref=False
        return output
    initial=engine.get_groups(engine.getmatches_by_contents(files()))
    assert len(initial)==1 and len(initial[0])==2
    snapshot=Results(None);snapshot.groups=initial;snapshot.save_to_xml(W/'results.xml')
    before=b.stat();payload=bytearray(b.read_bytes());payload[len(payload)//2]^=1;b.write_bytes(payload)
    os.utime(b,ns=(before.st_atime_ns,before.st_mtime_ns+1000000000))
    restored=Results(None);restored.load_from_xml(W/'results.xml',lambda p:fs.get_file(Path(p)))
    assert len(restored.groups)==1 and len(restored.groups[0])==2
    assert len(engine.getmatches_by_contents(files()))==0 and H(a)!=H(b)
    rows=[[1,' leading trailing ','comma,name'],[1,'quote"name','line\nbreak'],[2,'таблица','tab\tvalue']]
    export.export_to_csv(W/'report.csv',['Name','Value'],rows)
    with (W/'report.csv').open(newline='',encoding='utf-8')as stream:decoded=list(csv.reader(stream))
    assert decoded==[['Group ID','Name','Value']]+[[str(row[0]),*row[1:]]for row in rows]
    fs.filesdb.close();assert all(H(S/p)==pin for p,pin in pins.items())
    result={'status':'verified_original_snapshot_and_csv_controls','source_pins':pins,'saved_group_count':1,'loaded_group_count_after_content_change':1,'fresh_full_content_match_count_after_change':0,'files_have_different_sha256':True,'stored_match_percentages':[m.percentage for m in restored.groups[0].matches],'csv_rows_roundtrip':len(rows),'csv_sha256':H(W/'report.csv'),'snapshot_sha256':H(W/'results.xml'),'limits':['Loading persisted groups restores a historical snapshot and is not a fresh content-equality proof.','No marked-file action, deletion or move performed; only task-owned synthetic data used.','CSV normal Unicode/quoting controls only; no XHTML renderer, GUI, all-path encoding or full repository claim.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
