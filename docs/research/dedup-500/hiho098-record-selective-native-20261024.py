from pathlib import Path
import subprocess,shutil,json
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));b=root/'.dedup-work/hiho098-record-runtime-20261024';src=b/'src';classes=b/'classes';src.mkdir(parents=True,exist_ok=True);classes.mkdir(exist_ok=True);f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';sources=[]
for local in ['86-ColumnInfo.java','87-GenericDBWritable.java','89-TestGenericDBWritable.java','90-DBWritable.java','102-InterfaceAudience.java','103-InterfaceStability.java']:
 p=src/local.split('-',1)[1];shutil.copyfile(f/local,p);sources.append(str(p))
(src/'SelectedRecordTest.java').write_text('import org.junit.runner.*;public class SelectedRecordTest {public static void main(String[] args){Result r=new JUnitCore().run(Request.method(co.nubetech.hiho.mapreduce.lib.db.TestGenericDBWritable.class,"testSetColumns"));System.out.println("Selected original testSetColumns: run="+r.getRunCount()+" failures="+r.getFailureCount());for(Object f:r.getFailures())System.out.println(f);if(r.getRunCount()!=1||!r.wasSuccessful())throw new AssertionError();}}\n')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),*sources,str(src/'SelectedRecordTest.java')],[str(j/'bin/java'),'-cp',cp,'SelectedRecordTest']]:
 p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if p.returncode:break
(root/'docs/research/dedup-500/hiho098-record-selective-original-test-20261024.json').write_text(json.dumps(dict(scope='Exactly1unchangedoriginal testSetColumns selected with JUnit Request.method; DB-dependent testPopulateColumnInfo NOT invoked; no DB connection or SQL',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
