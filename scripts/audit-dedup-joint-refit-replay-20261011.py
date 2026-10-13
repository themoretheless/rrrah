#!/usr/bin/env python3
import hashlib,json,math,re,statistics,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def apply(m,p):
 x,y=p;z=m[2][0]*x+m[2][1]*y+m[2][2];return [(m[i][0]*x+m[i][1]*y+m[i][2])/z for i in range(2)]
def inverse(m):
 a,b,c=m[0];d,e,f=m[1];g,h,i=m[2];return [[e*i-f*h,c*h-b*i,b*f-c*e],[f*g-d*i,a*i-c*g,c*d-a*f],[d*h-e*g,b*g-a*h,a*e-b*d]]
def residuals(m,points):
 inv=inverse(m);s=[];t=[]
 for p in points:s.append(math.dist(apply(inv,p['target']),p['source']));t.append(math.dist(apply(m,p['source']),p['target']))
 return {'source_max':max(s),'source_median':statistics.median(s),'target_max':max(t),'target_median':statistics.median(t),'normalized_squared_error':math.fsum(x*x for x in s+t)/4}
def main():
 out=D/'dedup-joint-refit-replay-audit-20261011.json';assert not out.exists();parent=D/'dedup-exposure-replay-audit-20261011.json';pr=json.loads(parent.read_text());assert all(sha(p)==h for p,h in pr['pins'].items())
 ip=D/'dedup-exposure-replay-input-20261011.json';inp=json.loads(ip.read_text());op=D/'dedup-joint-refit-replay-output-20261011.json';obs=json.loads(op.read_text());assert obs['managed_used']==0;rows=obs['evidence']['rows'];assert len(rows)==24
 baseline=json.loads((D/'dedup-exposure-replay-output-20261011.json').read_text())['evidence']['rows'];summaries=[]
 for case in inp['cases']:
  name=case['name'];r=[x for x in rows if x['case']==name];assert len(r)==8
  for row in r:
   counts=[tuple(map(int,x))for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',row['result'])];assert len(counts)==2;assert row['supported']==all(n>=1000 and a/n>=0.9 for n,a in counts)
   if not row['refined']:
    old=next(x for x in baseline if x['case']==name and x['calibrated']==row['calibrated'] and x['rank']==row['rank']);assert row['result']==old['result'] and row['supported']==old['supported']
  before=next(x for x in r if not x['refined']);after=next(x for x in r if x['refined']);a=residuals(before['matrix'],case['witnesses']);b=residuals(after['matrix'],case['witnesses']);assert b['source_max']<=2 and b['target_max']<=2
  expected=float(re.search(r'normalized_squared_error: ([^,]+)',after['refit']).group(1));assert math.isclose(b['normalized_squared_error'],expected,rel_tol=1e-9,abs_tol=1e-10);assert 'hypotheses: 4096' in after['refit']
  ids=list(map(int,re.search(r'inliers: \[([^]]+)\]',after['refit']).group(1).split(',')));assert ids==list(range(len(case['witnesses'])))
  shift=max(math.dist(apply(before['matrix'],p['source']),apply(after['matrix'],p['source']))for p in case['witnesses'])
  unchanged=all(x['result']==next(y['result'] for y in r if not y['refined'] and y['rank']==x['rank'] and y['calibrated']==x['calibrated'])for x in r if x['refined'])
  summaries.append({'case':name,'witnesses':len(ids),'before':a,'after':b,'maximum_target_shift_at_witness':shift,'pixel_evidence_unchanged':unchanged})
 h=T/'rrrah-joint-refit-replay-harness-20261011';source=h/'src/main.rs';assert sha(source)==sha(R/'scripts/dedup-joint-refit-replay/src/main.rs');text=source.read_text()
 for literal in ['trials:4096','seed:173','min_inliers:n','source_tolerance:2.0','minimum_contrast:0.005','minimum_agreement:0.9']:assert literal in text
 log=D/'dedup-joint-refit-replay-build-20261011.log';assert 'Finished `dev` profile' in log.read_text();saved=T/'rrrah-joint-refit-replay-qualified-20261011';assert not saved.exists();saved.mkdir();binary=T/'rrrah-opposite-native-snapshot-20261011/target/debug/dedup-joint-refit-replay';shutil.copy2(binary,saved/binary.name)
 paths=[parent,ip,op,source,h/'Cargo.toml',h/'Cargo.lock',log,D/'dedup-joint-refit-replay-runtime-20261011.log',saved/binary.name,Path(__file__)]
 report={'status':'verified_joint_witness_refit_replay_no_recovery_or_precision_gain','pins':{str(p):sha(p)for p in paths},'summaries':summaries,'scope':'Existing joint projective library fitter runs4096 trials/seed173 on each saved full witness set, requiring all witnesses and source/target residuals<=2 under valid whole-image domains. Independent residual/score calculation; unchanged original-policy replay exact baseline parity. Correct bank7 and misplaced bank12 retain identical original-pixel evidence, including calibrated rank2 false support. No model extraction rerun, automatic pipeline adoption, performance or general precision qualification. Geometry Vec and diagnostic JSON external to managed payload; original JPEG byte guards and calibrated scratch lifetime admission preserved.'}
 out.write_text(json.dumps(report,indent=2)+'\n');print(report['status']);print(summaries)
if __name__=='__main__':main()
