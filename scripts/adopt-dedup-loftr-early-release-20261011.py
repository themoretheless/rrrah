#!/usr/bin/env python3
import hashlib,json,re,statistics
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-loftr-early-release-snapshot-20261011.json';m=json.loads(mp.read_text());src=Path(m['snapshot']);assert all(sha(src/f)==h for f,h in m['file_hashes'].items());assert sha(m['producer'])==m['producer_sha256'];assert sha(m['parent_manifest'])==m['parent_sha256']
 ip=D/'dedup-loftr-early-release-20261011.json';initial=json.loads(ip.read_text());rp=D/'dedup-loftr-early-release-repeat-20261011.json';repeat=json.loads(rp.read_text());assert repeat['status']=='complete_bounded_repeated_memory_comparison';assert all(sha(p)==h for p,h in repeat['pins'].items());assert len(repeat['results'])==8;references={v['case']:json.loads(v['stdout']) for v in initial['results']}
 for v in repeat['results']:
  assert sha(v['log'])==v['log_sha256'];raw=Path(v['log']).read_text();value,end=json.JSONDecoder().raw_decode(raw);ref=references[v['variant']+'_'+v['direction']];assert value['pairs']==ref['pairs'] and value['source_paths']==ref['source_paths'] and value['managed_used']==0;assert v['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',raw[end:]).group(1))
 comparisons=[]
 for direction in ['forward','reverse']:
  row={'direction':direction}
  for variant in ['baseline','candidate']:
   values=[v for v in repeat['results'] if v['direction']==direction and v['variant']==variant];assert len(values)==2;row[variant+'_rss_median']=statistics.median(v['rss_bytes'] for v in values);row[variant+'_seconds_median']=statistics.median(v['elapsed_seconds'] for v in values)
  comparisons.append(row)
 assert comparisons[0]['candidate_rss_median']<.98*comparisons[0]['baseline_rss_median'];assert comparisons[1]['candidate_rss_median']<1.01*comparisons[1]['baseline_rss_median']
 h=T/'rrrah-loftr-early-release-formatted-test-harness-20261011';model='vendor/loftr/src/loftr_model.rs'
 for p in (src/'vendor/loftr/src').rglob('*.rs'):
  q=h/'src'/p.relative_to(src/'vendor/loftr/src');assert q.exists()
  if p==src/model:
   a=p.read_text().splitlines();b=q.read_text().splitlines();assert len(a)==len(b) and all(x.lstrip()==y.lstrip() for x,y in zip(a,b)), 'Only leading source indentation may differ from native candidate'
  else: assert sha(p)==sha(q)
 log=D/'dedup-loftr-early-release-formatted-all-tests-20261011.log';assert '44 passed; 0 failed; 0 ignored' in log.read_text()
 parent=json.loads(Path(m['parent_manifest']).read_text());assert sha(R/model)==parent['file_hashes'][model];assert sha(R/'vendor/loftr/src/coarse_matching.rs')==m['file_hashes']['vendor/loftr/src/coarse_matching.rs'];assert sha(R/'vendor/loftr/src/coarse_matching/tests.rs')==m['file_hashes']['vendor/loftr/src/coarse_matching/tests.rs']
 out=D/'dedup-loftr-early-release-adoption-20261011.json';assert not out.exists();before=sha(R/model);(R/model).write_bytes((h/'src/loftr_model.rs').read_bytes());assert sha(R/model)==sha(h/'src/loftr_model.rs')
 pinpaths=[mp,ip,rp,Path(__file__),log,D/'dedup-loftr-early-release-formatted-test-artifacts-20261011.jsonl',D/'dedup-loftr-early-release-formatted-test-build-20261011.log',h/'Cargo.toml',h/'Cargo.lock',T/'rrrah-loftr-early-release-formatted-tests-qualified-20261011/loftr_memory_controls'];pins={str(p):sha(p) for p in pinpaths};pins.update({str(p):sha(p) for p in (h/'src').rglob('*.rs')})
 out.write_text(json.dumps({'status':'adopted_bounded_early_tensor_release','pins':pins,'before':{model:before},'adopted':{model:sha(R/model)},'comparisons':comparisons,'tests':44,'native_source_vs_adopted':'Only leading indentation of the debug expression differs; every source line compared. Final formatted source separately compiled and all44 tests passed.','scope':'One publisher positive both directions; further forward RSS reduction, reverse essentially unchanged. Shared-host timing inconsistent, no speed claim. No hard RSS cap, full corpus or platform parity.'},indent=2)+'\n');print(json.dumps(comparisons))
if __name__=='__main__':main()
