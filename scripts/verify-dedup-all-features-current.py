"""Qualify terminal all-feature crate tests, with explicit dependency drift scope."""
import json,hashlib,re
from pathlib import Path
root=Path(__file__).resolve().parents[1];base=root/'docs/research';pins_path=base/'dedup-all-features-current-20261008-pins.json';pins=json.loads(pins_path.read_text());digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p,h in pins.items():assert digest(root/p)==h,p
log=base/'dedup-all-features-current-20261008.log';text=log.read_text();assert not re.search(r'^error(?:\[|:)',text,re.M) and 'FAILED' not in text
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text);assert len(results)==81
assert all(int(f)==int(i)==int(m)==int(o)==0 for _,f,i,m,o in results);total=sum(int(n) for n,*_ in results);assert total==491
names=re.findall(r'^test (.+) \.\.\. ok$',text,re.M);assert len(names)==491
assert 'Doc-tests rrrah_dedup' in text and text.rstrip().endswith('test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s')
previous=json.loads((base/'dedup-all-features-current-20261008-state.json').read_text());scope='Terminal native macOS all-feature rrrah-dedup suite:491 tests,0 failures/ignored/filtered.346 pinned core/decode/dedup source,manifest,test/example files remain current. Concurrent unpinned PDF dependency/test changes after compilation are excluded; this does not qualify current entire workspace, Linux decode/corpus, Windows, or all-case duplicate recall/precision.'
state={'status':'verified_terminal_native_all_features_tests','returncode':0,'tests_passed':491,'result_groups':81,'ignored':0,'pinned_files_verified':len(pins),'dependency_scope_limit':previous.get('dependency_scope_limit'),'known_unpinned_dependency_source_drift':previous.get('known_unpinned_dependency_source_drift',[]),'input_hashes':{str(p):digest(p) for p in [pins_path,log,Path(__file__)]},'scope':scope}
(base/'dedup-all-features-current-20261008-state.json').write_text(json.dumps(state,indent=2)+'\n');print('verified terminal all-feature tests',total)
