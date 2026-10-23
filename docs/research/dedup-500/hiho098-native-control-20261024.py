from pathlib import Path
import subprocess,shutil,json
root=Path.cwd();base=root/'.dedup-work/hiho098-runtime-20261024';jdk=next(base.glob('*/Contents/Home'));e=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';src=base/'src';src.mkdir(exist_ok=True);classes=base/'classes';classes.mkdir(exist_ok=True)
for name in ['06-HashUtility.java','07-HihoTuple.java','12-TestHashUtility.java','13-TestHihoTuple.java','14-DelimitedLineRecordReader.java','15-DelimitedTextInputFormat.java','16-TestDelimitedLineRecordReader.java','17-TestDelimitedTextInputFormat.java']:
 shutil.copyfile(e/name,src/name.split('-',1)[1])
cp=str(base/'jars/*');commands=[]
def run(args):
 p=subprocess.run([str(x) for x in args],capture_output=True,text=True,timeout=60);commands.append(dict(args=[str(x) for x in args],returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr);return p
p=run([jdk/'bin/javac','-cp',cp,'-d',classes,*sorted(src.glob('*.java'))]);assert p.returncode==0
p=run([jdk/'bin/java','-cp',str(classes)+':'+cp,'org.junit.runner.JUnitCore',*['co.nubetech.hiho.dedup.'+n for n in ['TestHashUtility','TestHihoTuple','TestDelimitedLineRecordReader','TestDelimitedTextInputFormat']]])
out=root/'docs/research/dedup-500/hiho098-native-original-tests-20261024.json';out.write_text(json.dumps(dict(scope='23 original JUnit methods in4 original files; unchanged selected8sourcefiles; real Hadoop0.20.2/Java21.0.13; no cluster or full upstream suite',commands=commands),indent=2)+'\n');assert p.returncode==0
