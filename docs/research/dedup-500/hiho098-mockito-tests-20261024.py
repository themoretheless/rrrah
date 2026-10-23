from pathlib import Path
import subprocess,shutil,json
root=Path.cwd();b=root/'.dedup-work/hiho098-runtime-20261024';jdk=next(b.glob('*/Contents/Home'));f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';cp=str(b/'classes')+':'+str(b/'jars/*');sources=[];tests=[]
for local,package in [('18-TestDedupKeyMapper.java','dedup'),('19-TestDedupKeyReducer.java','dedup'),('20-TestDedupValueMapper.java','dedup'),('21-TestDedupValueReducer.java','dedup'),('31-TestMergeKeyMapper.java','merge'),('32-TestMergeValueMapper.java','merge'),('34-TestMergeKeyReducer.java','merge'),('35-TestMergeValueReducer.java','merge')]:
 name=local.split('-',1)[1];p=b/'src'/name;shutil.copyfile(f/local,p);sources.append(str(p));tests.append('co.nubetech.hiho.'+package+'.'+name[:-5])
steps=[]
for args in [[str(jdk/'bin/javac'),'-cp',cp,'-d',str(b/'classes'),*sources],[str(jdk/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'org.junit.runner.JUnitCore',*tests]]:
 p=subprocess.run(args,capture_output=True,text=True,timeout=60);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if p.returncode:break
out=root/'docs/research/dedup-500/hiho098-mockito-original-tests-20261024.json';out.write_text(json.dumps(dict(scope='28distinct original tests in8unchanged files; Hadoop0.20.2,Junit4.5,Mockito-all1.8.1,Java21with java.lang open for legacy mocks; no Hadoop cluster/full suite',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
