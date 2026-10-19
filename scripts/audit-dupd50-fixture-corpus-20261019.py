"""Review the complete fixture corpus by exact bytes and independent expected partitions."""
from pathlib import Path
from collections import Counter, defaultdict
import difflib
import hashlib
import io
import json
import os
import subprocess
import tarfile

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research/dedup-500'
S = R / '.dedup-work/dupd50-20261019/source'
W = R / '.dedup-work/dupd50-fixture-corpus-20261019'
SHA = lambda b: hashlib.sha256(b).hexdigest()

def main():
    W.mkdir(exist_ok=False)
    assert subprocess.check_output(['git','-C',str(S),'rev-parse','HEAD'],text=True).strip() == '33907bbcfd5fe8b63f54d4f55438fa222ea880aa'
    entries = []
    for line in subprocess.check_output(['git','-C',str(S),'ls-tree','-rl','HEAD'],text=True).splitlines():
        meta, path = line.split('\t'); mode, kind, blob, size = meta.split()
        assert kind == 'blob'
        entries.append({'path':path,'mode':mode,'git_blob':blob,'bytes':int(size)})
    request = ''.join(x['git_blob']+'\n' for x in entries).encode()
    output = io.BytesIO(subprocess.check_output(['git','-C',str(S),'cat-file','--batch'], input=request))
    data = {}
    for entry in entries:
        blob, kind, length = output.readline().decode().split()
        assert blob == entry['git_blob'] and kind == 'blob' and int(length) == entry['bytes']
        raw = output.read(int(length)); assert output.read(1) == b'\n'
        path = S / entry['path']
        actual = os.fsencode(os.readlink(path)) if entry['mode']=='120000' else path.read_bytes()
        assert raw == actual
        data[entry['path']] = raw; entry['sha256'] = SHA(raw)
    assert output.read() == b'' and len(entries) == 771
    already = set()
    for report in D.glob('dupd50*.json'):
        a = json.loads(report.read_text())
        for key in ['whole_read_blobs','new_whole_blobs','additional_generated_artifacts_completely_verified']:
            already.update(x['path'] for x in a.get(key,[]))
    remaining = [x for x in entries if x['path'] not in already]
    assert len(remaining)==607 and len({x['git_blob'] for x in remaining})==68
    assert all(x['path'].startswith('tests/') for x in remaining)
    groups = defaultdict(list)
    for entry in entries:
        p = entry['path']
        if p.startswith('tests/files/') and entry['mode']!='120000':
            groups[data[p]].append(p.removeprefix('tests/files/'))
    duplicates = [v for v in groups.values() if len(v)>1]
    assert len(groups)==17 and len(duplicates)==9 and sum(map(len,duplicates))==538
    assert sum(map(len,groups.values()))==546
    classes = {name:len(names) for names in groups.values() for name in names}
    expected08 = Counter(('DUPLICATE: ' if classes[name]>1 else '   UNIQUE: ')+name for name in classes)
    expected10 = Counter(name for name in classes if classes[name]>1)
    expected29 = expected08.copy()
    for name,n in classes.items():
        if n>1: expected29['             DUP: '+name] = n-1
    expected17 = Counter({'   UNIQUE: many/ma.1':1})
    expected17.update('             xxx: many/ma.'+str(i) for i in range(2,516))
    expected01 = Counter({'':19})
    for raw,names in groups.items():
        if len(names)<2: continue
        expected01.update('  '+name for name in names)
        expected01[f'{len(raw)*len(names)} total bytes used by duplicates of size {len(raw)}:'] += 1
    used = sum(len(raw)*len(names) for raw,names in groups.items() if len(names)>1)
    expected01[f'Total used: {used} bytes ({used//1024} KiB, {used//(1024*1024)} MiB, {used//(1024*1024*1024)} GiB)'] += 1
    golden_checks = []
    for name,expected in [('01',expected01),('08',expected08),('10',expected10),('17',expected17),('29',expected29)]:
        raw = data['tests/output.'+name]; actual = Counter(raw.decode().splitlines())
        assert actual==expected, (name,actual-expected,expected-actual)
        golden_checks.append({'path':'tests/output.'+name,'bytes':len(raw),'lines':sum(actual.values()),'unique_lines':len(actual),'exact_multiset_matches_independent_fixture_partition':True})
    small_goldens = ['05','12','14','15','23','24','35','40','41','43','44','50','55','56','58a','58b','58c','59a','59b','60a','60b','61a','61b','62a','62b','72']
    assert {x['path'] for x in remaining if x['path'].startswith('tests/output.')} == {'tests/output.'+n for n in small_goldens+['01','08','10','17','29']}
    archives = []
    for name in ['tests/files6/files.tar.gz','tests/files9/files.tar.gz']:
        members=[]; by_content=defaultdict(list)
        with tarfile.open(fileobj=io.BytesIO(data[name])) as archive:
            for member in archive:
                assert member.isfile() and member.name in ['1','2','3','4','5']
                raw=archive.extractfile(member).read(); assert len(raw)==member.size
                runs=[]
                for byte in raw:
                    if runs and runs[-1]['byte']==byte:runs[-1]['count']+=1
                    else:runs.append({'byte':byte,'count':1})
                assert len(runs)<=9
                assert b''.join(bytes([run['byte']])*run['count'] for run in runs)==raw
                members.append({'name':member.name,'size':len(raw),'sha256':SHA(raw),'complete_run_length_representation':runs})
                by_content[raw].append(member.name)
        assert len(members)==5
        expected=[['1','2','5'],['3'],['4']] if '/files6/' in name else [['1','2'],['3','4'],['5']]
        assert sorted(sorted(v) for v in by_content.values())==sorted(expected)
        archives.append({'path':name,'members':members,'partitions':sorted(sorted(v) for v in by_content.values())})
    patches={}
    base=data['COPYING'].decode().splitlines(True)
    for name,raw in data.items():
        if name.startswith('tests/files4/') and not name.rsplit('/',1)[1].startswith('z'):
            text=raw.decode().splitlines(True)
            delta=list(difflib.ndiff(base,text))
            assert ''.join(difflib.restore(delta,2)).encode()==raw
            patches[name]=''.join(difflib.unified_diff(base,text,fromfile='COPYING',tofile=name,n=1))
    (W/'complete-license-fixture-deltas.json').write_text(json.dumps(patches,indent=2)+'\n')
    for name in ['file5a','file5b','file5c','file5d']:
        assert data['tests/files/'+name]==data['tests/files/file4']+name[-1:].encode()+b'\n'
    assert data['tests/files/file3']==data['tests/files/file1']+b'this file differs past the 8k boundary only\n'
    many=[x for x in entries if x['path'].startswith('tests/files/many/')]
    assert len(many)==515 and all(data[x['path']]==b'hi\n' for x in many)
    for entry in remaining:
        p=entry['path']
        entry['review_method']=('exact_symlink_payload' if entry['mode']=='120000' else 'complete_archive_content_rle' if p.endswith('.tar.gz') else 'independent_expected_golden_multiset' if p in {'tests/output.'+n for n in ['01','08','10','17','29']} else 'whole_small_golden_text_read' if p.startswith('tests/output.') else 'complete_fixture_bytes_and_partition_analysis')
    result={'status':'verified_complete_remaining_fixture_corpus','remaining_paths':len(remaining),'distinct_remaining_git_blobs':len({x['git_blob'] for x in remaining}),'all_tree_blob_count':len(entries),'reviewed_remaining_paths':remaining,'all_tree_blob_manifest':entries,'golden_checks':golden_checks,'archives':archives,'base_fixture_partitions':[{'bytes':len(raw),'sha256':SHA(raw),'paths':names} for raw,names in groups.items()],'license_fixture_delta_artifact':'complete-license-fixture-deltas.json','limits':['Binary fixtures analyzed as complete byte sequences and partitions, not visual media or hand-read hex dumps.','Golden expectations use complete line multisets because original upstream checker sorts report lines; original display ordering is not qualified.','This is source/fixture review, not execution of the five deferred upstream scripts or all-platform testing.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:result[k] for k in ['status','remaining_paths','distinct_remaining_git_blobs','all_tree_blob_count']}))

if __name__=='__main__': main()
