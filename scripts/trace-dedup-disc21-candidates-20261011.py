#!/usr/bin/env python3
"""Isolated diagnostic overlay; never changes production thresholds or sources."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-disc21-bank-diagnostic-native-snapshot-20261011.json';m=json.loads(parent.read_text());old=Path(m['snapshot']);s=T/'rrrah-disc21-candidate-trace-snapshot-20261011';h=T/'rrrah-disc21-candidate-trace-harness-20261011';out=D/'dedup-disc21-candidate-trace-20261011.json'
 assert not s.exists() and not h.exists() and not out.exists()
 for f,v in m['file_hashes'].items():
  assert sha(old/f)==v,f
  q=s/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(old/f,q)
 f=s/'crates/rrrah-dedup/src/model_banks.rs';text=f.read_text()
 a='for radius in radii {\n                    if stopped()';b='eprintln!("TRACE candidate bank={} witnesses={} transform={:?}", candidate.hypothesis_bank, candidate.witnesses.len(), candidate.transform.matrix);\n                for radius in radii {\n                    if stopped()';assert text.count(a)==1;text=text.replace(a,b)
 a='Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))=>continue,';b='Err(AnchorRankFileError::Rank(LocalRankError::InsufficientGeometry))=>{eprintln!("TRACE radius={} insufficient_geometry", radius);continue},';assert text.count(a)==1;text=text.replace(a,b)
 a='if pixels.supported {\n                        if candidate';b='eprintln!("TRACE radius={} pixels={:?}", radius, pixels);\n                    if pixels.supported {\n                        if candidate';assert text.count(a)==1;f.write_text(text.replace(a,b))
 h.mkdir();oldh=T/'rrrah-disc21-bank-diagnostic-native-harness-20261011';(h/'Cargo.toml').write_text((oldh/'Cargo.toml').read_text().replace(str(old),str(s)));shutil.copy2(oldh/'Cargo.lock',h/'Cargo.lock');q=h/'src/bin/disc21_candidate_trace_files.rs';q.parent.mkdir(parents=True);shutil.copy2(oldh/'src/bin/disc21_bank_diagnostic_files.rs',q)
 pins={str(parent):sha(parent),str(Path(__file__)):sha(__file__)}
 pins.update({str(s/k):sha(s/k) for k in m['file_hashes']});pins.update({str(p):sha(p) for p in [h/'Cargo.toml',h/'Cargo.lock',q]})
 env=os.environ.copy();runtime=T/'rrrah-loftr-native-runtime-20261010';env.update(LIBTORCH_USE_PYTORCH='1',PATH=str(runtime/'bin')+':'+env['PATH'])
 log=T/'dedup-disc21-candidate-trace-build-20261011.log';r={'status':'building','pins':pins,'scope':'Diagnostic-only stderr overlay on pinned source: same candidates, thresholds, radii and decisions; no production fix or coverage claim.'}
 def save():out.write_text(json.dumps(r,indent=2)+'\n')
 save()
 with log.open('w') as stream:
  p=subprocess.run(['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin','disc21_candidate_trace_files','--locked','--offline','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')],cwd='/tmp',env=env,stdout=stream,stderr=subprocess.STDOUT)
 r['build_returncode']=p.returncode;r['build_log']=str(log);r['build_log_sha256']=sha(log);save();assert p.returncode==0
 binary=T/'rrrah-dedup-exact-portfolio-20261011/debug/disc21_candidate_trace_files';pins[str(binary)]=sha(binary)
 original=D/'dedup-disc21-bank-diagnostic-native-20261011.json';data=json.loads(original.read_text());assert all(sha(k)==v for k,v in data['pins'].items());pins[str(original)]=sha(original);cmd=data['command'].copy();cmd[4]=str(binary);r.update(status='running',command=cmd);save();start=time.monotonic();p=subprocess.run(cmd,capture_output=True,text=True);r.update(status='complete' if p.returncode==0 else 'execution_failed',returncode=p.returncode,stdout=p.stdout,stderr=p.stderr,elapsed_seconds=time.monotonic()-start)
 assert all(sha(k)==v for k,v in pins.items());save();print(p.stdout+p.stderr,flush=True)
if __name__=='__main__':main()
