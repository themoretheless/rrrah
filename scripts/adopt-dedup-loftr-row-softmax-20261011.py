#!/usr/bin/env python3
"""Adopt only the measured candidate and its tested boundary regression."""
import hashlib,json,re,statistics
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 repeat_path=D/'dedup-loftr-row-softmax-repeat-20261011.json';repeat=json.loads(repeat_path.read_text());assert repeat['status']=='complete_bounded_repeated_memory_comparison' and len(repeat['results'])==8;assert all(sha(p)==h for p,h in repeat['pins'].items())
 for row in repeat['results']:
  assert sha(row['log'])==row['log_sha256'];raw=Path(row['log']).read_text();value,tail=json.JSONDecoder().raw_decode(raw);assert value['managed_used']==0 and value['pairs'][0]['evidence']['supported'] is True;assert row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',raw[tail:]).group(1))
 comparison=[]
 for direction in ['forward','reverse']:
  rows=[v for v in repeat['results'] if v['direction']==direction];a=[v for v in rows if v['variant']=='baseline'];b=[v for v in rows if v['variant']=='candidate'];assert len(a)==len(b)==2;assert max(v['rss_bytes'] for v in b)<min(v['rss_bytes'] for v in a)
  comparison.append({'direction':direction,'baseline_rss_median':statistics.median(v['rss_bytes'] for v in a),'candidate_rss_median':statistics.median(v['rss_bytes'] for v in b),'baseline_seconds_median':statistics.median(v['elapsed_seconds'] for v in a),'candidate_seconds_median':statistics.median(v['elapsed_seconds'] for v in b)})
 manifest_path=D/'dedup-loftr-row-softmax-snapshot-20261011.json';manifest=json.loads(manifest_path.read_text());src=Path(manifest['snapshot']);parent=json.loads(Path(manifest['parent_manifest']).read_text());assert all(sha(src/f)==h for f,h in manifest['file_hashes'].items())
 harness=T/'rrrah-loftr-row-softmax-test-harness-20261011';assert all(sha(p)==sha(harness/'src'/p.relative_to(src/'vendor/loftr/src')) for p in (src/'vendor/loftr/src').rglob('*.rs') if p.relative_to(src/'vendor/loftr/src').as_posix()!='coarse_matching/tests.rs')
 oldtests=(src/'vendor/loftr/src/coarse_matching/tests.rs').read_text();assert (harness/'src/coarse_matching/tests.rs').read_text().startswith(oldtests)
 log=D/'dedup-loftr-row-softmax-all-tests-20261011.log';assert '44 passed; 0 failed; 0 ignored' in log.read_text()
 paths=['vendor/loftr/src/coarse_matching.rs','vendor/loftr/src/coarse_matching/tests.rs'];assert all(sha(R/f)==parent['file_hashes'][f] for f in paths)
 out=D/'dedup-loftr-row-softmax-adoption-20261011.json';assert not out.exists()
 sources=[src/paths[0],harness/'src/coarse_matching/tests.rs'];before={f:sha(R/f) for f in paths}
 for f,s in zip(paths,sources): (R/f).write_bytes(s.read_bytes());assert sha(R/f)==sha(s)
 pins={str(p):sha(p) for p in [repeat_path,manifest_path,log,Path(__file__),harness/'Cargo.toml',harness/'Cargo.lock',T/'rrrah-loftr-row-softmax-tests-qualified-20261011/loftr_memory_controls']};pins.update({str(p):sha(p) for p in (harness/'src').rglob('*.rs')})
 out.write_text(json.dumps({'status':'adopted_bounded_memory_reduction','pins':pins,'before':before,'adopted':{f:sha(R/f) for f in paths},'comparisons':comparison,'tests':44,'scope':'CPU Torch2.10/tch0.23 macOS, one original-resolution publisher positive both directions; exact geometry/pixel evidence preserved. No hard total RSS cap or full corpus/runtime parity claim. Sequential-window candidate rejected.'},indent=2)+'\n');print(json.dumps(comparison))
if __name__=='__main__':main()
