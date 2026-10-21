from pathlib import Path
import subprocess
root=Path.cwd();src=root/'.dedup-work/visualvault076-20261021/crates/utils/src/progress.rs';code=f'#[path="{src}"] mod progress;\n'+'''
#[test] fn control_completed_progress_stays_true_when_total_increases(){
 let mut p=progress::Progress::new();p.set_total(10);p.set_current(10);assert!(p.is_complete);p.set_total(20);assert_eq!(p.percentage(),50.);assert!(p.is_complete);
}
#[test] fn control_completed_progress_stays_true_when_current_decreases(){
 let mut p=progress::Progress::new();p.set_total(10);p.set_current(10);p.set_current(1);assert_eq!(p.percentage(),10.);assert!(p.is_complete);
}
''';out=root/'.dedup-work/visualvault076-progress-tests-20261021.rs';out.write_text(code);binary=root/'.dedup-work/visualvault076-progress-tests-20261021';subprocess.run(['rustc','+1.98.1','--edition=2021','--test',str(out),'-o',str(binary)],check=True)
with (root/'docs/research/dedup-500/visualvault076-progress-tests-20261021/run.log').open('w') as f:subprocess.run([str(binary)],stdout=f,stderr=subprocess.STDOUT,check=True)
