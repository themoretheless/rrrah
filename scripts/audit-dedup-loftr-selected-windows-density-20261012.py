#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'dedup-loftr-selected-windows-density-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_density_controls_not_adopted' and r['build_returncode']==0;assert all(sha(p)==h for p,h in r['pins'].items())
 old=T/'rrrah-loftr-selected-windows-test-harness-v2-20261012';h=T/'rrrah-loftr-selected-windows-density-harness-20261012';changed=[]
 for p in old.rglob('*'):
  if p.is_file() and sha(p)!=sha(h/p.relative_to(old)):changed.append(str(p.relative_to(old)))
 assert changed==['src/fine_preprocess/tests.rs']
 code=(old/changed[0]).read_text();expected=code.replace('let side=320_i64;let count=256_i64;','let side=320_i64;let count=std::env::var("RRRAH_SELECTED_WINDOW_COUNT").unwrap().parse::<i64>().unwrap();assert!((1..=12800).contains(&count));').replace('Tensor::arange(count,(Kind::Int64,Device::Cpu))*23','(Tensor::arange(count,(Kind::Int64,Device::Cpu))*23).remainder(6400)').replace('Tensor::arange(count,(Kind::Int64,Device::Cpu))*19','(Tensor::arange(count,(Kind::Int64,Device::Cpu))*19).remainder(6400)');assert expected==(h/changed[0]).read_text()
 rows=r['results'];assert [(x['count'],x['mode']) for x in rows]==[(n,mode) for n in [256,6400,12800] for mode in ['dense','selected']]
 comparisons=[]
 for row in rows:
  assert row['returncode']==0 and '1 passed; 0 failed' in row['stdout'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];assert row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1]);assert row['hash']==re.search(r'hash=([0-9a-f]{16})',row['stdout'])[1]
 for a,b in zip(rows[::2],rows[1::2]):
  assert a['hash']==b['hash'];comparisons.append({'selected_windows':a['count'],'available_windows':6400,'dense_rss_bytes':a['rss_bytes'],'selected_rss_bytes':b['rss_bytes'],'output_fnv64':a['hash']})
 out=D/'dedup-loftr-selected-windows-density-audit-20261012.json';assert not out.exists();pins=dict(r['pins']);pins.update({str(p):sha(p) for p in [rp,Path(__file__)]});out.write_text(json.dumps({'status':'verified_six_fresh_sparse_dense_repeated_stage_controls_not_adopted','comparisons':comparisons,'pins':pins,'scope':'Production identical to tested v2 candidate; only manual benchmark accepts fixed counts and modulo6400 indices. Covers sparse, all positions and twice as many selected windows with repeats. Same noncryptographic fingerprints, not full raw cryptographic output equality. One fresh process per variant/density, concurrent workload. No general memory bound, positive recall, full-model/collection gain or adoption.'},indent=2)+'\n');print(comparisons)
if __name__=='__main__':main()
