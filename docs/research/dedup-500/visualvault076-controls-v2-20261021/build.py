from pathlib import Path
import subprocess
root=Path.cwd();s=(root/'.dedup-work/visualvault076-20261021/crates/ui/src/duplicate_detector.rs').read_text();start=s.index('fn truncate_path(');end=s.index('\nfn draw_no_scan',start);s=s[start:end]
s+='''
fn main(){
 let ascii="x".repeat(80);let shortened=truncate_path(&ascii,40);assert_eq!(shortened.len(),40);println!("PASS original ASCII truncation keeps40bytes");
 let unicode=format!("/{}","я".repeat(30));
 std::panic::set_hook(Box::new(|_|{}));
 assert!(std::panic::catch_unwind(||truncate_path(&unicode,40)).is_err());
 println!("PASS original path truncation panics when byte cut splits Cyrillic UTF8");
}
'''
out=root/'.dedup-work/visualvault076-ui-controls-20261021.rs';out.write_text(s);binary=root/'.dedup-work/visualvault076-ui-controls-20261021';subprocess.run(['rustc','+1.98.1','--edition=2021',str(out),'-o',str(binary)],check=True)
with (root/'docs/research/dedup-500/visualvault076-controls-v2-20261021/run.log').open('w') as f:subprocess.run([str(binary)],stdout=f,stderr=subprocess.STDOUT,check=True)
