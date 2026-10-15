"""Independently check retained executable, source spans, fixture and both native runs."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();p=D/'dedup-near41-hamming-corpus-v2-20261015.json';a=json.loads(p.read_text());assert a['status']=='passed_twice_actual_hamming_core_with_independent_fullscan'
for file,sha in a['source_pins'].items():assert h(R/file)==sha
for file,sha in a['dependency_pins'].items():assert h(file)==sha
for file,sha in a['logs'].items():assert h(file)==sha
assert h(a['binary'])==a['binary_sha256'] and h(a['harness'])==a['harness_sha256']
W=Path(a['binary']).parent;s=(R/'crates/rrrah-dedup/src/lib.rs').read_text();l=(R/'crates/rrrah-dedup/src/local.rs').read_text();core=s[s.index('/// An indexed candidate'):s.index('\n#[cfg(test)]\nmod tests')];error=l[l.index('#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]'):l.index('\n/// Extract deterministic')];helper=l[l.index('pub(crate) fn reserve_slot'):l.index('\n#[allow(clippy::too_many_lines)]')]
assert Path(a['harness']).read_text()=='extern crate self as rrrah_dedup;\nmod local {\n'+error+'\n'+helper+'\n}\n'+core+'\n#[path="hamming_image_corpus.rs"]mod image_corpus;\n'
assert (W/'hamming_image_corpus.rs').read_bytes()==(R/'crates/rrrah-dedup/tests/hamming_image_corpus.rs').read_bytes();assert (W/'fixtures/near41-phash64.txt').read_bytes()==(R/'crates/rrrah-dedup/tests/fixtures/near41-phash64.txt').read_bytes()
fixture=[int(s,16) for s in (W/'fixtures/near41-phash64.txt').read_text().splitlines()];corpus=json.loads((D/'dedup-500/near41-visual-corpus-prepared-20261015.json').read_text());assert fixture==[r['hash_integer'] for r in corpus['records']]
# Deliberately use a different bit-count implementation from Rust count_ones.
edges=sum(bin(x^y).count('1')<=8 for i,x in enumerate(fixture) for j,y in enumerate(fixture) if i!=j);assert edges==2902
names=['image_corpus::image_hash_corpus_matches_full_scan_without_top_k_truncation'];inventory=subprocess.check_output([a['binary'],'--list'],text=True);assert re.findall(r'^(.*): test$',inventory,re.M)==names;assert inventory==(W/'inventory.log').read_text()
for run in ['run1','run2']:
 text=(W/(run+'.log')).read_text();assert re.findall(r'^test (\S+) \.\.\. ok$',text,re.M)==names;assert '1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in text
assert a['complete_queries_per_run']==540*8*3==a['result_budget_refusals_per_run'];assert h(a['binary'])==a['binary_sha256']
out=D/'dedup-near41-hamming-corpus-audit-20261015.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_scoped_current_hamming_corpus_regression','producer_report':str(p.relative_to(R)),'producer_report_sha256':h(p),'auditor':str(Path(__file__).relative_to(R)),'auditor_sha256':h(Path(__file__)),'binary_available_and_hash_verified':True,'source_spans_exactly_unchanged':True,'fixture_paths':540,'directed_hamming8_edges':edges,'runs':2,'complete_queries_per_run':12960,'budget_refusals_per_run':12960,'missing_edges':0,'scope':a['scope'],'historical_preformat_run':'docs/research/dedup-near41-hamming-corpus-20261015.json'},indent=2)+'\n');print('Independent native artifact/source/fixture/fullscan audit passed')
