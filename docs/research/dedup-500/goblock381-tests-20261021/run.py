from pathlib import Path
import subprocess,shutil,os
root=Path.cwd();source=root/'.dedup-work/goblock381-20261021';dest=Path('/Users/themoretheless/.codex/tmp/rrrah-goblock381-20261021');shutil.copytree(source,dest,dirs_exist_ok=True)
go=root/'.dedup-work/goblock381-toolchain-20261021/go/bin/go';env=os.environ.copy();env.update({'GOTOOLCHAIN':'local','GOCACHE':str(root/'.dedup-work/goblock381-gocache-20261021'),'GOMODCACHE':str(root/'.dedup-work/goblock381-gomodcache-20261021')})
log=root/'docs/research/dedup-500/goblock381-tests-20261021/run.log'
with log.open('w') as f:
 for args in [['version'],['get','github.com/dsoprea/go-logging@latest','github.com/nfnt/resize@latest','github.com/jessevdk/go-flags@v1.6.1','golang.org/x/image@v0.30.0'],['test','-v','./...']]:
  subprocess.run([str(go),*args],cwd=dest,env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
