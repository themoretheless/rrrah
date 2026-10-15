"""Verbatim selected pure numeric functions/progress bodies; no GTK application execution."""
from pathlib import Path
import hashlib,json,subprocess,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/fclones34-20261016';S=W/'source/fclones-gui/src';B=W/'selected-controls';B.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();a=(S/'bytes_entry.rs').read_text();numeric=a[a.index('fn bytes_to_scale'):];p=(S/'progress.rs').read_text();progress=p[p.index('pub struct ProgressBar {'):p.index('pub struct LogAdapter {')];(B/'numeric.rs').write_text(numeric);(B/'progress.rs').write_text(progress)
harness=r'''use std::sync::atomic::{AtomicU64,Ordering};
trait ProgressTracker { fn inc(&self,delta:u64); }
include!("numeric.rs");include!("progress.rs");
#[test]fn zero_and_one(){assert_eq!(bytes_to_scale(0),0.0);assert_eq!(scale_to_bytes(0.0),0);assert_eq!(scale_to_bytes(bytes_to_scale(1)),1);}
#[test]fn grid_is_quantized(){assert_eq!(bytes_to_scale(999),bytes_to_scale(1000));assert_eq!(scale_to_bytes(bytes_to_scale(999)),1000);}
#[test]fn known_decimal_powers(){assert_eq!(scale_to_bytes(2.0),100);assert_eq!(scale_to_bytes(3.0),1000);assert_eq!(scale_to_bytes(12.0),1_000_000_000_000);assert_eq!(round(12345.0,2),12000);}
#[test]fn maximum_u64_overflow_policy(){#[cfg(overflow_checked)]assert!(std::panic::catch_unwind(||bytes_to_scale(u64::MAX)).is_err());#[cfg(not(overflow_checked))]{let value=bytes_to_scale(u64::MAX);assert!(value.is_infinite()&&value.is_sign_negative());}}
#[test]fn known_progress_fraction(){let p=ProgressBar::new(Some(10));p.inc(5);assert_eq!(p.progress(),Some(0.5));}
#[test]fn unknown_progress(){let p=ProgressBar::new(None);p.inc(5);assert_eq!(p.progress(),None);}
#[test]fn zero_length_progress_is_nan(){let p=ProgressBar::new(Some(0));assert!(p.progress().unwrap().is_nan());}
#[test]fn progress_is_not_clamped(){let p=ProgressBar::new(Some(10));p.inc(11);assert_eq!(p.progress(),Some(1.1));}
''';(B/'harness.rs').write_text(harness);runs=[]
for label,checked in [('checked',True),('unchecked',False)]:
 exe=B/label;cmd=['rustc','+stable','--edition=2021','--test',str(B/'harness.rs'),'-Dwarnings','-C','opt-level=3','-C','overflow-checks='+('yes' if checked else 'no'),'-o',str(exe)]
 if checked:cmd+=['--cfg','overflow_checked']
 with (B/f'{label}-build.log').open('x') as stream:r=subprocess.run(cmd,stdout=stream,stderr=subprocess.STDOUT)
 assert r.returncode==0,(B/f'{label}-build.log').read_text();inventory=subprocess.check_output([str(exe),'--list'],text=True);(B/f'{label}-inventory.log').write_text(inventory);names=re.findall(r'^(.*): test$',inventory,re.M);assert len(names)==8
 for repeat in [1,2]:
  log=B/f'{label}-run{repeat}.log'
  with log.open('x') as stream:r=subprocess.run([str(exe),'--test-threads=1'],stdout=stream,stderr=subprocess.STDOUT,timeout=20)
  text=log.read_text();assert r.returncode==0 and '8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in text;assert sorted(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))==sorted(names);runs.append({'profile':label,'repeat':repeat,'tests':names,'log':str(log.relative_to(R)),'log_sha256':h(log),'binary':str(exe.relative_to(R)),'binary_sha256':h(exe)})
out=D/'fclones34-numeric-progress-controls-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_selected8controls_two_profiles_twice','whole_repo_complete':False,'producer_sha256':h(__file__),'source_pins':{str(x.relative_to(R)):h(x) for x in [S/'bytes_entry.rs',S/'progress.rs',B/'numeric.rs',B/'progress.rs',B/'harness.rs',*sorted(B.glob('*.log'))]},'runs':runs,'scope':'Verbatim complete3purebytefunctions andProgressBarstruct/3implblocks extracted unchanged. RealstdAtomicU64/floatmath, minimalProgressTracker trait-signature adapter replaces externalfclones trait only.8selectedcontrols checked/uncheckedarithmeticopt3 profiles eachtwice. No GTK/widget/delegate/corefclones/nativeUI/upstreamsuite/fullservice proof.','findings':['Helper bytes_to_scale(u64MAX) panicswithoverflowchecks, negativeinfinitywithout; validGUIcallback reachability nottested','Bytes scale is quantized,999and1000 maptosamegrid andback1000; widgetbidirectional-binding effects notexecuted','Zero-lengthprogress yieldsNaN; progress11/10 yields1.1, unknownNone andnormal.5 behaveasexpected; no GTKrender ofinvalidfraction tested'],'limits':['No userfile deletion/move/link performed, noGUIopened','Fullapplication build unavailable via currentpkgconfig GTK4/libadwaita; depsource outsidepinnedrepository not reviewed','No originalcodecopied into production or performance/adoption claim']},indent=2)+'\n');print('8verbatim selectedcontrols,2arithmeticprofiles,2runs:32passed; noGTKexecution')
