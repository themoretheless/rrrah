from pathlib import Path
import os,subprocess,tempfile,json
root=Path.cwd();source=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho/43-scripts--hiho';results=[]
with tempfile.TemporaryDirectory(prefix='hiho098-launcher-',dir=root/'.dedup-work') as tmp:
 base=Path(tmp);cwd=base/'working';cwd.mkdir();hadoop=base/'hadoop';(hadoop/'bin').mkdir(parents=True)
 launcher=hadoop/'bin/hadoop';launcher.write_text('#!/bin/bash\nprintf "%s\\n" "$HIHO_HOME" "$@" > "$OWNED_CAPTURE"\n');launcher.chmod(0o755)
 def run(label,home,hadoop_home):
  capture=base/(label+'.txt');env=os.environ.copy();env.pop('bin',None);env.pop('HIHO_HOME',None);env['HADOOP_HOME']=str(hadoop_home);env['OWNED_CAPTURE']=str(capture)
  if home is not None:env['HIHO_HOME']=str(home)
  p=subprocess.run(['/bin/bash',str(source),'dedup','-inputPath','name with spaces'],cwd=cwd,env=env,capture_output=True,text=True,timeout=10)
  rows=capture.read_text().splitlines() if capture.exists() else []
  results.append(dict(case=label,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr,captured=rows));return p,rows
 (cwd/'deploy').mkdir();(cwd/'deploy/hiho.jar').touch()
 p,rows=run('automatic-home',None,hadoop);assert p.returncode==0 and rows[0]==str(cwd) and rows[1:4]==['jar',str(cwd/'deploy/hiho.jar'),'co.nubetech.hiho.dedup.DedupJob'] and rows[-2:]==['-inputPath','name with spaces']
 release=base/'release with spaces';(release/'deploy').mkdir(parents=True);(release/'deploy/hiho.jar').touch();p,rows=run('space-in-hiho-home',release,hadoop);assert p.returncode==0 and str(release/'deploy/hiho.jar') not in rows and rows[2:5]==[str(base/'release'),'with','spaces/deploy/hiho*.jar']
 hadoop_spaces=base/'hadoop with spaces';(hadoop_spaces/'bin').mkdir(parents=True);(hadoop_spaces/'bin/hadoop').write_bytes(launcher.read_bytes());(hadoop_spaces/'bin/hadoop').chmod(0o755)
 p,rows=run('space-in-hadoop-home',cwd,hadoop_spaces);assert p.returncode!=0 and not rows
out=dict(scope='Original launcher executed via bash with owned recording stub replacing Hadoop executable; no Java/Hadoop job, no external service;3authorcontrols; -inputPath value with spaces survives quoted "$@"',checks=3,results=results)
(root/'docs/research/dedup-500/hiho098-launcher-controls-20261024.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
