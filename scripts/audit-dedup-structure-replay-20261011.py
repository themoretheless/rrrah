#!/usr/bin/env python3
"""Independent two-pass arithmetic check of published native patch samples."""
import hashlib,json,math,statistics,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-structure-replay-audit-20261011.json';assert not out.exists()
 parent=D/'dedup-exposure-replay-audit-20261011.json';pr=json.loads(parent.read_text());assert all(sha(p)==h for p,h in pr['pins'].items())
 mp=D/'dedup-exposure-calibration-snapshot-20261011.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items())
 ip=D/'dedup-exposure-replay-input-20261011.json';inp=json.loads(ip.read_text());op=D/'dedup-structure-replay-output-20261011.json';obs=json.loads(op.read_text());assert obs['managed_used']==0
 rows=obs['evidence']['rows'];assert len(rows)==9;assert len({(r['case'],r['source_radius']) for r in rows})==9
 summaries=[];pairs_count=0
 for row in rows:
  case=next(c for c in inp['cases'] if c['name']==row['case']);assert row['source_radius'] in [2.,4.,8.];patches=row['patches'];assert len(patches)==len(case['witnesses']);correlations=[]
  for patch,point in zip(patches,case['witnesses']):
   assert patch['source_center']==point['source'];pairs=patch['pairs'];assert len(pairs)==81 and patch['pixel_reads_upper']==81*8;pairs_count+=len(pairs)
   assert all(math.isfinite(x) for p in pairs for x in p)
   a,b=zip(*pairs);ma=statistics.mean(a);mb=statistics.mean(b);va=math.fsum((x-ma)**2 for x in a);vb=math.fsum((y-mb)**2 for y in b);cov=math.fsum((x-ma)*(y-mb) for x,y in pairs)
   for x,y in zip([ma,mb,va,vb,cov],patch['means']+patch['energy']+[patch['covariance']]):assert math.isclose(x,y,rel_tol=1e-10,abs_tol=1e-18)
   c=cov/math.sqrt(va*vb) if va>0 and vb>0 else None
   assert c is not None and math.isclose(c,patch['correlation'],rel_tol=1e-10,abs_tol=1e-12);correlations.append(c)
  summaries.append({'case':row['case'],'radius':row['source_radius'],'patches':len(patches),'correlation_median':statistics.median(correlations),'correlation_min':min(correlations),'correlation_max':max(correlations)})
 assert pairs_count==11421
 for radius in [4.,8.]:
  med=lambda name:next(s['correlation_median'] for s in summaries if s['case']==name and s['radius']==radius)
  assert med('rejected_bank12')>med('overlap_bank7')
 h=T/'rrrah-structure-replay-harness-20261011';source=h/'src/main.rs';assert sha(source)==sha(R/'scripts/dedup-structure-replay/src/main.rs')
 text=source.read_text();assert 'for radius in [2.0,4.0,8.0]' in text and 'ls.verify' in text and 'rs.verify' in text
 log=D/'dedup-structure-replay-build-20261011.log';assert 'Finished `dev` profile' in log.read_text()
 binary=T/'rrrah-opposite-native-snapshot-20261011/target/debug/dedup-structure-replay';saved=T/'rrrah-structure-replay-qualified-20261011';assert not saved.exists();saved.mkdir();shutil.copy2(binary,saved/binary.name)
 paths=[parent,mp,ip,op,source,h/'Cargo.toml',h/'Cargo.lock',log,D/'dedup-structure-replay-runtime-20261011.log',saved/binary.name,Path(__file__)]
 report={'status':'verified_original_multiscale_patch_correlation_insufficient_discriminator','pins':{str(p):sha(p) for p in paths},'patches':141,'sample_pairs':pairs_count,'summaries':summaries,'adoption':'no_correlation_acceptance_gate','scope':'Original native-decoded JPEG linear luminance; source-aligned9x9 samples at radii2/4/8 transformed to reference. Python independently recomputes two-pass statistics from published samples; it does not independently decode/recompute native bilinear samples. Samples around existing witnesses are not holdout evidence. No acceptance threshold chosen. Rejected bank12 has higher median correlation than true bank7 at radii4/8, so simple patch correlation does not resolve this false localization. Diagnostic vectors/JSON are outside managed payload accounting. No model rerun, corpus, general precision or performance qualification.'}
 out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'],pairs_count)
if __name__=='__main__':main()
