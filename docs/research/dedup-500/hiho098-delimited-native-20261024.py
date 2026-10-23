from pathlib import Path
import json,subprocess,shutil
root=Path.cwd();runtime=root/'.dedup-work/hiho098-runtime-20261024';j=next(runtime.glob('*/Contents/Home'));base=root/'.dedup-work/hiho098-format-runtime-20261024';src=base/'src';classes=base/'classes';f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';sources=[]
for local in ['127-DBInputDelimMapper.java','131-TestDBInputDelimMapper.java']:
 p=src/local.split('-',1)[1];shutil.copyfile(f/local,p);sources.append(str(p))
(src/'DelimitedBoundaryControl.java').write_text('''import co.nubetech.hiho.mapreduce.DBInputDelimMapper;import co.nubetech.hiho.mapreduce.lib.db.*;import co.nubetech.hiho.common.HIHOConf;import org.apache.hadoop.conf.Configuration;import org.apache.hadoop.io.*;import org.apache.hadoop.mapreduce.Mapper;import org.mockito.ArgumentCaptor;import java.util.*;import java.sql.Types;import static org.mockito.Mockito.*;
public class DelimitedBoundaryControl{
 static String output(String delimiter,Object... values)throws Exception{Configuration c=new Configuration();if(delimiter!=null)c.set(HIHOConf.INPUT_OUTPUT_DELIMITER,delimiter);Mapper.Context ctx=mock(Mapper.Context.class);when(ctx.getConfiguration()).thenReturn(c);ArrayList<ColumnInfo> cols=new ArrayList<>();for(int i=0;i<values.length;i++)cols.add(new ColumnInfo(i,Types.VARCHAR,"c"+i));GenericDBWritable value=new GenericDBWritable(cols,new ArrayList<>(Arrays.asList(values)));new DBInputDelimMapper().map(new LongWritable(0),value,ctx);ArgumentCaptor<Text> a=ArgumentCaptor.forClass(Text.class);verify(ctx).write(any(Text.class),a.capture());return a.getValue().toString();}
 public static void main(String[] args)throws Exception{
  if(!output("DELIM","a","b").equals("aDELIMb"))throw new AssertionError();System.out.println("PASS multi-character delimiter control");
  String first=output(",","a,b","c"),second=output(",","a","b,c");if(!first.equals(second)||!first.equals("a,b,c"))throw new AssertionError();System.out.println("OBSERVED distinct field lists serialize identically when field contains delimiter");
  if(!output(",",(Object)null).equals(output(",","null")))throw new AssertionError();System.out.println("OBSERVED null and literal null string serialize identically");
  try{output(null,"a");throw new AssertionError();}catch(NullPointerException expected){System.out.println("OBSERVED missing delimiter configuration throws NullPointerException");}System.out.println("delimited checks=4");
 }
}
''')
cp=str(classes)+':'+str(runtime/'jars/*');steps=[]
for i,args in enumerate([[str(j/'bin/javac'),'-cp',cp,'-d',str(classes),*sources,str(src/'DelimitedBoundaryControl.java')],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'org.junit.runner.JUnitCore','co.nubetech.hiho.mapreduce.TestDBInputDelimMapper'],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'DelimitedBoundaryControl']]):
 p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if i==0 and p.returncode:break
(root/'docs/research/dedup-500/hiho098-delimited-native-controls-20261024.json').write_text(json.dumps(dict(scope='3originalJUnitmethods and4authorcontrols of unchanged DBInputDelimMapper; realHadoop types/Mockito; no DB/network/cluster; original test failures if any preserved',commands=steps),indent=2)+'\n');assert len(steps)==3 and steps[0]['returncode']==0 and steps[2]['returncode']==0
