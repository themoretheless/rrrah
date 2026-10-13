#!/usr/bin/env python3
"""Close frozen Linux and real ModelFirst recovery gates; retain compressed ELF evidence."""
from pathlib import Path
import gzip
import hashlib
import json
import re
import shutil

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
LOG=D/'dedup-insufficient-anchor-linux-20261011.log'
MANIFEST=D/'dedup-insufficient-anchor-snapshot-20261011.json'
PAIR_AUDIT=D/'dedup-full229-211201-model-first-recovery-audit-20261011.json'
LINUX_AUDIT=D/'dedup-insufficient-anchor-linux-audit-20261011.json'
OUT=D/'dedup-insufficient-anchor-recovery-audit-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-insufficient-anchor-qualified-20261011')
VALIDATION=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native')
HELPER=Path('/Users/themoretheless/.codex/tmp/rrrah-projected-identity-linux-20261011.sh')

def sha(path):
    digest=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b''):digest.update(chunk)
    return digest.hexdigest()

def main():
    assert not OUT.exists() and not LINUX_AUDIT.exists() and not ARCHIVE.exists()
    text=LOG.read_text();counts=[int(n) for n in re.findall(r'test result: ok\. (\d+) passed;',text)]
    assert counts==[72,7,11,9,8,8,13,10,6,12],counts
    manifest=json.loads(MANIFEST.read_text());snapshot=Path(manifest['snapshot'])
    assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
    pair=json.loads(PAIR_AUDIT.read_text());assert pair['status']=='complete_bounded_frozen_model_first_pair_recovery'
    report_path=ROOT/pair['report'];assert sha(report_path)==pair['report_sha256']
    r=json.loads(report_path.read_text());assert all(sha(p)==h for p,h in r['pins'].items())
    assert pair['all_25_model_attempts_completed'] and pair['exact_native_first_geometry_and_pixel_proof']
    alternative=json.loads((D/'dedup-full229-211201-native-first-20261011.json').read_text())
    assert r['evidence']['managed_peak']==alternative['evidence']['managed_peak']
    paths=re.findall(r'Running [^\n]*\((/validation/target/debug/deps/[^)]+)\)',text)
    assert len(paths)==10
    ARCHIVE.mkdir();artifacts=[]
    for name in paths:
        source=VALIDATION/Path(name).relative_to('/validation')
        with source.open('rb') as stream:assert stream.read(4)==b'\x7fELF'
        expected=sha(source);target=ARCHIVE/(source.name+'.gz')
        with source.open('rb') as stream,target.open('wb') as output:
            with gzip.GzipFile(filename='',mode='wb',fileobj=output,compresslevel=1,mtime=0) as packed:
                shutil.copyfileobj(stream,packed,1024*1024)
        digest=hashlib.sha256()
        with gzip.open(target,'rb') as stream:
            for chunk in iter(lambda:stream.read(1024*1024),b''):digest.update(chunk)
        assert digest.hexdigest()==expected==sha(source)
        artifacts.append({'source':str(source),'archive':str(target),'format':'gzip','sha256':sha(target),'executable_sha256':expected,'executable_bytes':source.stat().st_size,'executable_mode':source.stat().st_mode & 0o7777,'archive_bytes':target.stat().st_size})
    for source in [LOG,MANIFEST,HELPER,Path(__file__),PAIR_AUDIT]:
        target=ARCHIVE/source.name;expected=sha(source);shutil.copy2(source,target);assert sha(target)==expected
        artifacts.append({'source':str(source),'archive':str(target),'format':'plain','sha256':expected})
    assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
    linux={'status':'complete_bounded_frozen_linux_insufficient_anchor_recovery_gate','tests':sum(counts),'suite_counts':counts,'artifacts':artifacts,'snapshot_files':len(manifest['file_hashes']),'scope':'Linux aarch64 decode-feature generic execution on exact frozen repaired snapshot. Compressed ELF archives roundtrip to SHA-256 of each executed program. No Linux Torch execution.'}
    LINUX_AUDIT.write_text(json.dumps(linux,indent=2)+'\n')
    report={'status':'complete_bounded_frozen_insufficient_anchor_recovery_qualification','real_model_first_pair_supported':True,'linux_generic_tests':sum(counts),'macos_root_compatibility_tests_before_equivalent_cleanup':35,'source_manifest':str(MANIFEST.relative_to(ROOT)),'source_manifest_sha256':sha(MANIFEST),'linux_audit':str(LINUX_AUDIT.relative_to(ROOT)),'linux_audit_sha256':sha(LINUX_AUDIT),'native_pair_audit':str(PAIR_AUDIT.relative_to(ROOT)),'native_pair_audit_sha256':sha(PAIR_AUDIT),'all_25_model_attempts_completed':True,'insufficient_model_attempts':1,'exact_previous_native_geometry_and_pixel_proof':True,'managed_release_verified':True,'managed_peak_matches_native_first_alternative':True,'acceptance_parameters_unchanged':True,'limits':['One real repaired pair plus bounded generic regression; full repaired229 still required','Native internal six-recipe inconclusive continuation not qualified','All-query negative precision, automatic projected search and large indexed collection remain open','Other-platform Torch and total tensor/workspace RSS admission remain open']}
    OUT.write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['linux_generic_tests'])

if __name__=='__main__':main()
