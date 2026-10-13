#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1]; D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-joint-expanded-window-fixed-20261011.json';a=T/'rrrah-joint-expanded-window-fixed-qualified-20261011';assert not out.exists() and not a.exists();a.mkdir()
 parent=D/'dedup-joint-window-roster-audit-20261011.json';audit=json.loads(parent.read_text());assert all(sha(p)==h for p,h in audit['pins'].items())
 base=D/'dedup-joint-model-window-roster-controls-20261011.json';old=json.loads(base.read_text());row=old['plan'][1];src=T/'rrrah-dedup-exact-portfolio-20261011/debug/joint_model_expanded_window';binary=a/src.name;shutil.copy2(src,binary)
 h=T/'rrrah-joint-expanded-window-harness-20261011';paths=[parent,base,src,binary,Path(__file__),h/'Cargo.toml',h/'Cargo.lock',h/'src/bin/joint_model_expanded_window.rs'];pins={str(p):sha(p) for p in paths};pins.update(old['pins'])
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'plan':row,'results':[],'scope':'Reverse203502→203500 fixed filter12,1m hypotheses, window radii6/12. Only center-window coverage changes; contrast/agreement/minimum1000 pairs unchanged. Diagnostic not automatic policy or precision qualification.'}
 def save():out.write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
 save()
 for radius in [6,12]:
  assert all(sha(p)==v for p,v in pins.items());cmd=old['results'][1]['command'][:];cmd[4]=str(binary);assert cmd[4]==str(binary);cmd.append(str(radius));v={'window_radius':radius,'command':cmd,'status':'running'};report['results'].append(v);save();start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);v['pid']=p.pid;save();stdout,stderr=p.communicate();log=a/f'radius{radius}.log';log.write_text(stdout+stderr);v.update(returncode=p.returncode,stdout=stdout,stderr=stderr,log=str(log),log_sha256=sha(log),elapsed_seconds=time.monotonic()-start);save();assert p.returncode==0,stderr;value=json.loads(stdout);assert value['managed_used']==0;v['status']='supported' if value['supported'] else 'inconclusive';save();print(radius,v['status'],flush=True)
 report['status']='complete_bounded_expanded_windows';save()
if __name__=='__main__':main()
