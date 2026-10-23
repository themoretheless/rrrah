from pathlib import Path
import subprocess,json,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));base=root/'.dedup-work/hiho098-format-runtime-20261024';src=base/'src';classes=base/'classes';f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';sources=[]
for local in ['29-HIHOConf.java','86-ColumnInfo.java','87-GenericDBWritable.java','123-GenericDBOutputFormat.java','124-TestGenericDBOutputFormat.java','125-GenericDBLoadDataMapper.java']:
 p=src/local.split('-',1)[1];shutil.copyfile(f/local,p);sources.append(str(p))
(src/'SelectedOutputTest.java').write_text('import org.junit.runner.*;public class SelectedOutputTest{public static void main(String[] args){Result r=new JUnitCore().run(Request.method(co.nubetech.hiho.mapreduce.lib.db.TestGenericDBOutputFormat.class,"testSetColumnInfo"));System.out.println("ORIGINAL run="+r.getRunCount()+" failures="+r.getFailureCount());for(Object f:r.getFailures())System.out.println(f);if(r.getRunCount()!=1||!r.wasSuccessful())System.exit(1);}}\n')
(src/'GenericMapperControl.java').write_text('''import co.nubetech.hiho.mapreduce.GenericDBLoadDataMapper;import co.nubetech.hiho.mapreduce.lib.db.*;import org.apache.hadoop.mapreduce.Mapper;import org.mockito.ArgumentCaptor;import java.util.*;import java.sql.Types;import java.io.IOException;import static org.mockito.Mockito.*;
public class GenericMapperControl{
 static GenericDBLoadDataMapper<String,String> mapper(int middle){GenericDBLoadDataMapper<String,String> m=new GenericDBLoadDataMapper<>();m.setDelimiter(",");ArrayList<ColumnInfo> c=new ArrayList<>();c.add(new ColumnInfo(0,Types.VARCHAR,"left"));c.add(new ColumnInfo(1,middle,"middle"));c.add(new ColumnInfo(2,Types.VARCHAR,"right"));m.setTableInfo(c);return m;}
 static GenericDBWritable output(GenericDBLoadDataMapper<String,String> m,String row)throws Exception{Mapper.Context ctx=mock(Mapper.Context.class);m.map("key",row,ctx);ArgumentCaptor<GenericDBWritable> a=ArgumentCaptor.forClass(GenericDBWritable.class);verify(ctx).write(a.capture(),isNull());return a.getValue();}
 public static void main(String[] args)throws Exception{
  if(!output(mapper(Types.VARCHAR),"a,b,c").getValues().equals(List.of("a","b","c")))throw new AssertionError();System.out.println("PASS ordinary3column row");
  try{output(mapper(Types.VARCHAR),"a,,c");throw new AssertionError();}catch(IOException expected){System.out.println("OBSERVED empty middle field rejected by token count");}
  if(!output(mapper(Types.VARCHAR),"a, ,c").getValues().equals(Arrays.asList("a",null,"c")))throw new AssertionError();System.out.println("PASS whitespace field retained as null");
  GenericDBWritable dropped=output(mapper(Types.DECIMAL),"a,2.5,c");if(dropped.getColumns().size()!=3||!dropped.getValues().equals(List.of("a","c")))throw new AssertionError();System.out.println("OBSERVED DECIMAL field silently omitted:metadata3,values2;mapper checks=4");
 }
}
''')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
commands=[[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),*sources,str(src/'SelectedOutputTest.java'),str(src/'GenericMapperControl.java')],[str(j/'bin/java'),'-cp',cp,'SelectedOutputTest'],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'GenericMapperControl']]
for i,args in enumerate(commands):
 p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if i==0 and p.returncode:break
out=dict(author_setup_correction='Old Mockito1.8 eq(null) matcher threw in authored control; replaced with isNull. Original sources unchanged; repeated original metadata test counted once.',initial_attempt=json.loads((root/'.dedup-work/hiho098-generic-output-initial-null-matcher-20261024.json').read_text()),scope='Exactly1original JSON metadata test selected; DB-dependent testPopulateColumnInfo not run.4authorcontrols execute original mapper with configured metadata and recording contexts. Jackson1.4.0 matches pinnedpom property, not whole resolved dependency cohort; no external DB/SQL/network.',commands=steps)
(root/'docs/research/dedup-500/hiho098-generic-output-native-controls-20261024.json').write_text(json.dumps(out,indent=2)+'\n');assert len(steps)==3 and steps[0]['returncode']==0 and steps[2]['returncode']==0
