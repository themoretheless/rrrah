#!/usr/bin/env python3
"""Execute all458 fixed publisher directions on the qualified current snapshot.

Retain one immutable artifact per case; do not repeatedly rewrite full evidence.
Use a distinct binary name so historical executable pins remain intact.
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    manifest = D / 'dedup-current-decode-gate-20261012.json'
    audit = D / 'dedup-current-decode-gate-complete-audit-20261012.json'
    m, a = [json.loads(p.read_text()) for p in [manifest, audit]]
    assert a['status'] == 'verified_complete_frozen_current_decode_gate'
    assert all(sha(p) == h for p, h in a['pins'].items())
    source = Path(m['source'])
    assert all(sha(source/n) == h for n, h in m['source_hashes'].items())
    adoption = D / 'dedup-loftr-position-floor-adoption-20261012.json'
    adopted = json.loads(adoption.read_text())
    assert all(sha(source/'vendor/loftr/src'/n) == h for n, h in adopted['after'].items())
    old_manifest = D / 'dedup-native-bank-outcome-native-harness-20261011.json'
    old = json.loads(old_manifest.read_text())
    assert all(sha(p) == h for p, h in old['pins'].items())
    original = T / 'rrrah-native-bank-outcome-native-harness-20261011'
    old_source = T / 'rrrah-native-bank-outcome-native-snapshot-20261011'
    harness = T / 'rrrah-current-native-bank-all-positives-harness-20261012'
    folder = T / 'rrrah-current-native-bank-all-positives-qualified-20261012'
    out = D / 'dedup-current-native-bank-all-positives-20261012.json'
    assert not any(p.exists() for p in [harness, folder, out])
    harness.mkdir(); folder.mkdir()
    text = (original/'Cargo.toml').read_text()
    assert text.count(str(old_source)) == 7
    (harness/'Cargo.toml').write_text(text.replace(str(old_source), str(source)))
    shutil.copy2(original/'Cargo.lock', harness/'Cargo.lock')
    binary_name = 'native_bank_current_all_positives'
    caller = harness/'src/bin'/f'{binary_name}.rs'
    caller.parent.mkdir(parents=True)
    shutil.copy2(original/'src/bin/native_bank_outcome_files.rs', caller)
    policy = harness/'src/native_policy.rs'
    shutil.copy2(original/'src/native_policy.rs', policy)
    ordering = T / 'rrrah-loftr-publisher-positives-qualified-20261011/audits/ordering-prefix-0458.json'
    b = json.loads(ordering.read_text())
    priority = [3,7,27,57,67,153,255]
    indices = priority + [i for i in range(458) if i not in priority]
    assert len(b['plan']) == 458 and sorted(indices) == list(range(458))
    plan = [dict(b['plan'][i], baseline_index=i) for i in indices]
    weights = T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors'
    runtime_manifest = D/'dedup-loftr-native-20261010-runtime-pins.json'
    rt = json.loads(runtime_manifest.read_text())
    libs = {p:h for p,h in rt['pins'].items() if p.endswith('.dylib')}
    assert libs and all(sha(p)==h for p,h in libs.items())
    pinned = [manifest,audit,adoption,old_manifest,ordering,runtime_manifest,weights,
              harness/'Cargo.toml',harness/'Cargo.lock',caller,policy,Path(__file__)]
    pins = {str(p):sha(p) for p in pinned}
    pins.update(libs)
    for row in plan:
        for key in ['left','right']:
            pins[row[key]] = sha(row[key])
    r = dict(status='building', producer_pid=os.getpid(), source=str(source),
             source_manifest=str(manifest), harness=str(harness), folder=str(folder),
             pins=pins, plan=plan, required=458, completed=0, results=[],
             counts=dict(supported=0,inconclusive=0,rejected=0,execution_failed=0),
             scope='All229 publisher positives in both directions on frozen current native/opposite/bank composition. Exact historical caller/policy bytes and ordering; only manifest path substitutions and distinct binary filename. Fixed4096native/1mopposite/25banks/radii2and6; no per-case tuning. Not semantic negative precision, fullformats/platforms/full500 or hardRSS/latency qualification.')
    def save():
        tmp = out.with_suffix('.tmp')
        tmp.write_text(json.dumps(r,indent=2)+'\n')
        tmp.replace(out)
    save()
    try:
        target = T/'rrrah-dedup-exact-portfolio-20261011'
        build_json = folder/'build.jsonl'
        build_log = folder/'build.stderr'
        runtime = T/'rrrah-loftr-native-runtime-20261010'
        env = dict(os.environ, LIBTORCH_USE_PYTORCH='1')
        env['PATH'] = str(runtime/'bin')+':'+env['PATH']
        cmd = ['cargo','build','--locked','--offline','--bin',binary_name,
               '--message-format=json','--target-dir',str(target)]
        r['build_command'] = cmd
        with build_json.open('w') as o, build_log.open('w') as e:
            child = subprocess.Popen(cmd,cwd=harness,env=env,stdout=o,stderr=e)
            r['build_pid'] = child.pid; save()
            rc = child.wait()
        r['build_returncode'] = rc
        r['pins'].update({str(p):sha(p) for p in [build_json,build_log]})
        if rc:
            r['status']='terminal_build_failure'; save(); return
        entries = [json.loads(line) for line in build_json.read_text().splitlines()]
        assert entries[-1]['reason']=='build-finished' and entries[-1]['success']
        executables = [v['executable'] for v in entries if v.get('reason')=='compiler-artifact'
                       and v.get('target',{}).get('name')==binary_name and v.get('executable')]
        assert len(executables)==1
        binary = folder/binary_name
        shutil.copy2(executables[0],binary)
        r['pins'][str(binary)] = sha(binary)
        r.update(status='running',executable=str(binary)); save()
        for index,row in enumerate(plan):
            for p in [row['left'],row['right'],str(binary),str(weights)]:
                assert sha(p)==r['pins'][p]
            cmd = ['/usr/bin/time','-l','/usr/bin/env',
                   'DYLD_LIBRARY_PATH='+str(runtime/'lib/python3.12/site-packages/torch/lib'),
                   str(binary),row['left'],row['right'],str(weights),'640',
                   str(row['source_tolerance']),'1000000']
            start = time.monotonic()
            child = subprocess.Popen(cmd,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            r.update(active_index=index,child_pid=child.pid); save()
            stdout,stderr = child.communicate()
            case = dict(index=index,baseline_index=row['baseline_index'],command=cmd,
                        returncode=child.returncode,stdout=stdout,stderr=stderr,
                        elapsed_seconds=time.monotonic()-start)
            kind = 'execution_failed'
            if child.returncode==0:
                v = json.loads(stdout)
                assert v['files']==[2,9] and v['managed_used']==0 and len(v['pairs'])==1
                pair = v['pairs'][0]
                assert (pair['left'],pair['right'])==(2,9)
                support = pair['support']
                assert support is None or type(support) is bool
                kind = 'supported' if support is True else ('rejected' if support is False else 'inconclusive')
                case['rss_bytes'] = int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
            case['status'] = kind
            path = folder/f'{index:04d}.json'
            assert not path.exists()
            path.write_text(json.dumps(case,indent=2)+'\n')
            r['results'].append(dict(index=index,baseline_index=row['baseline_index'],status=kind,
                                     report=str(path),sha256=sha(path)))
            r['counts'][kind] += 1
            r['completed'] = index+1
            for p in [row['left'],row['right'],str(binary),str(weights)]:
                assert sha(p)==r['pins'][p]
            save(); print(index,row['baseline_index'],kind,flush=True)
        assert all(sha(p)==h for p,h in r['pins'].items())
        assert all(sha(source/n)==h for n,h in m['source_hashes'].items())
        r['status']='complete_current_fixed458_execution'; save()
    except Exception as error:
        r.update(status='terminal_controller_failure',error=str(error)); save(); raise

if __name__=='__main__':
    main()
