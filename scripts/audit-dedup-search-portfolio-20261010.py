"""Record completed pair-search qualification, preserving prior evidence and full scope."""
import hashlib
import json
from pathlib import Path
import re
import shutil

D = Path('docs/research')
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest = D / 'dedup-search-portfolio-full-snapshot-20261010.json'
s = json.loads(manifest.read_text())
assert all(sha(p) == h and sha(Path(s['snapshot'])/p) == h for p,h in s['file_hashes'].items())
measurement = D / 'dedup-search-portfolio-full-measurement-20261010.json'
m = json.loads(measurement.read_text())
assert m['status'] == 'complete_five_pair_automatic_search_cpu_qualification'
assert len(m['results']) == 5 and all(r['returncode'] == 0 for r in m['results'])
assert all(sha(p) == h for p,h in m['pins'].items())
mac = D / 'dedup-search-portfolio-final-tests-20261010.log'
linux = D / 'dedup-search-portfolio-full-linux-tests-20261010.log'
for log in (mac,linux):
    assert sorted(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed',log.read_text()))) == [3,5,7]
runtime = D / 'dedup-loftr-native-20261010-runtime-pins.json'
assert all(sha(p) == h for p,h in json.loads(runtime.read_text())['pins'].items())
prior = json.loads((D/'dedup-loftr-library-f32-source-pins-20261010.json').read_text())
archive = Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-library-f32-before-portfolio-20261010')
preserved = {p:str(archive/p) for p in ('crates/rrrah-dedup/src/lib.rs','crates/rrrah-dedup/src/model_compare.rs','crates/rrrah-dedup/src/model_loftr.rs')}
assert all(sha(path) == prior['file_hashes'][p] for p,path in preserved.items())
linux_binaries = []
for executable in re.findall(r'Running tests/\S+ \(([^)]+)\)',linux.read_text()):
    host = Path(executable.replace('/validation','/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native',1))
    target = T / ('search-portfolio-linux-'+host.name.split('-')[0]+'-qualified-20261010')
    assert not target.exists()
    shutil.copy2(host,target)
    linux_binaries.append(target)
assert len(linux_binaries) == 3
files = [manifest,measurement,mac,linux,runtime,Path(__file__),D/'dedup-search-portfolio-full-native-build-20261010.log',D/'dedup-search-portfolio-linux-tests-20261010.log',Path('/Users/themoretheless/.codex/tmp/rrrah-search-portfolio-linux-20261010.sh'),T/'loftr-search-portfolio-full-qualified-20261010',T/'search-portfolio-tests-qualified-20261010',T/'search-portfolio-model-tests-qualified-20261010',T/'search-portfolio-native-tests-qualified-20261010',*linux_binaries,*map(Path,preserved.values())]
remaining = ['Automatic collection-wide native/learned search and complete real-collection qualification','Hard inference tensor/workspace/RSS admission; kernel preemption/latency','Crumpled-photo case 200302','Alpha/HDR learned input, broad format/corpus/platform qualification; Windows and Linux LoFTR runtime unqualified','Native-first recovery/performance for the original large rephoto not established by these pair controls']
a = dict(status='complete_guarded_native_learned_pair_search_qualification',apis=['search_portfolio::search_portfolio_files_with','model_loftr::search_loftr_portfolio_files'],mac_tests=15,linux_aarch64_tests=15,new_portfolio_tests=5,real_cpu_comparisons=5,rephoto_model_first_complete_model_evidence_parity=True,source_file_count=len(s['file_hashes']),sources_unchanged=True,peak_process_rss_bytes=max(r['process_peak_rss_bytes'] for r in m['results']),superseded_qualified_source_preserved=preserved,initial_linux_snapshot_failure={'report':'dedup-search-portfolio-linux-tests-20261010.log','reason':'Incomplete packaging omitted current decoder dependency modules. Full snapshot adds all current dependency files and is independently compiled/tested on Linux.','added_paths':s['added_paths']},pins={str(p):sha(p) for p in files},scope='Pair API integration. Five macOS CPU weight-backed controls: two large rephoto model-first sizes, ordinary/mirror native early support, foreign native miss then model miss. Five generic portfolio tests plus seven model lifecycle and three native portfolio tests per platform. Linux tests use decode only, not libtorch. Root tests use root lock; native weight-backed diagnostic uses standalone probe lock and optimized dev profile. No controlled latency, full collection/corpus/format coverage or hard total RSS claim.',remaining=remaining)
out = D / 'dedup-search-portfolio-audit-20261010.json'
assert not out.exists()
out.write_text(json.dumps(a,indent=2)+'\n')
p = D / 'dedup-six-searches-state.json'
state = json.loads(p.read_text())
state['automatic_native_loftr_pair_search_20261010'] = dict(status=a['status'],audit=str(out),mac_tests=15,linux_generic_tests=15,real_comparisons=5,automatic_pair_portfolio_integrated=True,automatic_collection_integrated=False,hard_total_rss_admission=False,remaining=remaining)
old = state.get('loftr_library_provider_20261010')
if old:
    old.update(automatic_portfolio_integrated=True,automatic_collection_integrated=False,portfolio_qualification_audit=str(out),remaining=remaining)
p.write_text(json.dumps(state,indent=2,ensure_ascii=False)+'\n')
print(a['status'],a['mac_tests'],a['linux_aarch64_tests'],a['real_cpu_comparisons'])
