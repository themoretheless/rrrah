from pathlib import Path
import subprocess
root=Path.cwd();old=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-cache-20261021');dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-init-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=(old/'src/main.rs').read_text().split('use database_cache::')[0]+'''
#[tokio::main] async fn main(){
 let cache=database_cache::DatabaseCache::new_uninit();
 assert!(cache.len().await.is_err());
 println!("PASS original lazy placeholder cache has no initialized schema; len returns error");
}
''';(dest/'src/main.rs').write_text(s);(dest/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace('visualvault076-cache','visualvault076-init'))
with (root/'docs/research/dedup-500/visualvault076-init-control-20261021/run.log').open('w') as f:subprocess.run(['cargo','+1.98.1','run','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-cache-target-20261021')],stdout=f,stderr=subprocess.STDOUT,check=True)
