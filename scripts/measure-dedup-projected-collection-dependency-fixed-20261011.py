#!/usr/bin/env python3
"""Build frozen generic controls, then execute isolated copied test programs."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
OUT=D/'dedup-projected-collection-dependency-fixed-generic-20261011.json'
MANIFEST=D/'dedup-projected-collection-dependency-fixed-snapshot-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-projected-collection-dependency-fixed-generic-qualified-20261011')
EXPECTED={'projected_collection':8}

def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
    manifest=json.loads(MANIFEST.read_text());snapshot=Path(manifest['snapshot'])
    def verify():
        assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
    verify()
    command=['cargo','test','--manifest-path',str(snapshot/'Cargo.toml'),'--target-dir',str(ROOT/'target'),
             '-p','rrrah-dedup','--locked','--offline','--no-default-features','--features','decode','--no-run','--message-format=json']
    for name in EXPECTED:command+=['--test',name]
    report={'status':'running','producer_pid':os.getpid(),'command':command,'source_manifest':str(MANIFEST),
            'source_manifest_sha256':sha(MANIFEST),'results':[]}
    def save():
        temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
    save()
    try:
        build=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        report['build_pid']=build.pid;save();stdout,stderr=build.communicate()
        (ARCHIVE/'build.jsonl').write_text(stdout);(ARCHIVE/'build.log').write_text(stderr)
        report['build_returncode']=build.returncode;save();assert build.returncode==0,stderr
        artifacts={}
        for line in stdout.splitlines():
            try: item=json.loads(line)
            except json.JSONDecodeError:continue
            if item.get('reason')=='compiler-artifact' and item.get('executable') and item['target']['name'] in EXPECTED:
                assert str(snapshot) in item['package_id']
                artifacts[item['target']['name']]=item
        assert set(artifacts)==set(EXPECTED)
        copied={}
        for name,item in artifacts.items():
            source=Path(item['executable']);digest=sha(source);target=ARCHIVE/name
            shutil.copy2(source,target);assert sha(source)==sha(target)==digest
            assert str(snapshot).encode() in target.read_bytes()
            copied[name]=(target,digest)
        for name,expected in EXPECTED.items():
            binary,digest=copied[name];assert sha(binary)==digest
            row={'suite':name,'status':'running','binary':str(binary),'binary_sha256':digest}
            report['results'].append(row);save()
            process=subprocess.Popen([str(binary)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            row['process_pid']=process.pid;save();stdout,stderr=process.communicate()
            log=ARCHIVE/(name+'.log');log.write_text(stdout+stderr)
            row.update(returncode=process.returncode,log=str(log),log_sha256=sha(log))
            assert process.returncode==0,(name,stdout,stderr)
            counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',stdout)
            assert counts==[str(expected)],(name,counts)
            assert sha(binary)==digest;row.update(status='passed',tests=expected);save()
            print(name,expected,flush=True)
        verify()
        pins={str(p):sha(p) for p in [MANIFEST,Path(__file__),ARCHIVE/'build.jsonl',ARCHIVE/'build.log']}
        report.update(status='complete_bounded_frozen_projected_collection_dependency_fixed_generic_execution',tests=sum(EXPECTED.values()),
                      frozen_files=len(manifest['file_hashes']),pins=pins,
                      scope='macOS decode-feature generic providers on frozen dependencies and copied pre-execution programs; no Torch or total RSS qualification')
    except Exception as e:
        report.update(status='failed_projected_collection_dependency_fixed_generic_gate',error=str(e));raise
    finally:save()
if __name__=='__main__':main()
