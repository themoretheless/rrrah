#!/usr/bin/env python3
"""Independent two-pass arithmetic check of published native patch samples."""
import hashlib,json,math,statistics,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-aperture-replay-audit-20261011.json';assert not out.exists()
 parent=D/'dedup-exposure-replay-audit-20261011.json';pr=json.loads(parent.read_text());assert all(sha(p)==h for p,h in pr['pins'].items())
 mp=D/'dedup-exposure-calibration-snapshot-20261011.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items())
 ip=D/'dedup-exposure-replay-input-20261011.json';inp=json.loads(ip.read_text());op=D/'dedup-aperture-replay-output-20261011.json';obs=json.loads(op.read_text());assert obs['managed_used']==0
 rows=obs['evidence']['rows'];assert len(rows)==36;assert len({(r['case'],r['source_radius'],r['target_aperture_source_radius']) for r in rows})==36
 baseline=json.loads((D/'dedup-structure-replay-output-20261011.json').read_text())
 for row in rows:
  if row['target_aperture_source_radius']==0.0:
   old=next(v for v in baseline['evidence']['rows'] if v['case']==row['case'] and v['source_radius']==row['source_radius']);assert row['patches']==old['patches']
 summaries=[];pairs_count=0
 for row in rows:
  case=next(c for c in inp['cases'] if c['name']==row['case']);assert row['source_radius'] in [2.,4.,8.];patches=row['patches'];assert len(patches)==len(case['witnesses']);correlations=[]
  for patch,point in zip(patches,case['witnesses']):
   assert patch['source_center']==point['source'];pairs=patch['pairs'];assert 72<=len(pairs)<=81 and patch['pixel_reads_upper']==81*(8 if row['target_aperture_source_radius']==0.0 else 104);pairs_count+=len(pairs)
   assert all(math.isfinite(x) for p in pairs for x in p)
   a,b=zip(*pairs);ma=statistics.mean(a);mb=statistics.mean(b);va=math.fsum((x-ma)**2 for x in a);vb=math.fsum((y-mb)**2 for y in b);cov=math.fsum((x-ma)*(y-mb) for x,y in pairs)
   for x,y in zip([ma,mb,va,vb,cov],patch['means']+patch['energy']+[patch['covariance']]):assert math.isclose(x,y,rel_tol=1e-10,abs_tol=1e-18)
   c=cov/math.sqrt(va*vb) if va>0 and vb>0 else None
   assert c is not None and math.isclose(c,patch['correlation'],rel_tol=1e-10,abs_tol=1e-12);correlations.append(c)
  summaries.append({'case':row['case'],'radius':row['source_radius'],'aperture':row['target_aperture_source_radius'],'patches':len(patches),'correlation_median':statistics.median(correlations),'correlation_min':min(correlations),'correlation_max':max(correlations)})
 assert pairs_count==45516
 med=lambda name,radius,aperture:next(s['correlation_median'] for s in summaries if s['case']==name and s['radius']==radius and s['aperture']==aperture)
 assert med('overlap_bank7',2.,1.)>med('overlap_bank7',2.,0.)
 assert med('rejected_bank12',2.,1.)<med('rejected_bank12',2.,0.)
 assert med('rejected_bank12',8.,1.)>med('overlap_bank7',8.,1.)
 h=T/'rrrah-aperture-replay-harness-20261011';source=h/'src/main.rs';assert sha(source)==sha(R/'scripts/dedup-aperture-replay/src/main.rs')
 text=source.read_text();assert 'for radius in [2.0,4.0,8.0]' in text and 'ls.verify' in text and 'rs.verify' in text
 log=D/'dedup-aperture-replay-build-20261011.log';assert 'Finished `dev` profile' in log.read_text()
 binary=T/'rrrah-opposite-native-snapshot-20261011/target/debug/dedup-aperture-replay';saved=T/'rrrah-aperture-replay-qualified-20261011';assert not saved.exists();saved.mkdir();shutil.copy2(binary,saved/binary.name)
 paths=[D/'dedup-structure-replay-audit-20261011.json',D/'dedup-structure-replay-output-20261011.json',parent,mp,ip,op,source,h/'Cargo.toml',h/'Cargo.lock',log,D/'dedup-aperture-replay-runtime-20261011.log',saved/binary.name,Path(__file__)]
 report={'status':'verified_original_reference_aperture_effect_no_acceptance_gate','pins':{str(p):sha(p) for p in paths},'patches':564,'incomplete_edge_patches':sum(len(p['pairs'])<81 for row in rows for p in row['patches']),'sample_pairs':pairs_count,'summaries':summaries,'adoption':'no_correlation_acceptance_gate','scope':'Original native-decoded JPEGs and saved candidates; reference-only25-tap mapped averaging at source aperture radii0/0.5/1/2. All zero-aperture patch samples/statistics exactly equal previous raw replay. Python independently recomputes two-pass statistics from published averaged pairs, not decoder pixels or individual aperture taps. Aperture1 increases correct bank7 small-window median0.2692 to0.6722 and lowers misplaced bank12 from0.4297 to0.2980; at radius8 misplaced bank12 still has higher correlation. Incomplete reference-edge apertures skip whole sample pairs, without padding or renormalizing partial tap windows; valid sample counts72..81 are published. No threshold chosen and no general recovery/precision claim. Diagnostic vectors/JSON external to managed payload. Original JPEG byte guards span all calls; no sidecars qualified.'}
 out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'],pairs_count)
if __name__=='__main__':main()
