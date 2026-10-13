#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-exact-upstream-adversary-audit-20261011.json';r=json.loads(p.read_text());m=json.loads(Path(r['source_manifest']).read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());assert all(sha(k)==v for k,v in r['pins'].items())
 # The historical producer counted18 source attributes, including one Linux-only
 # fixture. Its executable actually passed all17 macOS tests; preserve that refusal.
 assert r['status']=='failed' and r['error'].endswith('/exact_files.log')
 a=Path(r['error']).parent;binary=a/'exact_files';log=a/'exact_files.log';s=log.read_text();assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',s)==['17']
 assert 'test repeated_exact_scan_rejects_stale_equality_after_rewrite_and_growth ... ok' in s
 entries=[json.loads(x) for x in (a/'build.jsonl').read_text().splitlines()];assert entries[-1]['reason']=='build-finished' and entries[-1]['success'];e=next(x for x in entries if x.get('reason')=='compiler-artifact' and x.get('executable') and x['target']['name']=='exact_files');assert sha(binary)==sha(e['executable'])
 for f in ['crates/rrrah-dedup/src/exact.rs','crates/rrrah-dedup/tests/exact_files.rs']:assert sha(R/f)==m['file_hashes'][f]
 pins=dict(r['pins']);pins.update({str(x):sha(x) for x in [p,binary,log,Path(__file__)]});out=D/'dedup-exact-upstream-adversary-verification-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_17_macos_exact_tests_with_upstream_mutation_adversaries','pins':pins,'original_producer_refusal_preserved':'Expected18 source-level tests but one is Linux-only; actual executable17 passed0failed. No tests changed after execution.','scope':'Pinned unchanged production exact source plus new repeated-scan same-length rewrite and distinct-suffix growth regression. MacOS17 tests; Linux-only fixture and other platforms not qualified by this run.'},indent=2)+'\n');print('Verified17 macOS exact-file tests, original count refusal preserved')
if __name__=='__main__':main()
